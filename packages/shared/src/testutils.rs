extern crate std;

pub use k256::ecdsa::SigningKey as SecpSigningKey;
use sha3::{Digest, Keccak256};
use soroban_sdk::Env;

use ed25519_dalek::Signer;
pub use ed25519_dalek::SigningKey as Ed25519SigningKey;

pub use crate::interfaces::{CompressedSecpPubKey, Ed25519PubKey};

// ── Secp256k1 ───────────────────────────────────────────────────────

/// Deterministically generate a secp256k1 signing key from a seed byte.
pub fn make_secp256k1_key(seed: u8) -> SecpSigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed; // minimal valid scalar
    SecpSigningKey::from_bytes(&secret.into()).unwrap()
}

/// Derive the compressed public key (33 bytes) from a secp256k1 signing key.
pub fn secp256k1_pubkey(env: &Env, key: &SecpSigningKey) -> CompressedSecpPubKey {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes(); // compressed by default (33 bytes)
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    CompressedSecpPubKey::from_array(env, &arr)
}

/// Sign an envelope the same way `is_valid_signature` expects:
/// digest = keccak256("\x19Ethereum Signed Message:\n32" || keccak256(envelope))
/// Returns a 65-byte signature: r(32) || s(32) || v(1) with Ethereum-style v (27/28).
pub fn secp256k1_sign_envelope(key: &SecpSigningKey, envelope: &[u8]) -> [u8; 65] {
    // Step 1: keccak256(envelope)
    let inner_hash = Keccak256::digest(envelope);

    // Step 2: EIP-191 wrap
    let mut prefixed = std::vec::Vec::new();
    prefixed.extend_from_slice(b"\x19Ethereum Signed Message:\n32");
    prefixed.extend_from_slice(&inner_hash);
    let digest = Keccak256::digest(&prefixed);

    // Step 3: sign the digest
    let (sig, recid) = key
        .sign_prehash_recoverable(&digest)
        .expect("signing failed");

    // Step 4: pack as r || s || v (Ethereum format: v = recid + 27)
    let mut result = [0u8; 65];
    result[..64].copy_from_slice(&sig.to_bytes());
    result[64] = recid.to_byte() + 27;
    result
}

// pub use SecpSigningKey as SigningKey;

// pub fn make_signing_key(seed: u8) -> SecpSigningKey {
//     make_secp256k1_key(seed)
// }

// pub fn compressed_pubkey(env: &Env, key: &SecpSigningKey) -> CompressedSecpPubKey {
//     secp256k1_pubkey(env, key)
// }

// pub fn sign_envelope(key: &SecpSigningKey, envelope: &[u8]) -> [u8; 65] {
//     secp256k1_sign_envelope(key, envelope)
// }

// ── Ed25519 ─────────────────────────────────────────────────────────

/// Deterministically generate an ed25519 signing key from a seed byte.
pub fn make_ed25519_key(seed: u8) -> Ed25519SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed;
    Ed25519SigningKey::from_bytes(&secret)
}

/// Derive the public key (32 bytes) from an ed25519 signing key.
pub fn ed25519_pubkey(env: &Env, key: &Ed25519SigningKey) -> Ed25519PubKey {
    let vk = key.verifying_key();
    Ed25519PubKey::from_array(env, vk.as_bytes())
}

/// Sign an envelope using SEP-0053 format with ed25519.
/// Computes `SHA256("Stellar Signed Message:\n" || envelope)` then signs the hash.
/// Returns a 64-byte ed25519 signature.
pub fn ed25519_sign_envelope(key: &Ed25519SigningKey, envelope: &[u8]) -> [u8; 64] {
    use sha2::{Digest, Sha256};

    let mut payload = std::vec::Vec::new();
    payload.extend_from_slice(b"Stellar Signed Message:\n");
    payload.extend_from_slice(envelope);
    let hash = Sha256::digest(&payload);

    let sig = key.sign(&hash);
    sig.to_bytes()
}
