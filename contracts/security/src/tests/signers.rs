extern crate std;

use crate::storage::PubKey;

use super::setup::deploy_contract;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, Env, Vec,
};

fn make_signer(env: &Env, seed: u8) -> PubKey {
    // TODO: random generation
    PubKey::from_array(env, &[seed; 33])
}

#[test]
fn test_add_signer_basic_queries() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = deploy_contract(&env);
    let key = make_signer(&env, 1);
    let key2 = make_signer(&env, 2);
    let weight = 45;

    // Nothing to start
    assert_eq!(client.get_total_weight(), 0);
    assert_eq!(client.get_signer_weight(&key), 0);
    assert_eq!(client.list_signers().len(), 0);

    // Add signer
    client.add_signer(&key, &weight);

    // Got values
    assert_eq!(client.get_total_weight(), weight);
    assert_eq!(client.get_signer_weight(&key), weight);
    assert_eq!(client.get_signer_weight(&key2), 0);
    assert_eq!(client.list_signers().len(), 1);
}

#[test]
fn test_add_and_remove_signer() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = deploy_contract(&env);
    let key1 = make_signer(&env, 1);
    let key2 = make_signer(&env, 2);

    client.add_signer(&key1, &30);
    client.add_signer(&key2, &70);

    assert_eq!(client.get_total_weight(), 100);
    assert_eq!(client.get_signer_weight(&key1), 30);
    assert_eq!(client.get_signer_weight(&key2), 70);
    assert_eq!(client.list_signers().len(), 2);

    client.remove_signer(&key1);

    assert_eq!(client.get_total_weight(), 70);
    assert_eq!(client.get_signer_weight(&key1), 0);
    assert_eq!(client.get_signer_weight(&key2), 70);
    assert_eq!(client.list_signers().len(), 1);
}

#[test]
fn test_update_signer_weight() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, _admin) = deploy_contract(&env);
    let key = make_signer(&env, 1);

    client.add_signer(&key, &50);
    assert_eq!(client.get_signer_weight(&key), 50);
    assert_eq!(client.get_total_weight(), 50);

    // Update weight by setting same key again
    client.add_signer(&key, &80);
    assert_eq!(client.get_signer_weight(&key), 80);
    assert_eq!(client.get_total_weight(), 80);
    assert_eq!(client.list_signers().len(), 1);
}



#[test]
fn test_assert_admin_unauthorized() {
    let env = Env::default();

    let (client, _admin) = deploy_contract(&env);
    let non_admin = Address::generate(&env);
    let key = make_signer(&env, 4);
    let weight = 102;

    env.mock_auths(&[MockAuth {
        address: &non_admin,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "add_signer",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    assert_eq!(
        client.try_add_signer(&key, &weight),
        Err(Ok(soroban_sdk::Error::from_type_and_code(
            soroban_sdk::xdr::ScErrorType::Context,
            soroban_sdk::xdr::ScErrorCode::InvalidAction,
        )))
    );
}

#[test]
fn test_assert_admin_auth() {
    let env = Env::default();

    let (client, admin) = deploy_contract(&env);
    let key = make_signer(&env, 4);
    let weight = 102;

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "add_signer",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    client.add_signer(&key, &weight);
    assert_eq!(client.get_total_weight(), weight);
}
