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