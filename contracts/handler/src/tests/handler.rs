extern crate alloc;
extern crate std;

use crate::contract::XlmEnvelope;
use crate::envelope::Envelope;
use crate::{Handler, HandlerClient, HandlerError, SignatureData};
use alloy_primitives::FixedBytes;
use alloy_sol_types::SolValue;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _, testutils::Ledger as _};
use warpdrive_security::{Security, SecurityClient};
use warpdrive_shared::testutils::{
    PubKey, SigningKey, compressed_pubkey, make_signing_key, sign_envelope,
};
use warpdrive_verification::Verification;

/// Reference block used by default in tests — signers are registered at this ledger.
const TEST_REF_BLOCK: u32 = 10;
/// Current ledger sequence in tests — must be > TEST_REF_BLOCK.
const TEST_CURRENT_LEDGER: u32 = 100;

fn make_envelope_bytes_eth(env: &Env, event_id_seed: u8) -> Bytes {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = Envelope {
        eventId: FixedBytes(event_id),
        ordering: FixedBytes([0u8; 12]),
        payload: alloc::vec![event_id_seed; 8].into(),
    };

    let encoded = envelope.abi_encode();
    Bytes::from_slice(env, &encoded)
}

fn make_envelope_bytes_xlm(env: &Env, event_id_seed: u8) -> Bytes {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = XlmEnvelope {
        event_id: BytesN::from_array(env, &event_id),
        ordering: BytesN::from_array(env, &[0u8; 12]),
        payload: Bytes::from_slice(env, &[event_id_seed; 8]),
    };

    envelope.to_xdr(env)
}

fn expected_event_id(env: &Env, seed: u8) -> BytesN<20> {
    let mut id = [0u8; 20];
    id[0] = seed;
    BytesN::from_array(env, &id)
}

fn expected_payload(env: &Env, seed: u8) -> Bytes {
    Bytes::from_slice(env, &[seed; 8])
}

/// Returns (handler_client, key1, pubkey1, key2, pubkey2) with key1 and key2
/// registered as signers (weight 100 and 200, threshold 55%).
/// Required weight = (100+200)*55/100 = 165.
/// Signers are registered at ledger TEST_REF_BLOCK, current ledger is TEST_CURRENT_LEDGER.
fn setup_handler_with_signers(
    env: &Env,
) -> (HandlerClient<'_>, SigningKey, PubKey, SigningKey, PubKey) {
    let admin = soroban_sdk::Address::generate(env);

    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);
    let pk1 = compressed_pubkey(env, &key1);
    let pk2 = compressed_pubkey(env, &key2);

    // Register signers at TEST_REF_BLOCK so checkpoints are recorded there
    env.ledger().set_sequence_number(TEST_REF_BLOCK);

    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(env, &handler_id);

    // Advance ledger so reference_block is in the past
    env.ledger().set_sequence_number(TEST_CURRENT_LEDGER);

    (client, key1, pk1, key2, pk2)
}

/// Build a SignatureData with signers ordered by ascending pubkey bytes.
fn make_sig_data(
    env: &Env,
    envelope_raw: &[u8],
    keys_and_pubs: &[(SigningKey, PubKey)],
) -> SignatureData {
    let mut sorted: std::vec::Vec<_> = keys_and_pubs.to_vec();
    sorted.sort_by(|a, b| a.1.to_array().cmp(&b.1.to_array()));

    let mut signers: Vec<PubKey> = Vec::new(env);
    let mut signatures: Vec<BytesN<65>> = Vec::new(env);

    for (key, pubkey) in &sorted {
        signers.push_back(pubkey.clone());
        let sig_bytes = sign_envelope(key, envelope_raw);
        signatures.push_back(BytesN::from_array(env, &sig_bytes));
    }

    SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    }
}

/********************** ETH VARIANT *******************/

// ── Happy path ──────────────────────────────────────────────────────

#[test]
fn test_verify_success() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify_eth(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

#[test]
fn test_verify_success_combined_weight() {
    let env = Env::default();
    let (client, key1, pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1), (key2, pk2)]);

    let result = client.try_verify_eth(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

// ── Duplicate event ─────────────────────────────────────────────────

#[test]
fn test_verify_duplicate_event_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify_eth(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );

    let result = client.try_verify_eth(&envelope, &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn test_verify_different_events_succeed() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let env1 = make_envelope_bytes_eth(&env, 1);
    let env2 = make_envelope_bytes_eth(&env, 2);
    let sig1 = make_sig_data(&env, &env1.to_alloc_vec(), &[(key2.clone(), pk2.clone())]);
    let sig2 = make_sig_data(&env, &env2.to_alloc_vec(), &[(key2, pk2)]);

    assert_eq!(client.try_verify_eth(&env1, &sig1), Ok(Ok(())));
    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );

    assert_eq!(
        client.try_verify_eth(&env1, &sig1),
        Err(Ok(HandlerError::EventAlreadySeen))
    );
    assert_eq!(client.try_verify_eth(&env2, &sig2), Ok(Ok(())));
    assert_eq!(
        client.payload(&expected_event_id(&env, 2)),
        Some(expected_payload(&env, 2))
    );
}

// ── Verification errors propagate from verification contract ────────

#[test]
fn test_verify_invalid_signature_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, _key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &[0xAA; 65]));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidSignature)),
    );

    assert_eq!(client.payload(&expected_event_id(&env, 1)), None);
}

#[test]
fn test_verify_insufficient_weight_fails() {
    let env = Env::default();
    let (client, key1, pk1, _key2, _pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1)]);

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InsufficientWeight)),
    );

    assert_eq!(client.payload(&expected_event_id(&env, 1)), None);
}

/********************** XLM VARIANT *******************/

#[test]
fn test_verify_success_xlm() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify_xlm(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

#[test]
fn test_verify_success_combined_weight_xlm() {
    let env = Env::default();
    let (client, key1, pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1), (key2, pk2)]);

    let result = client.try_verify_xlm(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

#[test]
fn test_verify_duplicate_event_fails_xlm() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify_xlm(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );

    let result = client.try_verify_xlm(&envelope, &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn test_verify_different_events_succeed_xlm() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let env1 = make_envelope_bytes_xlm(&env, 1);
    let env2 = make_envelope_bytes_xlm(&env, 2);
    let sig1 = make_sig_data(&env, &env1.to_alloc_vec(), &[(key2.clone(), pk2.clone())]);
    let sig2 = make_sig_data(&env, &env2.to_alloc_vec(), &[(key2, pk2)]);

    assert_eq!(client.try_verify_xlm(&env1, &sig1), Ok(Ok(())));
    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );

    assert_eq!(
        client.try_verify_xlm(&env1, &sig1),
        Err(Ok(HandlerError::EventAlreadySeen))
    );
    assert_eq!(client.try_verify_xlm(&env2, &sig2), Ok(Ok(())));
    assert_eq!(
        client.payload(&expected_event_id(&env, 2)),
        Some(expected_payload(&env, 2))
    );
}

#[test]
fn test_verify_invalid_signature_fails_xlm() {
    let env = Env::default();
    let (client, _key1, _pk1, _key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &[0xAA; 65]));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_xlm(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidSignature)),
    );

    assert_eq!(client.payload(&expected_event_id(&env, 1)), None);
}

#[test]
fn test_verify_insufficient_weight_fails_xlm() {
    let env = Env::default();
    let (client, key1, pk1, _key2, _pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1)]);

    assert_eq!(
        client.try_verify_xlm(&envelope, &sig_data),
        Err(Ok(HandlerError::InsufficientWeight)),
    );

    assert_eq!(client.payload(&expected_event_id(&env, 1)), None);
}

/********************* XLM vs ETH **************************/

#[test]
fn test_eth_refuses_xlm_packets() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_xlm(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    // Must fail with InvalidEnvelope — XLM data is not valid ABI
    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidEnvelope)),
    );

    // Must pass
    let result = client.try_verify_xlm(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

#[test]
fn test_xlm_refuses_eth_packets() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    // Must fail — ETH data is not valid XDR (from_xdr panics at host level)
    assert!(client.try_verify_xlm(&envelope, &sig_data).is_err());

    // Must pass
    let result = client.try_verify_eth(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    assert_eq!(
        client.payload(&expected_event_id(&env, 1)),
        Some(expected_payload(&env, 1))
    );
}

// ── M-3: Malformed envelope tests ───────────────────────────────────

#[test]
fn test_verify_eth_malformed_envelope() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let garbage = Bytes::from_slice(&env, &[0xDE, 0xAD, 0xBE, 0xEF]);
    let sig_data = make_sig_data(&env, &garbage.to_alloc_vec(), &[(key2, pk2)]);

    assert_eq!(
        client.try_verify_eth(&garbage, &sig_data),
        Err(Ok(HandlerError::InvalidEnvelope)),
    );
}

#[test]
fn test_verify_xlm_malformed_envelope() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let garbage = Bytes::from_slice(&env, &[0xDE, 0xAD, 0xBE, 0xEF]);
    let sig_data = make_sig_data(&env, &garbage.to_alloc_vec(), &[(key2, pk2)]);

    // from_xdr on garbage bytes fails
    assert!(client.try_verify_xlm(&garbage, &sig_data).is_err());
}

// ── T-7: Remaining error propagation paths ──────────────────────────

#[test]
fn test_verify_empty_signatures_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, _key2, _pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);

    let sig_data = SignatureData {
        signers: Vec::new(&env),
        signatures: Vec::new(&env),
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::EmptySignatures)),
    );
}

#[test]
fn test_verify_length_mismatch_fails() {
    let env = Env::default();
    let (client, key1, pk1, _key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig_bytes = sign_envelope(&key1, &raw);

    // One signature, two pubkeys
    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk1);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig_bytes));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::LengthMismatch)),
    );
}

#[test]
fn test_verify_signers_not_ordered_fails() {
    let env = Env::default();
    let (client, key1, pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig1 = sign_envelope(&key1, &raw);
    let sig2 = sign_envelope(&key2, &raw);

    // Determine correct order, then reverse it
    let (lo_pk, lo_sig, hi_pk, hi_sig) = if pk1.to_array() < pk2.to_array() {
        (pk1, sig1, pk2, sig2)
    } else {
        (pk2, sig2, pk1, sig1)
    };

    // Provide in descending order (wrong)
    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(hi_pk);
    signers.push_back(lo_pk);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &hi_sig));
    signatures.push_back(BytesN::from_array(&env, &lo_sig));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::SignersNotOrdered)),
    );
}

#[test]
fn test_verify_signer_not_registered_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, _key2, _pk2) = setup_handler_with_signers(&env);

    let key3 = make_signing_key(3);
    let pk3 = compressed_pubkey(&env, &key3);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig3 = sign_envelope(&key3, &raw);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk3);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig3));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::SignerNotRegistered)),
    );
}

// ── T-8: verification_contract getter ───────────────────────────────

#[test]
fn test_verification_contract_getter() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    env.ledger().set_sequence_number(TEST_REF_BLOCK);
    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(&env, &handler_id);

    assert_eq!(client.verification_contract(), verification_id);
}

/********************* REFERENCE BLOCK VALIDATION **************************/

#[test]
fn test_verify_reference_block_in_future_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig_bytes = sign_envelope(&key2, &raw);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig_bytes));

    // reference_block == current ledger (not strictly in the past)
    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_CURRENT_LEDGER,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidReferenceBlock)),
    );
}

#[test]
fn test_verify_reference_block_too_old_fails() {
    let env = Env::default();

    // Set up at ledger 10, advance to 300 (more than 200 blocks past reference)
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);
    env.ledger().set_sequence_number(300);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig_bytes = sign_envelope(&key2, &raw);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig_bytes));

    // reference_block = 10, current = 300, age = 290 > 200
    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: TEST_REF_BLOCK,
    };

    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidReferenceBlock)),
    );
}

/********************* HISTORICAL CHECKPOINT TESTS **************************/

#[test]
fn test_verify_historical_passes_current_fails() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key2 = make_signing_key(2);
    let pk2 = compressed_pubkey(&env, &key2);
    let key1 = make_signing_key(1);
    let pk1 = compressed_pubkey(&env, &key1);

    // Ledger 10: key1=100, key2=200, total=300, required=165
    env.ledger().set_sequence_number(10);
    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(&env, &handler_id);

    // Ledger 50: reduce key2 to 50 → total=150, required=82, key2 alone=50 < 82
    env.ledger().set_sequence_number(50);
    security.mock_all_auths().add_signer(&pk2, &50);

    // Advance to 100 for handler calls
    env.ledger().set_sequence_number(100);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig_bytes = sign_envelope(&key2, &raw);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig_bytes));

    // reference_block=10: key2 had 200, total 300, required 165 → passes
    let sig_data = SignatureData {
        signers: signers.clone(),
        signatures: signatures.clone(),
        reference_block: 10,
    };
    assert_eq!(client.try_verify_eth(&envelope, &sig_data), Ok(Ok(())));

    // reference_block=50: key2 has 50, total 150, required 82 → fails
    let envelope2 = make_envelope_bytes_eth(&env, 2);
    let raw2 = envelope2.to_alloc_vec();
    let sig_bytes2 = sign_envelope(&key2, &raw2);
    let mut signatures2: Vec<BytesN<65>> = Vec::new(&env);
    signatures2.push_back(BytesN::from_array(&env, &sig_bytes2));

    let sig_data2 = SignatureData {
        signers,
        signatures: signatures2,
        reference_block: 50,
    };
    assert_eq!(
        client.try_verify_eth(&envelope2, &sig_data2),
        Err(Ok(HandlerError::InsufficientWeight)),
    );
}

#[test]
fn test_verify_historical_fails_current_passes() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_signing_key(1);
    let pk1 = compressed_pubkey(&env, &key1);
    let key2 = make_signing_key(2);
    let pk2 = compressed_pubkey(&env, &key2);

    // Ledger 10: key1=50, key2=200, total=250, required=137, key1 alone=50 < 137
    env.ledger().set_sequence_number(10);
    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(&env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &50);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(&env, &handler_id);

    // Ledger 50: key1=200, remove key2 → total=200, required=110, key1 alone=200 >= 110
    env.ledger().set_sequence_number(50);
    security.mock_all_auths().add_signer(&pk1, &200);
    security.mock_all_auths().remove_signer(&pk2);

    // Advance to 100
    env.ledger().set_sequence_number(100);

    let envelope = make_envelope_bytes_eth(&env, 1);
    let raw = envelope.to_alloc_vec();
    let sig_bytes = sign_envelope(&key1, &raw);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk1);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &sig_bytes));

    // reference_block=10: key1 had 50, total 250, required 137 → fails
    let sig_data = SignatureData {
        signers: signers.clone(),
        signatures: signatures.clone(),
        reference_block: 10,
    };
    assert_eq!(
        client.try_verify_eth(&envelope, &sig_data),
        Err(Ok(HandlerError::InsufficientWeight)),
    );

    // reference_block=50: key1 has 200, total 200, required 110 → passes
    let envelope2 = make_envelope_bytes_eth(&env, 2);
    let raw2 = envelope2.to_alloc_vec();
    let sig_bytes2 = sign_envelope(&key1, &raw2);
    let mut signatures2: Vec<BytesN<65>> = Vec::new(&env);
    signatures2.push_back(BytesN::from_array(&env, &sig_bytes2));

    let sig_data2 = SignatureData {
        signers,
        signatures: signatures2,
        reference_block: 50,
    };
    assert_eq!(client.try_verify_eth(&envelope2, &sig_data2), Ok(Ok(())));
}
