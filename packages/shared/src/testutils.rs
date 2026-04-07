extern crate std;

pub use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};
use soroban_sdk::{BytesN, Env};

pub type PubKey = BytesN<33>;

/// Deterministically generate a secp256k1 signing key from a seed byte.
pub fn make_signing_key(seed: u8) -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed; // minimal valid scalar
    SigningKey::from_bytes(&secret.into()).unwrap()
}

/// Derive the compressed public key (33 bytes) from a signing key.
pub fn compressed_pubkey(env: &Env, key: &SigningKey) -> PubKey {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes(); // compressed by default (33 bytes)
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    PubKey::from_array(env, &arr)
}

/// Sign an envelope the same way `is_valid_signature` expects:
/// digest = keccak256("\x19Ethereum Signed Message:\n32" || keccak256(envelope))
/// Returns a 65-byte signature: r(32) || s(32) || v(1) with Ethereum-style v (27/28).
pub fn sign_envelope(key: &SigningKey, envelope: &[u8]) -> [u8; 65] {
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
