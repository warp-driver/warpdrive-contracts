//! Generates test vectors for testnet smoke tests.
//!
//! Outputs shell-sourceable variables for both secp256k1 (Ethereum) and ed25519 (Stellar) flows.
//! Usage: eval $(cargo run -p test-vectors)

use alloy_primitives::FixedBytes;
use alloy_sol_types::{SolValue, sol};
use ed25519_dalek::Signer;
use k256::ecdsa::SigningKey;
use sha2::Sha256;
use sha3::{Digest, Keccak256};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Bytes, BytesN, Env};
use warpdrive_shared::interfaces::handler::XlmEnvelope;

sol! {
    struct Envelope {
        bytes20 eventId;
        bytes12 ordering;
        bytes payload;
    }
}

// ── Secp256k1 helpers ──────────────────────────────────────────────

fn make_secp_key(seed: u8) -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    SigningKey::from_bytes(&secret.into()).unwrap()
}

fn secp_compressed_pubkey(key: &SigningKey) -> [u8; 33] {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes();
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    arr
}

fn secp_sign_envelope(key: &SigningKey, envelope: &[u8]) -> [u8; 65] {
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

fn make_eth_envelope(event_id_seed: u8) -> (Vec<u8>, [u8; 20]) {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = Envelope {
        eventId: FixedBytes(event_id),
        ordering: FixedBytes([0u8; 12]),
        payload: vec![event_id_seed; 8].into(),
    };

    (envelope.abi_encode(), event_id)
}

// ── Ed25519 helpers ────────────────────────────────────────────────

fn make_ed25519_key(seed: u8) -> ed25519_dalek::SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    ed25519_dalek::SigningKey::from_bytes(&secret)
}

fn ed25519_pubkey(key: &ed25519_dalek::SigningKey) -> [u8; 32] {
    *key.verifying_key().as_bytes()
}

/// SEP-0053 signing: SHA256("Stellar Signed Message:\n" || envelope), then sign the hash
fn ed25519_sign_envelope(key: &ed25519_dalek::SigningKey, envelope: &[u8]) -> [u8; 64] {
    use sha2::Digest;
    let mut payload = Vec::new();
    payload.extend_from_slice(b"Stellar Signed Message:\n");
    payload.extend_from_slice(envelope);
    let hash = Sha256::digest(&payload);
    let sig = key.sign(&hash);
    sig.to_bytes()
}

fn make_xlm_envelope(env: &Env, event_id_seed: u8) -> (Vec<u8>, [u8; 20]) {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = XlmEnvelope {
        event_id: BytesN::from_array(env, &event_id),
        ordering: BytesN::from_array(env, &[0u8; 12]),
        payload: Bytes::from_slice(env, &[event_id_seed; 8]),
    };

    let xdr_bytes = envelope.to_xdr(env);
    (xdr_bytes.to_alloc_vec(), event_id)
}

// ── Main ───────────────────────────────────────────────────────────

fn main() {
    // ── Secp256k1 (Ethereum) vectors ───────────────────────────────

    let key1 = make_secp_key(1);
    let key2 = make_secp_key(2);
    let key3 = make_secp_key(3);

    let pk1 = secp_compressed_pubkey(&key1);
    let pk2 = secp_compressed_pubkey(&key2);
    let pk3 = secp_compressed_pubkey(&key3);

    let mut signers: Vec<(u8, [u8; 33], &SigningKey)> =
        vec![(1, pk1, &key1), (2, pk2, &key2), (3, pk3, &key3)];
    signers.sort_by(|a, b| a.1.cmp(&b.1));

    let (envelope1, event_id1) = make_eth_envelope(0x01);
    let (envelope2, event_id2) = make_eth_envelope(0x02);
    let (envelope3, event_id3) = make_eth_envelope(0x03);

    let sigs1: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| secp_sign_envelope(key, &envelope1))
        .collect();
    let sigs2: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| secp_sign_envelope(key, &envelope2))
        .collect();
    let sigs3: Vec<[u8; 65]> = signers
        .iter()
        .map(|(_, _, key)| secp_sign_envelope(key, &envelope3))
        .collect();

    println!("# ── Secp256k1 (Ethereum) test vectors ──");
    for (i, (seed, pk, _)) in signers.iter().enumerate() {
        let idx = i + 1;
        println!("SIGNER{}_SEED={}", idx, seed);
        println!("SIGNER{}_PUBKEY={}", idx, hex::encode(pk));
    }

    println!();
    println!("ENVELOPE1={}", hex::encode(&envelope1));
    println!("EVENT_ID1={}", hex::encode(event_id1));
    for (i, sig) in sigs1.iter().enumerate() {
        println!("ENVELOPE1_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("ENVELOPE2={}", hex::encode(&envelope2));
    println!("EVENT_ID2={}", hex::encode(event_id2));
    for (i, sig) in sigs2.iter().enumerate() {
        println!("ENVELOPE2_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("ENVELOPE3={}", hex::encode(&envelope3));
    println!("EVENT_ID3={}", hex::encode(event_id3));
    for (i, sig) in sigs3.iter().enumerate() {
        println!("ENVELOPE3_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("INVALID_SIG={}", hex::encode([0xAAu8; 65]));

    // ── Ed25519 (Stellar) vectors ──────────────────────────────────

    let env = Env::default();

    let ekey1 = make_ed25519_key(1);
    let ekey2 = make_ed25519_key(2);
    let ekey3 = make_ed25519_key(3);

    let epk1 = ed25519_pubkey(&ekey1);
    let epk2 = ed25519_pubkey(&ekey2);
    let epk3 = ed25519_pubkey(&ekey3);

    let mut ed_signers: Vec<(u8, [u8; 32], &ed25519_dalek::SigningKey)> =
        vec![(1, epk1, &ekey1), (2, epk2, &ekey2), (3, epk3, &ekey3)];
    ed_signers.sort_by(|a, b| a.1.cmp(&b.1));

    let (xlm_envelope1, xlm_event_id1) = make_xlm_envelope(&env, 0x11);
    let (xlm_envelope2, xlm_event_id2) = make_xlm_envelope(&env, 0x12);
    let (xlm_envelope3, xlm_event_id3) = make_xlm_envelope(&env, 0x13);

    let esigs1: Vec<[u8; 64]> = ed_signers
        .iter()
        .map(|(_, _, key)| ed25519_sign_envelope(key, &xlm_envelope1))
        .collect();
    let esigs2: Vec<[u8; 64]> = ed_signers
        .iter()
        .map(|(_, _, key)| ed25519_sign_envelope(key, &xlm_envelope2))
        .collect();
    let esigs3: Vec<[u8; 64]> = ed_signers
        .iter()
        .map(|(_, _, key)| ed25519_sign_envelope(key, &xlm_envelope3))
        .collect();

    println!();
    println!("# ── Ed25519 (Stellar) test vectors ──");
    for (i, (seed, pk, _)) in ed_signers.iter().enumerate() {
        let idx = i + 1;
        println!("ED_SIGNER{}_SEED={}", idx, seed);
        println!("ED_SIGNER{}_PUBKEY={}", idx, hex::encode(pk));
    }

    println!();
    println!("XLM_ENVELOPE1={}", hex::encode(&xlm_envelope1));
    println!("XLM_EVENT_ID1={}", hex::encode(xlm_event_id1));
    for (i, sig) in esigs1.iter().enumerate() {
        println!("XLM_ENVELOPE1_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("XLM_ENVELOPE2={}", hex::encode(&xlm_envelope2));
    println!("XLM_EVENT_ID2={}", hex::encode(xlm_event_id2));
    for (i, sig) in esigs2.iter().enumerate() {
        println!("XLM_ENVELOPE2_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("XLM_ENVELOPE3={}", hex::encode(&xlm_envelope3));
    println!("XLM_EVENT_ID3={}", hex::encode(xlm_event_id3));
    for (i, sig) in esigs3.iter().enumerate() {
        println!("XLM_ENVELOPE3_SIG{}={}", i + 1, hex::encode(sig));
    }

    println!();
    println!("ED_INVALID_SIG={}", hex::encode([0xBBu8; 64]));
}
