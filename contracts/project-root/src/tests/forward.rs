extern crate std;

use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{
    Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec,
    testutils::{Address as _, MockAuth, MockAuthInvoke},
};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_shared::interfaces::project_root::VerificationType;
use warpdrive_shared::interfaces::security::SecurityError;
use warpdrive_shared::testutils::{make_secp256k1_key, secp256k1_pubkey};

/// Deploys ProjectRoot as the admin of a Secp256k1Security contract. This is
/// the realistic proxy-admin setup: a single rotation point (ProjectRoot) sits
/// in front of N downstream contracts and forwards admin calls to them.
fn deploy_proxy<'a>(env: &Env) -> (ProjectRootClient<'a>, Secp256k1SecurityClient<'a>, Address) {
    let admin = Address::generate(env);

    // Deploy ProjectRoot first with placeholder downstream addresses; we'll
    // wire up the real security_contract address inside the tests via
    // forward(). For these tests, project_root.security_contract is unused.
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

    // Now deploy a Secp256k1Security whose admin is project_root itself.
    let security_id = env.register(Secp256k1Security, (&project_root_id, 2u64, 3u64));

    (
        ProjectRootClient::new(env, &project_root_id),
        Secp256k1SecurityClient::new(env, &security_id),
        admin,
    )
}

fn add_signer_args(env: &Env, key: &BytesN<33>, weight: u64) -> Vec<Val> {
    let mut args = Vec::new(env);
    args.push_back(key.to_val());
    args.push_back(weight.into_val(env));
    args
}

// ── Happy path ──────────────────────────────────────────────────────────

#[test]
fn forward_invokes_add_signer_on_downstream_security() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(1));
    let weight: u64 = 100;

    assert_eq!(security.get_signer_weight(&key), 0);

    let fn_name = Symbol::new(&env, "add_signer");
    project_root.forward(
        &security.address,
        &fn_name,
        &add_signer_args(&env, &key, weight),
    );

    assert_eq!(security.get_signer_weight(&key), weight);
    assert_eq!(security.get_total_weight(), weight);
}

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

    // Now route a query through forward and assert the returned Val decodes.
    let get = Symbol::new(&env, "get_signer_weight");
    let mut args = Vec::new(&env);
    args.push_back(key.to_val());
    let result: Val = project_root.forward(&security.address, &get, &args);
    let got: u64 = result.into_val(&env);
    assert_eq!(got, weight);
}

// ── AC1 unhappy paths ──────────────────────────────────────────────────

#[test]
fn forward_rejects_non_admin_caller() {
    let env = Env::default();

    let (project_root, security, _admin) = deploy_proxy(&env);
    let attacker = Address::generate(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(2));
    let args = add_signer_args(&env, &key, 50);
    let fn_name = Symbol::new(&env, "add_signer");

    // Attacker authorizes their own call; admin auth is missing.
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

    // And the downstream state is unchanged.
    env.mock_all_auths();
    assert_eq!(security.get_signer_weight(&key), 0);
}

#[test]
fn forward_uses_rotated_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, old_admin) = deploy_proxy(&env);
    let new_admin = Address::generate(&env);

    // Rotate ProjectRoot admin
    project_root.propose_admin(&new_admin);
    project_root.accept_admin();
    assert_eq!(project_root.admin(), new_admin);

    // After rotation, an auth scoped to the OLD admin no longer satisfies forward.
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

    // And the NEW admin can forward.
    env.mock_auths(&[MockAuth {
        address: &new_admin,
        invoke: &MockAuthInvoke {
            contract: &project_root.address,
            fn_name: "forward",
            args: (security.address.clone(), fn_name.clone(), args.clone()).into_val(&env),
            sub_invokes: &[MockAuthInvoke {
                // The downstream security contract sees project_root as caller, and
                // since project_root is the direct invoker its own auth is implicit —
                // but the mock auth tree needs the sub-call entry described.
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
    // that also does require_auth. We demonstrate this by having forward
    // call back into project_root.update_project_spec_repo, which requires
    // its own admin auth. Without a sub_invoke for it, the inner call fails.
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

    // Spec repo unchanged.
    assert_eq!(
        project_root.project_spec_repo(),
        String::from_str(&env, "https://github.com/example/spec")
    );
}

#[test]
fn forward_propagates_inner_call_error() {
    // The downstream security contract returns SecurityError::ZeroWeight when
    // weight is zero. That error must surface as the forward call's error.
    let env = Env::default();
    env.mock_all_auths();

    let (project_root, security, _admin) = deploy_proxy(&env);
    let key = secp256k1_pubkey(&env, &make_secp256k1_key(4));
    let args = add_signer_args(&env, &key, 0); // <- zero weight
    let fn_name = Symbol::new(&env, "add_signer");

    let result = project_root.try_forward(&security.address, &fn_name, &args);
    let outer_err = result.expect_err("zero-weight inner call should fail");
    let inner_err: soroban_sdk::Error = outer_err.expect("inner contract error preserved");
    // Inner contract error is preserved through the proxy.
    assert_eq!(inner_err, SecurityError::ZeroWeight.into());
}
