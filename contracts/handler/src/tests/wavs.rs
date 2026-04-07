// This file tests compatibility with WAVS, using code from github.com/Lay3rLabs/WAVS to generate a test vector.
// In particular, this is slightly adapted from the test case at  packages/utils/src/evm_client/signing.rs:196-259

use crate::{Handler, HandlerClient, HandlerError, SignatureData};
use hex_literal::hex;
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _, testutils::Ledger as _};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_secp256k1_verification::Secp256k1Verification;

/*
From `cargo test evm_client::signing::test::envelope_test_vector` in wavs/packages/utils:

Envelope: 000000000000000000000000000000000000000000000000000000000000002001010101010101010101010101010101010101010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000030102030000000000000000000000000000000000000000000000000000000000
Signature: 96A68226A31FDF9D1339D27E705A9A231A580F14736C005714A674425649BA371D00172C79D29B6E0C79F13F408023E808A12431469D5FD13CC417AEB90425AF1C
PubKey: 0232298EDEBA83F60F31CAB61995A240D88E5C7604B9F70D09EEE95331CB444599
Event ID: 0101010101010101010101010101010101010101
Ordering: 000000000000000000000000
Payload: 010203
*/

// Different runs with random private keys
mod ex1 {
    use super::hex;
    pub const SIGNATURE: [u8; 65] = hex!(
        "96A68226A31FDF9D1339D27E705A9A231A580F14736C005714A674425649BA371D00172C79D29B6E0C79F13F408023E808A12431469D5FD13CC417AEB90425AF1C"
    );
    pub const PUB_KEY: [u8; 33] =
        hex!("0232298EDEBA83F60F31CAB61995A240D88E5C7604B9F70D09EEE95331CB444599");
}

mod ex2 {
    use super::hex;
    pub const SIGNATURE: [u8; 65] = hex!(
        "F723DCDDAD3B48D93BEA69179F6114A3C8E0FA97218948094CFA7C69F5D7DB781C219129D3149102C4348FD0799EC721AEEAC73459FDEE2A0EC695A8C4B320311B"
    );
    pub const PUB_KEY: [u8; 33] =
        hex!("03ADA8BB4E3F7CF5AB52BBEEF53BA9BAD2E7EB7B49FA7D37E5E3ACF226B1211D60");
}

// Same for all test cases
pub const ENVELOPE: [u8; 192] = hex!(
    "000000000000000000000000000000000000000000000000000000000000002001010101010101010101010101010101010101010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000030102030000000000000000000000000000000000000000000000000000000000"
);
const EVENT_ID: [u8; 20] = hex!("0101010101010101010101010101010101010101");
// const ORDERING: [u8; 12] = hex!("000000000000000000000000");
const PAYLOAD: [u8; 3] = hex!("010203");

const TEST_REF_BLOCK: u32 = 10;

type PubKey = BytesN<33>;

fn setup_handler_with_signer<'a>(env: &'a Env, pubkey: &[u8; 33]) -> HandlerClient<'a> {
    let admin = soroban_sdk::Address::generate(env);

    let pk = PubKey::from_array(env, pubkey);

    env.ledger().set_sequence_number(TEST_REF_BLOCK);

    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(env, &security_id);
    security.mock_all_auths().add_signer(&pk, &100);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));

    env.ledger().set_sequence_number(100);

    HandlerClient::new(env, &handler_id)
}

#[test]
fn test_verify_success_ex1() {
    let env = Env::default();
    let client = setup_handler_with_signer(&env, &ex1::PUB_KEY);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_front(PubKey::from_array(&env, &ex1::PUB_KEY));

    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_front(BytesN::from_array(&env, &ex1::SIGNATURE));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    let result = client.try_verify_eth(&Bytes::from_array(&env, &ENVELOPE), &sig_data);
    assert_eq!(result, Ok(Ok(())));

    let event_id = BytesN::from_array(&env, &EVENT_ID);
    let payload = Bytes::from_array(&env, &PAYLOAD);
    assert_eq!(client.payload(&event_id), Some(payload));
}

#[test]
fn test_verify_success_ex2() {
    let env = Env::default();
    let client = setup_handler_with_signer(&env, &ex2::PUB_KEY);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_front(PubKey::from_array(&env, &ex2::PUB_KEY));

    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_front(BytesN::from_array(&env, &ex2::SIGNATURE));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    let result = client.try_verify_eth(&Bytes::from_array(&env, &ENVELOPE), &sig_data);
    assert_eq!(result, Ok(Ok(())));

    let event_id = BytesN::from_array(&env, &EVENT_ID);
    let payload = Bytes::from_array(&env, &PAYLOAD);
    assert_eq!(client.payload(&event_id), Some(payload));
}

#[test]
fn test_fail_wrong_signer() {
    let env = Env::default();
    let client = setup_handler_with_signer(&env, &ex1::PUB_KEY);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_front(PubKey::from_array(&env, &ex1::PUB_KEY));

    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_front(BytesN::from_array(&env, &ex2::SIGNATURE));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    let result = client.try_verify_eth(&Bytes::from_array(&env, &ENVELOPE), &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::InvalidSignature)));
}
