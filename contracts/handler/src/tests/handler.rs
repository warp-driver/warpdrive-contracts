extern crate alloc;
extern crate std;

use crate::envelope::Envelope;
use crate::{Handler, HandlerClient, HandlerError, SignatureData};
use alloy_primitives::FixedBytes;
use alloy_sol_types::SolValue;
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _};
use warpdrive_security::{Security, SecurityClient};
use warpdrive_verification::Verification;

type PubKey = BytesN<33>;

fn make_signing_key(seed: u8) -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    SigningKey::from_bytes(&secret.into()).unwrap()
}

fn compressed_pubkey(env: &Env, key: &SigningKey) -> PubKey {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes();
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    PubKey::from_array(env, &arr)
}

fn sign_envelope(key: &SigningKey, envelope: &[u8]) -> [u8; 65] {
    let inner_hash = Keccak256::digest(envelope);
    let mut prefixed = std::vec::Vec::new();
    prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
    prefixed.extend_from_slice(&inner_hash);
    let digest = Keccak256::digest(&prefixed);
    let (sig, recid) = key
        .sign_prehash_recoverable(&digest)
        .expect("signing failed");
    let mut result = [0u8; 65];
    result[..64].copy_from_slice(&sig.to_bytes());
    result[64] = recid.to_byte() + 27;
    result
}

fn make_envelope_bytes(env: &Env, event_id_seed: u8) -> Bytes {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = Envelope {
        eventId: FixedBytes(event_id),
        ordering: FixedBytes([0u8; 12]),
        payload: alloc::vec![].into(),
    };

    let encoded = envelope.abi_encode();
    Bytes::from_slice(env, &encoded)
}

/// Returns (handler_client, key1, pubkey1, key2, pubkey2) with key1 and key2
/// registered as signers (weight 100 and 200, threshold 55%).
/// Required weight = (100+200)*55/100 = 165.
fn setup_handler_with_signers(
    env: &Env,
) -> (HandlerClient<'_>, SigningKey, PubKey, SigningKey, PubKey) {
    let admin = soroban_sdk::Address::generate(env);

    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);
    let pk1 = compressed_pubkey(env, &key1);
    let pk2 = compressed_pubkey(env, &key2);

    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(env, &security_id);
    security.mock_all_auths().add_signer(&pk1, &100);
    security.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(env, &handler_id);

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
        reference_block: 0,
    }
}

// ── Happy path ──────────────────────────────────────────────────────

#[test]
fn test_verify_success() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes(&env, 1);
    // key2 has weight 200 >= required 165
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_verify_success_combined_weight() {
    let env = Env::default();
    let (client, key1, pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1), (key2, pk2)]);

    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));
}

// ── Duplicate event ─────────────────────────────────────────────────

#[test]
fn test_verify_duplicate_event_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes(&env, 1);
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key2, pk2)]);

    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    // Same event_id again
    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn test_verify_different_events_succeed() {
    let env = Env::default();
    let (client, _key1, _pk1, key2, pk2) = setup_handler_with_signers(&env);

    let env1 = make_envelope_bytes(&env, 1);
    let env2 = make_envelope_bytes(&env, 2);
    let sig1 = make_sig_data(&env, &env1.to_alloc_vec(), &[(key2.clone(), pk2.clone())]);
    let sig2 = make_sig_data(&env, &env2.to_alloc_vec(), &[(key2, pk2)]);

    assert_eq!(client.try_verify(&env1, &sig1), Ok(Ok(())));
    assert_eq!(
        client.try_verify(&env1, &sig1),
        Err(Ok(HandlerError::EventAlreadySeen))
    );
    assert_eq!(client.try_verify(&env2, &sig2), Ok(Ok(())));
}

// ── Verification errors propagate from verification contract ────────

#[test]
fn test_verify_invalid_signature_fails() {
    let env = Env::default();
    let (client, _key1, _pk1, _key2, pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes(&env, 1);

    let mut signers: Vec<PubKey> = Vec::new(&env);
    signers.push_back(pk2);
    let mut signatures: Vec<BytesN<65>> = Vec::new(&env);
    signatures.push_back(BytesN::from_array(&env, &[0xAA; 65]));

    let sig_data = SignatureData {
        signers,
        signatures,
        reference_block: 0,
    };

    assert_eq!(
        client.try_verify(&envelope, &sig_data),
        Err(Ok(HandlerError::InvalidSignature)),
    );
}

#[test]
fn test_verify_insufficient_weight_fails() {
    let env = Env::default();
    let (client, key1, pk1, _key2, _pk2) = setup_handler_with_signers(&env);

    let envelope = make_envelope_bytes(&env, 1);
    // key1 has weight 100 < required 165
    let sig_data = make_sig_data(&env, &envelope.to_alloc_vec(), &[(key1, pk1)]);

    assert_eq!(
        client.try_verify(&envelope, &sig_data),
        Err(Ok(HandlerError::InsufficientWeight)),
    );
}
