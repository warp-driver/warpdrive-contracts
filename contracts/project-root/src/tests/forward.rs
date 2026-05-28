extern crate std;

use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{
    Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec,
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

/// Deploys ProjectRoot as the admin of a Secp256k1Security contract. This is
/// the realistic proxy-admin setup: a single rotation point (ProjectRoot) sits
/// in front of N downstream contracts and forwards admin calls to them.
fn deploy_proxy<'a>(env: &Env) -> (ProjectRootClient<'a>, Secp256k1SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let placeholder = Address::generate(env);
    let repo = String::from_str(env, "https://github.com/example/spec");
    let project_root_id = env.register(
        ProjectRoot,
        (
            &admin,
            &placeholder,
            &placeholder,
            &repo,
            VerificationType::Ethereum,
        ),
    );

    let security_id = env.register(Secp256k1Security, (&project_root_id, 2u64, 3u64));

    (
        ProjectRootClient::new(env, &project_root_id),
        Secp256k1SecurityClient::new(env, &security_id),
        admin,
    )
}

/// Same as deploy_proxy but wires the project_root's stored `security_contract`
/// to a downstream Secp256k1Security instance — so the typed helpers
/// (add_secp256k1_signer etc.) resolve to it.
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

fn add_signer_args(env: &Env, key: &BytesN<33>, weight: u64) -> Vec<Val> {
    let mut args = Vec::new(env);
    args.push_back(key.to_val());
    args.push_back(weight.into_val(env));
    args
}

// ── Generic forward: kept to cover the untyped path ─────────────────────

#[test]
fn forward_returns_inner_call_value_for_queries() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(7));
    let weight: u64 = 42;

    let add = Symbol::new(&env, "add_signer");
    project_root.forward(
        &security.address,
        &add,
        &add_signer_args(&env, &key, weight),
    );

    // Route a query through forward and assert the returned Val decodes.
    let get = Symbol::new(&env, "get_signer_weight");
    let mut args = Vec::new(&env);
    args.push_back(key.to_val());
    let result: Val = project_root.forward(&security.address, &get, &args);
    let got: u64 = result.into_val(&env);
    assert_eq!(got, weight);
}

#[test]
fn forward_rejects_non_admin_caller() {
    let env = Env::default();

    let (project_root, security, _admin) = deploy_proxy(&env);
    let attacker = Address::generate(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(2));
    let args = add_signer_args(&env, &key, 50);
    let fn_name = Symbol::new(&env, "add_signer");

    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "forward",
            args: (security.address.clone(), fn_name.clone(), args.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let result = project_root.try_forward(&security.address, &fn_name, &args);
    assert!(result.is_err());

    env.mock_all_auths();
    assert_eq!(security.get_signer_weight(&key), 0);
}

#[test]
fn forward_uses_rotated_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, old_admin) = deploy_proxy(&env);
    let new_admin = Address::generate(&env);

    project_root.propose_admin(&new_admin);
    project_root.accept_admin();
    assert_eq!(project_root.admin(), new_admin);

    let key = secp256k1_pubkey(&env, &make_secp256k1_key(3));
    let args = add_signer_args(&env, &key, 50);
    let fn_name = Symbol::new(&env, "add_signer");
    env.mock_auths(&[MockAuth {
        address: &old_admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "forward",
            args: (security.address.clone(), fn_name.clone(), args.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = project_root.try_forward(&security.address, &fn_name, &args);
    assert!(result.is_err());

    env.mock_auths(&[MockAuth {
        address: &new_admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "forward",
            args: (security.address.clone(), fn_name.clone(), args.clone()).into_val(&env),
            sub_invokes: &[MockAuthInvoke {
                contract: &security.address,
                fn_name: "add_signer",
                args: (key.clone(), 50u64).into_val(&env),
                sub_invokes: &[],
            }],
        },
    }]);
    project_root.forward(&security.address, &fn_name, &args);
    assert_eq!(security.get_signer_weight(&key), 50);
}

#[test]
fn forward_inner_call_does_not_inherit_admin_auth() {
    // Reentrancy: each require_auth in the call tree needs its own entry.
    // Admin authorizing forward(...) does NOT carry over to an inner call
    // that also does require_auth. Demonstrate this by having forward call
    // back into project_root.update_project_spec_repo, which requires its
    // own admin auth. Without a sub_invoke for it, the inner call fails.
    let env = Env::default();

    let (project_root, _security, admin) = deploy_proxy(&env);
    let new_repo = String::from_str(&env, "https://github.com/attacker/spec");
    let fn_name = Symbol::new(&env, "update_project_spec_repo");
    let mut args = Vec::new(&env);
    args.push_back(new_repo.to_val());

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "forward",
            args: (project_root.address.clone(), fn_name.clone(), args.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    let result = project_root.try_forward(&project_root.address, &fn_name, &args);
    assert!(result.is_err());

    assert_eq!(
        project_root.project_spec_repo(),
        String::from_str(&env, "https://github.com/example/spec")
    );
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
