extern crate std;

use super::setup::deploy_contract;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    Address, Env, Vec,
};

#[test]
fn test_admin_increment() {
    let env = Env::default();
    env.mock_all_auths();

    let (client, admin) = deploy_contract(&env);
    assert_eq!(client.admin(), admin);
    assert_eq!(client.count(), 0u64);

    client.increment();
    assert_eq!(client.count(), 1u64);
}

#[test]
fn test_assert_admin_auth() {
    let env = Env::default();

    let (client, admin) = deploy_contract(&env);

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "increment",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    client.increment();
    assert_eq!(client.count(), 1u64);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_assert_admin_unauthorized() {
    let env = Env::default();

    let (client, _admin) = deploy_contract(&env);
    let non_admin = Address::generate(&env);

    env.mock_auths(&[MockAuth {
        address: &non_admin,
        invoke: &MockAuthInvoke {
            contract: &client.address,
            fn_name: "increment",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    client.increment();
}
