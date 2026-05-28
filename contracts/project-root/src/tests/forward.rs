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
fn accept_contract_admin_takes_over_downstream() {
    // Inverse flow: a downstream contract has an external admin who proposes
    // ProjectRoot. ProjectRoot's admin then uses accept_contract_admin to
    // take over.
    let env = Env::default();
    env.mock_all_auths();

    let external_admin = Address::generate(&env);
    let placeholder = Address::generate(&env);
    let repo = String::from_str(&env, "https://github.com/example/spec");
    let project_root_id = env.register(
        ProjectRoot,
        (
            &external_admin,
            &placeholder,
            &placeholder,
            &repo,
            VerificationType::Ethereum,
        ),
    );
    let project_root = ProjectRootClient::new(&env, &project_root_id);

    let security_id = env.register(Secp256k1Security, (&external_admin, 2u64, 3u64));
    let security = Secp256k1SecurityClient::new(&env, &security_id);

    // External admin proposes project_root as the next admin of security.
    security.propose_admin(&project_root.address);
    assert_eq!(security.pending_admin(), Some(project_root.address.clone()));

    // ProjectRoot's admin uses the helper to accept on project_root's behalf.
    project_root.accept_contract_admin(&security.address);

    assert_eq!(security.admin(), project_root.address);
    assert_eq!(security.pending_admin(), None);
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
