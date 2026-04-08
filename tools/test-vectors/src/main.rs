//! Generates test vectors for testnet smoke tests.
//!
//! Outputs shell-sourceable variables: pubkeys, ABI-encoded envelopes, signatures.
//! Usage: eval $(cargo run -p test-vectors)

use alloy_primitives::FixedBytes;
use alloy_sol_types::{SolValue, sol};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

sol! {
    struct Envelope {
        bytes20 eventId;
        bytes12 ordering;
        bytes payload;
    }
}

/// Deterministically generate a secp256k1 signing key from a seed byte.
fn make_signing_key(seed: u8) -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    SigningKey::from_bytes(&secret.into()).unwrap()
}

/// Derive the compressed public key (33 bytes) from a signing key.
fn compressed_pubkey(key: &SigningKey) -> [u8; 33] {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes(); // compressed by default
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    arr
}

/// Sign an envelope the same way the contract's `is_valid_signature` expects:
/// digest = keccak256("\x19Ethereum Signed Message:\n32" || keccak256(envelope))
/// Returns r(32) || s(32) || v(1) with Ethereum-style v (27/28).
fn sign_envelope(key: &SigningKey, envelope: &[u8]) -> [u8; 65] {
    let inner_hash = Keccak256::digest(envelope);

    let mut prefixed = Vec::new();
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

fn make_envelope(event_id_seed: u8) -> (Vec<u8>, [u8; 20]) {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = Envelope {
        eventId: FixedBytes(event_id),
        ordering: FixedBytes([0u8; 12]),
        payload: vec![event_id_seed; 8].into(),
    };

    (envelope.abi_encode(), event_id)
}

fn main() {
    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);
    let key3 = make_signing_key(3);

    let pk1 = compressed_pubkey(&key1);
    let pk2 = compressed_pubkey(&key2);
    let pk3 = compressed_pubkey(&key3);

    // Sort signers by ascending pubkey bytes (required by verify)
    let mut signers: Vec<(u8, [u8; 33], &SigningKey)> =
        vec![(1, pk1, &key1), (2, pk2, &key2), (3, pk3, &key3)];
    signers.sort_by(|a, b| a.1.cmp(&b.1));

    // Generate 3 envelopes with different event IDs for different test scenarios
    let (envelope1, event_id1) = make_envelope(0x01);
    let (envelope2, event_id2) = make_envelope(0x02);
    let (envelope3, event_id3) = make_envelope(0x03);

    // Sign each envelope with all 3 keys (in sorted order)
    let sigs1: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| sign_envelope(key, &envelope1))
        .collect();
    let sigs2: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| sign_envelope(key, &envelope2))
        .collect();
    let sigs3: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| sign_envelope(key, &envelope3))
        .collect();

    // Output shell variables
    // Pubkeys in sorted order
    for (i, (seed, pk, _)) in signers.iter().enumerate() {
        let idx = i + 1;
        println!("SIGNER{}_SEED={}", idx, seed);
        println!("SIGNER{}_PUBKEY={}", idx, hex::encode(pk));
    }

    println!();

    // Envelope 1: for happy path test (2-of-3)
    println!("ENVELOPE1={}", hex::encode(&envelope1));
    println!("EVENT_ID1={}", hex::encode(event_id1));
    for (i, sig) in sigs1.iter().enumerate() {
        println!("ENVELOPE1_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();

    // Envelope 2: for insufficient weight test (1-of-3)
    println!("ENVELOPE2={}", hex::encode(&envelope2));
    println!("EVENT_ID2={}", hex::encode(event_id2));
    for (i, sig) in sigs2.iter().enumerate() {
        println!("ENVELOPE2_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();

    // Envelope 3: for invalid signature test
    println!("ENVELOPE3={}", hex::encode(&envelope3));
    println!("EVENT_ID3={}", hex::encode(event_id3));
    for (i, sig) in sigs3.iter().enumerate() {
        println!("ENVELOPE3_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();

    // Invalid signature (garbage bytes)
    println!("INVALID_SIG={}", hex::encode([0xAAu8; 65]));
}
