extern crate std;

use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{
    Address, Env, IntoVal, String,
    testutils::{Address as _, MockAuth, MockAuthInvoke},
};
use warpdrive_ed25519_security::{Ed25519Security, Ed25519SecurityClient};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_shared::interfaces::project_root::VerificationType;
use warpdrive_shared::interfaces::security::SecurityError;
use warpdrive_shared::testutils::{
    ed25519_pubkey, make_ed25519_key, make_secp256k1_key, secp256k1_pubkey,
};

use super::setup::install_contract_wasm;

// ── Test fixtures ───────────────────────────────────────────────────────

/// Deploys ProjectRoot with the given Secp256k1Security set as its registered
/// security contract, then rotates the security's admin to project_root so
/// the typed helpers can govern it.
fn deploy_proxy_with_registered_security<'a>(
    env: &Env,
) -> (ProjectRootClient<'a>, Secp256k1SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let verification_placeholder = Address::generate(env);
    let repo = String::from_str(env, "https://github.com/example/spec");

    // Bootstrap: deploy a security with the EoA as admin, deploy project_root
    // pointing at it, then rotate security's admin to project_root.
    let security_id = env.register(Secp256k1Security, (&admin, 2u64, 3u64));
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &verification_placeholder,
            &repo,
            VerificationType::Ethereum,
        ),
    );
    let security = Secp256k1SecurityClient::new(env, &security_id);
    env.mock_all_auths();
    security.propose_admin(&project_root_id);
    security.accept_admin();
    assert_eq!(security.admin(), project_root_id);

    (
        ProjectRootClient::new(env, &project_root_id),
        security,
        admin,
    )
}

/// Same as deploy_proxy_with_registered_security but for an Ed25519Security.
fn deploy_proxy_with_registered_ed25519<'a>(
    env: &Env,
) -> (ProjectRootClient<'a>, Ed25519SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let verification_placeholder = Address::generate(env);
    let repo = String::from_str(env, "https://github.com/example/spec");

    let security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &verification_placeholder,
            &repo,
            VerificationType::Stellar,
        ),
    );
    let security = Ed25519SecurityClient::new(env, &security_id);
    env.mock_all_auths();
    security.propose_admin(&project_root_id);
    security.accept_admin();
    assert_eq!(security.admin(), project_root_id);

    (
        ProjectRootClient::new(env, &project_root_id),
        security,
        admin,
    )
}

// ── Typed security helpers: secp256k1 ──────────────────────────────────

#[test]
fn add_secp256k1_signer_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(1));
    let weight: u64 = 100;

    assert_eq!(security.get_signer_weight(&key), 0);

    project_root.add_secp256k1_signer(&key, &weight);

    assert_eq!(security.get_signer_weight(&key), weight);
    assert_eq!(security.get_total_weight(), weight);
}

#[test]
fn remove_secp256k1_signer_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(5));
    project_root.add_secp256k1_signer(&key, &75);
    assert_eq!(security.get_signer_weight(&key), 75);

    project_root.remove_secp256k1_signer(&key);

    assert_eq!(security.get_signer_weight(&key), 0);
    assert_eq!(security.get_total_weight(), 0);
}

#[test]
fn add_secp256k1_signer_propagates_zero_weight_error() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, _security, _admin) = deploy_proxy_with_registered_security(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(4));

    let result = project_root.try_add_secp256k1_signer(&key, &0);
    assert_eq!(result, Err(Ok(SecurityError::ZeroWeight)));
}

#[test]
fn add_secp256k1_signer_rejects_non_admin_caller() {
    let env = Env::default();

    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    let attacker = Address::generate(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(6));

    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "add_secp256k1_signer",
            args: (key.clone(), 50u64).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let result = project_root.try_add_secp256k1_signer(&key, &50);
    assert!(result.is_err());

    env.mock_all_auths();
    assert_eq!(security.get_signer_weight(&key), 0);
}

// ── Typed security helpers: ed25519 ────────────────────────────────────

#[test]
fn add_ed25519_signer_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_ed25519(&env);
    let key = ed25519_pubkey(&env, &make_ed25519_key(1));
    let weight: u64 = 88;

    project_root.add_ed25519_signer(&key, &weight);

    assert_eq!(security.get_signer_weight(&key), weight);
    assert_eq!(security.get_total_weight(), weight);
}

#[test]
fn remove_ed25519_signer_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_ed25519(&env);
    let key = ed25519_pubkey(&env, &make_ed25519_key(2));
    project_root.add_ed25519_signer(&key, &30);

    project_root.remove_ed25519_signer(&key);

    assert_eq!(security.get_signer_weight(&key), 0);
}

// ── Typed security helper: set_threshold ───────────────────────────────

#[test]
fn set_threshold_helper_succeeds() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    assert_eq!(security.threshold_numerator(), 2);
    assert_eq!(security.threshold_denominator(), 3);

    project_root.set_threshold(&3, &5);

    assert_eq!(security.threshold_numerator(), 3);
    assert_eq!(security.threshold_denominator(), 5);
}

#[test]
fn set_threshold_propagates_zero_denominator_error() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, _security, _admin) = deploy_proxy_with_registered_security(&env);

    let result = project_root.try_set_threshold(&1, &0);
    assert_eq!(result, Err(Ok(SecurityError::ZeroDenominator)));
}

// ── Typed WarpDriveInterface forwarders ────────────────────────────────

#[test]
fn upgrade_contract_helper_forwards_to_target() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    // Use the project_root wasm as the "new" wasm. The point of this test
    // is to verify the forwarded upgrade call reaches the target and that
    // its admin gate (project_root) accepts. We can't usefully call into
    // the security contract afterward — its bytecode is now project_root's
    // — but version() reads instance storage which is preserved.
    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    let new_wasm_hash = install_contract_wasm(&env);
    let new_version = String::from_str(&env, "9.9.9");

    project_root.upgrade_contract(&security.address, &new_wasm_hash, &new_version);

    assert_eq!(security.version(), new_version);
}

#[test]
fn propose_contract_admin_rotates_downstream_admin_away() {
    // ProjectRoot is currently security's admin. Use propose_contract_admin
    // to hand admin off to an external EoA, then have the EoA accept.
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy_with_registered_security(&env);
    let external_admin = Address::generate(&env);

    assert_eq!(security.admin(), project_root.address);
    project_root.propose_contract_admin(&security.address, &external_admin);
    assert_eq!(security.pending_admin(), Some(external_admin.clone()));

    // External EoA finishes the rotation directly on security.
    security.accept_admin();
    assert_eq!(security.admin(), external_admin);
    assert_eq!(security.pending_admin(), None);
}

#[test]
fn propose_contract_admin_accepts_handler_pointing_at_registered_verification() {
    // A handler whose verification_contract() returns our registered
    // verification is part of this project, so propose_contract_admin must
    // forward the rotation through to it.
    use warpdrive_ed25519_verification::Ed25519Verification;
    use warpdrive_stellar_handler::{StellarHandler, StellarHandlerClient};

    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let repo = String::from_str(&env, "https://github.com/example/spec");
    let security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Ed25519Verification, (&admin, &security_id));
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &verification_id,
            &repo,
            VerificationType::Stellar,
        ),
    );
    let project_root = ProjectRootClient::new(&env, &project_root_id);

    // Handler points at our registered verification, then hands its admin to
    // project_root so the forwarded propose_admin clears the target's gate.
    let handler_id = env.register(StellarHandler, (&admin, &verification_id));
    let handler = StellarHandlerClient::new(&env, &handler_id);
    handler.propose_admin(&project_root.address);
    handler.accept_admin();
    assert_eq!(handler.admin(), project_root.address);

    let next_admin = Address::generate(&env);
    project_root.propose_contract_admin(&handler.address, &next_admin);
    assert_eq!(handler.pending_admin(), Some(next_admin));
}

#[test]
fn propose_contract_admin_rejects_handler_with_other_verification() {
    // A handler that reports a different verification address is not part of
    // this project and must be rejected even though it implements the
    // verification_contract() query.
    use warpdrive_ed25519_verification::Ed25519Verification;
    use warpdrive_stellar_handler::{StellarHandler, StellarHandlerClient};

    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let repo = String::from_str(&env, "https://github.com/example/spec");
    let security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let our_verification_id = env.register(Ed25519Verification, (&admin, &security_id));
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &our_verification_id,
            &repo,
            VerificationType::Stellar,
        ),
    );
    let project_root = ProjectRootClient::new(&env, &project_root_id);

    // A second, independent verification contract — and a handler pointing at
    // it. From project_root's perspective this handler is foreign.
    let other_security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let other_verification_id = env.register(Ed25519Verification, (&admin, &other_security_id));
    let foreign_handler_id = env.register(StellarHandler, (&admin, &other_verification_id));
    let foreign_handler = StellarHandlerClient::new(&env, &foreign_handler_id);
    foreign_handler.propose_admin(&project_root.address);
    foreign_handler.accept_admin();
    assert_eq!(foreign_handler.admin(), project_root.address);

    let next_admin = Address::generate(&env);
    assert!(
        project_root
            .try_propose_contract_admin(&foreign_handler.address, &next_admin)
            .is_err()
    );
    assert_eq!(foreign_handler.pending_admin(), None);
}

#[test]
fn accept_contract_admin_takes_over_downstream() {
    // Inverse flow: a downstream contract has an external admin who proposes
    // ProjectRoot. ProjectRoot's admin then uses accept_contract_admin to
    // take over. The downstream must be the security or verification contract
    // registered with project_root — see ensure_our_contract.
    let env = Env::default();
    env.mock_all_auths();

    let external_admin = Address::generate(&env);
    let placeholder = Address::generate(&env);
    let repo = String::from_str(&env, "https://github.com/example/spec");

    // Pre-deploy the security contract with the external admin so we can
    // register it as project_root's security from construction time.
    let security_id = env.register(Secp256k1Security, (&external_admin, 2u64, 3u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);

    let project_root_id = env.register(
        ProjectRoot,
        (
            &external_admin,
            &security_id,
            &placeholder,
            &repo,
            VerificationType::Ethereum,
        ),
    );
    let project_root = ProjectRootClient::new(&env, &project_root_id);

    // External admin proposes project_root as the next admin of security.
    security.propose_admin(&project_root.address);
    assert_eq!(security.pending_admin(), Some(project_root.address.clone()));

    // ProjectRoot's admin uses the helper to accept on project_root's behalf.
    project_root.accept_contract_admin(&security.address);

    assert_eq!(security.admin(), project_root.address);
    assert_eq!(security.pending_admin(), None);
}

#[test]
fn accept_contract_admin_rejects_unrelated_target() {
    // A security contract that was never registered with project_root must
    // not be acceptable, even if it has proposed project_root as its admin.
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, _our_security, _admin) = deploy_proxy_with_registered_security(&env);

    let external_admin = Address::generate(&env);
    let unrelated_id = env.register(Secp256k1Security, (&external_admin, 2u64, 3u64));
    let unrelated = Secp256k1SecurityClient::new(&env, &unrelated_id);
    unrelated.propose_admin(&project_root.address);

    assert!(
        project_root
            .try_accept_contract_admin(&unrelated.address)
            .is_err()
    );
    // Pending state on the unrelated contract is untouched.
    assert_eq!(
        unrelated.pending_admin(),
        Some(project_root.address.clone())
    );
    assert_eq!(unrelated.admin(), external_admin);
}

#[test]
fn upgrade_contract_rejects_unrelated_target() {
    // A contract that isn't part of the project must not be upgradable
    // through project_root, even if project_root happens to be its admin.
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let (project_root, _our_security, _admin) = deploy_proxy_with_registered_security(&env);

    // Spin up an unrelated security whose admin we hand to project_root so
    // the only barrier to upgrading it is the ensure_our_contract check.
    let bootstrap_admin = Address::generate(&env);
    let unrelated_id = env.register(Secp256k1Security, (&bootstrap_admin, 2u64, 3u64));
    let unrelated = Secp256k1SecurityClient::new(&env, &unrelated_id);
    unrelated.propose_admin(&project_root.address);
    unrelated.accept_admin();
    assert_eq!(unrelated.admin(), project_root.address);

    let new_wasm_hash = install_contract_wasm(&env);
    let new_version = String::from_str(&env, "9.9.9");

    assert!(
        project_root
            .try_upgrade_contract(&unrelated.address, &new_wasm_hash, &new_version)
            .is_err()
    );
}

#[test]
fn upgrade_contract_accepts_handler_pointing_at_registered_verification() {
    // A handler that reports our registered verification address must be
    // accepted as part of this project even though it isn't directly
    // registered in project_root storage.
    use warpdrive_ed25519_verification::Ed25519Verification;
    use warpdrive_stellar_handler::{StellarHandler, StellarHandlerClient};

    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    // Build project_root with a real verification contract so the handler
    // we spin up below can point at it.
    let admin = Address::generate(&env);
    let repo = String::from_str(&env, "https://github.com/example/spec");
    let security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Ed25519Verification, (&admin, &security_id));
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &verification_id,
            &repo,
            VerificationType::Stellar,
        ),
    );
    let project_root = ProjectRootClient::new(&env, &project_root_id);

    // Deploy a handler that points back at our verification, then rotate its
    // admin to project_root so the upgrade can land.
    let handler_id = env.register(StellarHandler, (&admin, &verification_id));
    let handler = StellarHandlerClient::new(&env, &handler_id);
    handler.propose_admin(&project_root.address);
    handler.accept_admin();
    assert_eq!(handler.admin(), project_root.address);

    let new_wasm_hash = install_contract_wasm(&env);
    let new_version = String::from_str(&env, "9.9.9");
    project_root.upgrade_contract(&handler.address, &new_wasm_hash, &new_version);
    assert_eq!(handler.version(), new_version);
}

// ── ProjectRoot-level admin behaviour, exercised through typed helpers ─

#[test]
fn typed_helper_uses_rotated_admin() {
    // Converted from the old generic-forward rotation test: after rotating
    // ProjectRoot's admin, only the NEW admin can invoke a typed helper.
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, old_admin) = deploy_proxy_with_registered_security(&env);
    let new_admin = Address::generate(&env);

    project_root.propose_admin(&new_admin);
    project_root.accept_admin();
    assert_eq!(project_root.admin(), new_admin);

    let key = secp256k1_pubkey(&env, &make_secp256k1_key(3));
    let weight: u64 = 50;

    // Old admin's auth no longer satisfies the typed helper.
    env.mock_auths(&[MockAuth {
        address: &old_admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "add_secp256k1_signer",
            args: (key.clone(), weight).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    assert!(
        project_root
            .try_add_secp256k1_signer(&key, &weight)
            .is_err()
    );

    // New admin's auth does.
    env.mock_auths(&[MockAuth {
        address: &new_admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "add_secp256k1_signer",
            args: (key.clone(), weight).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    project_root.add_secp256k1_signer(&key, &weight);
    assert_eq!(security.get_signer_weight(&key), weight);
}

#[test]
fn typed_helper_inner_require_auth_does_not_inherit_outer_auth() {
    // Converted from the old generic-forward reentrancy test. Admin
    // authorizes propose_contract_admin pointing back at project_root —
    // the inner call hits ProjectRoot::propose_admin which itself does
    // admin.require_auth. That nested require_auth requires its own auth
    // entry; with mock_auths declaring only the outer call, the inner
    // one fails and no pending admin is recorded.
    let env = Env::default();

    let (project_root, _security, admin) = deploy_proxy_with_registered_security(&env);
    let attacker = Address::generate(&env);

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "propose_contract_admin",
            args: (project_root.address.clone(), attacker.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let result = project_root.try_propose_contract_admin(&project_root.address, &attacker);
    assert!(result.is_err());
    assert_eq!(project_root.pending_admin(), None);
}
