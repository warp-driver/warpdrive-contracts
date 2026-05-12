// External-vector compatibility tests. Vectors are produced by
// `cargo run -p test-vectors` (see tools/test-vectors). Regenerate from there
// if the on-wire format (Envelope or DataWithId) changes.
//
// The envelope embeds a DataWithId-wrapped payload (triggerId = 1001,
// data = [0x01; 8]). Signatures are EIP-191 over keccak256(envelope_bytes).

use crate::envelope::DataWithId;
use crate::{EthereumHandler, EthereumHandlerClient, HandlerError, SignatureData};
use alloy_sol_types::SolValue;
use hex_literal::hex;
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _, testutils::Ledger as _};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_secp256k1_verification::Secp256k1Verification;

mod ex1 {
    use super::hex;
    pub const PUB_KEY: [u8; 33] =
        hex!("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798");
    pub const SIGNATURE: [u8; 65] = hex!(
        "4634d3a3884bd61b098b84f871530aadda0396f8b28a97bc44c30d102802f02a1fca785b5392a53b0b5e81de22e734c0b2756b44c3ff7a72c748b46014d1c20c1b"
    );
}

mod ex2 {
    use super::hex;
    pub const PUB_KEY: [u8; 33] =
        hex!("02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5");
    pub const SIGNATURE: [u8; 65] = hex!(
        "ad91e75bd83950a7ad0637a3aa4d0dd7a2a2a5cb09981df0cb0473d4fc4ca35d1896a523f759aa83f6351f2a334a3d044dbc81e41d5aadb34bd93974933b41cd1b"
    );
}

pub const ENVELOPE: [u8; 320] = hex!(
    "000000000000000000000000000000000000000000000000000000000000002001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000003e9000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000080101010101010101000000000000000000000000000000000000000000000000"
);
const EVENT_ID: [u8; 20] = hex!("0100000000000000000000000000000000000000");
const TRIGGER_ID: u64 = 1001;
const DATA: [u8; 8] = [0x01; 8];

const TEST_REF_BLOCK: u32 = 10;

type PubKey = BytesN<33>;

fn expected_payload(env: &Env) -> Bytes {
    let inner = DataWithId {
        triggerId: TRIGGER_ID,
        data: DATA.to_vec().into(),
    };
    Bytes::from_slice(env, &inner.abi_encode())
}

fn setup_handler_with_signer<'a>(env: &'a Env, pubkey: &[u8; 33]) -> EthereumHandlerClient<'a> {
    let admin = soroban_sdk::Address::generate(env);

    let pk = PubKey::from_array(env, pubkey);

    env.ledger().set_sequence_number(TEST_REF_BLOCK);

    let security_id = env.register(Secp256k1Security, (&admin, 55u64, 100u64));
    let security = Secp256k1SecurityClient::new(env, &security_id);
    security.mock_all_auths().add_signer(&pk, &100);

    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let handler_id = env.register(EthereumHandler, (&admin, &verification_id));

    env.ledger().set_sequence_number(100);

    EthereumHandlerClient::new(env, &handler_id)
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
    assert_eq!(client.payload(&event_id), Some(expected_payload(&env)));
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
    assert_eq!(client.payload(&event_id), Some(expected_payload(&env)));
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
