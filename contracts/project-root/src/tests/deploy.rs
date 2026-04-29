extern crate std;

use super::setup::{deploy_contract, install_contract_wasm};
use soroban_sdk::testutils::Events as _;
use soroban_sdk::{Env, IntoVal, Map, String, Symbol, Val, vec};

#[test]
fn test_deploy() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = deploy_contract(&env);

    assert_eq!(client.admin(), admin);
    assert_eq!(
        client.version(),
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(
        client.project_spec_repo(),
        String::from_str(&env, "https://github.com/example/spec")
    );
    // security_contract and verification_contract are set
    let _ = client.security_contract();
    let _ = client.verification_contract();
}

#[test]
fn test_upgrade() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let (client, admin) = deploy_contract(&env);
    assert_eq!(
        client.version(),
        String::from_str(&env, env!("CARGO_PKG_VERSION"))
    );

    let new_wasm_hash = install_contract_wasm(&env);
    client.upgrade(&new_wasm_hash, &String::from_str(&env, "0.0.2"));

    assert_eq!(client.version(), String::from_str(&env, "0.0.2"));
    assert_eq!(client.admin(), admin);
}

#[test]
fn test_update_project_spec_repo() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = deploy_contract(&env);

    assert_eq!(
        client.project_spec_repo(),
        String::from_str(&env, "https://github.com/example/spec")
    );

    let new_repo = String::from_str(&env, "https://github.com/example/spec-v2");
    client.update_project_spec_repo(&new_repo);

    let got = env.events().all();
    assert_eq!(
        got,
        vec![
            &env,
            (
                client.address.clone(),
                (Symbol::new(&env, "updated_spec_repo"),).into_val(&env),
                Map::<Symbol, Val>::from_array(
                    &env,
                    [(Symbol::new(&env, "repo"), new_repo.clone().into_val(&env))]
                )
                .into_val(&env),
            ),
        ]
    );

    assert_eq!(client.project_spec_repo(), new_repo);
}

#[test]
fn test_admin_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = deploy_contract(&env);
    assert_eq!(client.admin(), admin);
    assert!(client.pending_admin().is_none());

    use soroban_sdk::testutils::Address as _;
    let new_admin = soroban_sdk::Address::generate(&env);
    client.propose_admin(&new_admin);
    assert_eq!(client.pending_admin(), Some(new_admin.clone()));

    client.accept_admin();
    assert_eq!(client.admin(), new_admin);
    assert!(client.pending_admin().is_none());
}
