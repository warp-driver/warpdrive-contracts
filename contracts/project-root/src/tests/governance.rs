//! End-to-end deployment + handover test.
//!
//! Walks the exact flow a real deploy script must perform:
//!
//!   1. Deploy security, verification, handler, project_root (deployer is
//!      admin of all four).
//!   2. Deployer configures security: registers a signer and threshold.
//!   3. project_root construction "registers" security + verification.
//!   4. Deployer hands every downstream admin to project_root via the
//!      propose/accept dance, using accept_contract_admin so project_root
//!      finishes the handover on its own behalf.
//!   5. Deployer hands project_root's admin to a separate owner address
//!      (multisig stand-in).
//!   6. The deployer must retain zero privileges after the script finishes.
//!      Verified by attempting every admin-gated op authorized by the
//!      deployer and asserting each one fails.
//!
//! The same fixture is then used to confirm the new owner can govern the
//! deployment through project_root: setting signers on security and
//! upgrading verification.

extern crate std;

use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{
    Address, Env, IntoVal, String,
    testutils::{Address as _, MockAuth, MockAuthInvoke},
};
use warpdrive_ed25519_security::{Ed25519Security, Ed25519SecurityClient};
use warpdrive_ed25519_verification::{Ed25519Verification, Ed25519VerificationClient};
use warpdrive_shared::interfaces::project_root::VerificationType;
use warpdrive_shared::testutils::{ed25519_pubkey, make_ed25519_key};
use warpdrive_stellar_handler::{StellarHandler, StellarHandlerClient};

use super::setup::install_contract_wasm;

// ── Helpers ────────────────────────────────────────────────────────────

/// Holds every address/client a deploy script touches. Kept loose so each
/// step in the test reads like the corresponding shell-script step.
struct Deployment<'a> {
    deployer: Address,
    owner: Address,
    project_root: ProjectRootClient<'a>,
    security: Ed25519SecurityClient<'a>,
    verification: Ed25519VerificationClient<'a>,
    handler: StellarHandlerClient<'a>,
}

/// Run the deploy script. Mirrors the steps documented above.
fn run_deployment_script(env: &Env) -> Deployment<'_> {
    let deployer = Address::generate(env);
    let owner = Address::generate(env);

    env.mock_all_auths();

    // Step 1: deploy each governed contract with deployer as admin.
    let security_id = env.register(Ed25519Security, (&deployer, 2u64, 3u64));
    let security = Ed25519SecurityClient::new(env, &security_id);

    let verification_id = env.register(Ed25519Verification, (&deployer, &security_id));
    let verification = Ed25519VerificationClient::new(env, &verification_id);

    let handler_id = env.register(StellarHandler, (&deployer, &verification_id));
    let handler = StellarHandlerClient::new(env, &handler_id);

    // Step 2: deployer configures the security contract.
    let initial_signer = ed25519_pubkey(env, &make_ed25519_key(0));
    security.add_signer(&initial_signer, &100);

    // Step 3: deploy project_root. The constructor wires (registers) the
    // verification + security addresses; no separate registration call is
    // required because they're immutable for the lifetime of the deployment.
    let repo = String::from_str(env, "https://github.com/example/spec");
    let project_root_id = env.register(
        ProjectRoot,
        (
            &deployer,
            &security_id,
            &verification_id,
            &repo,
            VerificationType::Stellar,
        ),
    );
    let project_root = ProjectRootClient::new(env, &project_root_id);

    // Step 4: rotate admin on every governed contract to project_root.
    // Each contract: deployer (current admin) proposes; project_root (its
    // own admin still being deployer) accepts via accept_contract_admin.
    for contract_addr in [&security.address, &verification.address, &handler.address] {
        let downstream_propose = MockAuthInvoke {
            contract: contract_addr,
            fn_name: "propose_admin",
            args: (project_root_id.clone(),).into_val(env),
            sub_invokes: &[],
        };
        env.mock_auths(&[MockAuth {
            address: &deployer,
            invoke: &downstream_propose,
        }]);
        // Re-build a typed handle just to dispatch the call uniformly.
        if contract_addr == &security.address {
            security.propose_admin(&project_root_id);
        } else if contract_addr == &verification.address {
            verification.propose_admin(&project_root_id);
        } else {
            handler.propose_admin(&project_root_id);
        }

        // Deployer is still project_root's admin, so they authorize the
        // accept_contract_admin call which in turn finishes the handover.
        env.mock_auths(&[MockAuth {
            address: &deployer,
            invoke: &MockAuthInvoke {
                contract: &project_root.address,
                fn_name: "accept_contract_admin",
                args: ((*contract_addr).clone(),).into_val(env),
                sub_invokes: &[],
            },
        }]);
        project_root.accept_contract_admin(contract_addr);
    }

    // Step 5: rotate project_root's admin from deployer to owner.
    env.mock_auths(&[MockAuth {
        address: &deployer,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "propose_admin",
            args: (owner.clone(),).into_val(env),
            sub_invokes: &[],
        },
    }]);
    project_root.propose_admin(&owner);

    env.mock_auths(&[MockAuth {
        address: &owner,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "accept_admin",
            args: ().into_val(env),
            sub_invokes: &[],
        },
    }]);
    project_root.accept_admin();

    Deployment {
        deployer,
        owner,
        project_root,
        security,
        verification,
        handler,
    }
}

// ── Step 6: the deployer is left with zero privileges ──────────────────

#[test]
fn deployer_has_no_remaining_privileges_after_handover() {
    let env = Env::default();
    let d = run_deployment_script(&env);

    // Sanity: storage-level admin checks for every contract.
    assert_eq!(d.security.admin(), d.project_root.address);
    assert_eq!(d.verification.admin(), d.project_root.address);
    assert_eq!(d.handler.admin(), d.project_root.address);
    assert_eq!(d.project_root.admin(), d.owner);

    // Behavioural checks: every admin-gated path with only the deployer's
    // auth available must fail.

    // (a) Deployer can't touch security directly.
    let new_signer = ed25519_pubkey(&env, &make_ed25519_key(9));
    env.mock_auths(&[MockAuth {
        address: &d.deployer,
        invoke: &MockAuthInvoke {
            contract: &d.security.address,
            fn_name: "add_signer",
            args: (new_signer.clone(), 50u64).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(d.security.try_add_signer(&new_signer, &50).is_err());

    // (b) Deployer can't upgrade verification directly.
    let wasm_hash = install_contract_wasm(&env);
    env.cost_estimate().budget().reset_unlimited();
    let new_version = String::from_str(&env, "9.9.9");
    env.mock_auths(&[MockAuth {
        address: &d.deployer,
        invoke: &MockAuthInvoke {
            contract: &d.verification.address,
            fn_name: "upgrade",
            args: (wasm_hash.clone(), new_version.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        d.verification
            .try_upgrade(&wasm_hash, &new_version)
            .is_err()
    );

    // (c) Deployer can't upgrade the handler directly.
    env.mock_auths(&[MockAuth {
        address: &d.deployer,
        invoke: &MockAuthInvoke {
            contract: &d.handler.address,
            fn_name: "upgrade",
            args: (wasm_hash.clone(), new_version.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(d.handler.try_upgrade(&wasm_hash, &new_version).is_err());

    // (d) Deployer can't even speak to project_root: not as the admin,
    //     not as a forwarder, not as a typed helper.
    env.mock_auths(&[MockAuth {
        address: &d.deployer,
        invoke: &MockAuthInvoke {
            contract: &d.project_root.address,
            fn_name: "upgrade",
            args: (wasm_hash.clone(), new_version.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        d.project_root
            .try_upgrade(&wasm_hash, &new_version)
            .is_err()
    );

    env.mock_auths(&[MockAuth {
        address: &d.deployer,
        invoke: &MockAuthInvoke {
            contract: &d.project_root.address,
            fn_name: "add_ed25519_signer",
            args: (new_signer.clone(), 50u64).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        d.project_root
            .try_add_ed25519_signer(&new_signer, &50)
            .is_err()
    );

    // (e) Final state is unchanged by all those failed attempts.
    assert_eq!(d.security.get_signer_weight(&new_signer), 0);
}

// ── Owner exercises governance through project_root ────────────────────

#[test]
fn owner_can_set_signers_on_security_via_project_root() {
    let env = Env::default();
    let d = run_deployment_script(&env);

    let new_signer = ed25519_pubkey(&env, &make_ed25519_key(1));
    let weight: u64 = 75;

    env.mock_auths(&[MockAuth {
        address: &d.owner,
        invoke: &MockAuthInvoke {
            contract: &d.project_root.address,
            fn_name: "add_ed25519_signer",
            args: (new_signer.clone(), weight).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    d.project_root.add_ed25519_signer(&new_signer, &weight);

    assert_eq!(d.security.get_signer_weight(&new_signer), weight);
}

#[test]
fn owner_can_upgrade_verification_via_project_root() {
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    let d = run_deployment_script(&env);

    let new_wasm_hash = install_contract_wasm(&env);
    let new_version = String::from_str(&env, "2.0.0");

    env.mock_auths(&[MockAuth {
        address: &d.owner,
        invoke: &MockAuthInvoke {
            contract: &d.project_root.address,
            fn_name: "upgrade_contract",
            args: (
                d.verification.address.clone(),
                new_wasm_hash.clone(),
                new_version.clone(),
            )
                .into_val(&env),
            sub_invokes: &[],
        },
    }]);
    d.project_root
        .upgrade_contract(&d.verification.address, &new_wasm_hash, &new_version);

    // Storage is preserved across upgrades, so version() reflects the new
    // value even though the contract bytecode is now project_root's.
    assert_eq!(d.verification.version(), new_version);
}
