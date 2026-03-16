extern crate std;

use crate::{Verification, VerificationClient};
use soroban_sdk::{BytesN, Env, testutils::Address as _};
use warpdrive_security::{Security, SecurityClient};

type PubKey = BytesN<33>;

fn make_signer(env: &Env, seed: u8) -> PubKey {
    PubKey::from_array(env, &[seed; 33])
}

fn setup_contracts(env: &Env) -> (VerificationClient<'_>, SecurityClient<'_>) {
    let admin = soroban_sdk::Address::generate(env);

    // Deploy security contract with threshold 55/100
    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(env, &security_id);

    // Add two signers: key1 with weight 100, key2 with weight 200
    security
        .mock_all_auths()
        .add_signer(&make_signer(env, 1), &100);
    security
        .mock_all_auths()
        .add_signer(&make_signer(env, 2), &200);

    // Deploy verification contract referencing the security contract
    let verification_id = env.register(Verification, (&admin, &security_id));
    let verification = VerificationClient::new(env, &verification_id);

    (verification, security)
}

#[test]
fn test_required_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    // total_weight = 100 + 200 = 300
    // required = 300 * 55 / 100 = 165
    assert_eq!(verification.required_weight(), 165);
}

#[test]
fn test_signer_weight_existing() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    assert_eq!(verification.signer_weight(&make_signer(&env, 1)), 100);
    assert_eq!(verification.signer_weight(&make_signer(&env, 2)), 200);
}

#[test]
fn test_signer_weight_missing() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    // Key 3 was never registered
    assert_eq!(verification.signer_weight(&make_signer(&env, 3)), 0);
}
