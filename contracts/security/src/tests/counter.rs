extern crate std;

use super::setup::deploy_contract;
use soroban_sdk::Env;

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
    env.mock_all_auths();

    let (client, admin) = deploy_contract(&env);
    assert_eq!(client.admin(), admin);
    assert_eq!(client.count(), 0u64);

    // How do I set it up so this is signed by a different user, and thus fails (test only admin can do this)?
    client.increment();
    // assert_eq!(client.count(), 0u64);
}
