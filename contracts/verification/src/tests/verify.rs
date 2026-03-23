extern crate std;

use crate::{Verification, VerificationClient, VerifyError};
use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};
use soroban_sdk::{Bytes, BytesN, Env, Vec, testutils::Address as _};
use warpdrive_security::{Security, SecurityClient};

type PubKey = BytesN<33>;

/// Deterministically generate a secp256k1 signing key from a seed byte.
fn make_signing_key(seed: u8) -> SigningKey {
    let mut secret = [0u8; 32];
    secret[31] = seed; // minimal valid scalar
    SigningKey::from_bytes(&secret.into()).unwrap()
}

/// Derive the compressed public key (33 bytes) from a signing key.
fn compressed_pubkey(env: &Env, key: &SigningKey) -> PubKey {
    let vk = key.verifying_key();
    let bytes = vk.to_sec1_bytes(); // compressed by default (33 bytes)
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&bytes);
    PubKey::from_array(env, &arr)
}

/// Sign an envelope the same way `is_valid_signature` expects:
/// digest = keccak256("\x19Ethereum Signed Message:\n32" || keccak256(envelope))
/// Returns a 65-byte signature: r(32) || s(32) || v(1) with Ethereum-style v (27/28).
fn sign_envelope(key: &SigningKey, envelope: &[u8]) -> [u8; 65] {
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

fn setup_contracts(env: &Env) -> (VerificationClient<'_>, SecurityClient<'_>) {
    let admin = soroban_sdk::Address::generate(env);

    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);

    // Deploy security contract with threshold 55/100
    let security_id = env.register(Security, (&admin, 55u64, 100u64));
    let security = SecurityClient::new(env, &security_id);

    // key1 weight 100, key2 weight 200
    security
        .mock_all_auths()
        .add_signer(&compressed_pubkey(env, &key1), &100);
    security
        .mock_all_auths()
        .add_signer(&compressed_pubkey(env, &key2), &200);

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

    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);

    assert_eq!(
        verification.signer_weight(&compressed_pubkey(&env, &key1)),
        100
    );
    assert_eq!(
        verification.signer_weight(&compressed_pubkey(&env, &key2)),
        200
    );
}

#[test]
fn test_signer_weight_missing() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key3 = make_signing_key(3);
    assert_eq!(
        verification.signer_weight(&compressed_pubkey(&env, &key3)),
        0
    );
}

#[test]
fn test_verify_invalid_signature() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_signing_key(2);
    let pubkey2 = compressed_pubkey(&env, &key2);

    let envelope = Bytes::from_slice(&env, b"hello world");
    // Garbage signature — not valid for any message
    let bad_sig = BytesN::from_array(&env, &[0xAA; 65]);

    let result = verification.try_verify_one(&envelope, &bad_sig, &pubkey2);
    assert_eq!(result, Err(Ok(VerifyError::InvalidSignature)));
}

#[test]
fn test_verify_success_high_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key2 = make_signing_key(2);
    let pubkey2 = compressed_pubkey(&env, &key2);

    let message = b"hello world";
    let sig_bytes = sign_envelope(&key2, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key2 has weight 200 >= required 165, should succeed
    let result = verification.try_verify_one(&envelope, &signature, &pubkey2);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_verify_insufficient_weight() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key1 = make_signing_key(1);
    let pubkey1 = compressed_pubkey(&env, &key1);

    let message = b"hello world";
    let sig_bytes = sign_envelope(&key1, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key1 has weight 100 < required 165
    let result = verification.try_verify_one(&envelope, &signature, &pubkey1);
    assert_eq!(result, Err(Ok(VerifyError::InsufficientWeight)));
}

#[test]
fn test_verify_signer_not_registered() {
    let env = Env::default();
    let (verification, _security) = setup_contracts(&env);

    let key3 = make_signing_key(3);
    let pubkey3 = compressed_pubkey(&env, &key3);

    let message = b"hello world";
    let sig_bytes = sign_envelope(&key3, message);

    let envelope = Bytes::from_slice(&env, message);
    let signature = BytesN::from_array(&env, &sig_bytes);

    // key3 is not registered in the security contract
    let result = verification.try_verify_one(&envelope, &signature, &pubkey3);
    assert_eq!(result, Err(Ok(VerifyError::SignerNotRegistered)));
}

// ── verify (multi-sig) tests ────────────────────────────────────────

/// Return (lo_key, lo_pubkey, hi_key, hi_pubkey) where lo < hi by byte order.
fn ordered_keys(env: &Env) -> (SigningKey, PubKey, SigningKey, PubKey) {
    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);
    let pk1 = compressed_pubkey(env, &key1);
    let pk2 = compressed_pubkey(env, &key2);

    if pk1.to_array() < pk2.to_array() {
        (key1, pk1, key2, pk2)
    } else {
        (key2, pk2, key1, pk1)
    }
}

#[test]
fn test_verify_multi_empty_signatures() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let envelope = Bytes::from_slice(&env, b"hello world");
    let sigs: Vec<BytesN<65>> = Vec::new(&env);
    let pubs: Vec<PubKey> = Vec::new(&env);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::EmptySignatures)));
}

#[test]
fn test_verify_multi_length_mismatch() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_bytes = sign_envelope(&lo_key, message);
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_bytes));

    // Two pubkeys but only one signature
    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(lo_pub.clone());
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::LengthMismatch)));
}

#[test]
fn test_verify_multi_signers_not_ordered() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = sign_envelope(&lo_key, message);
    let sig_hi = sign_envelope(&hi_key, message);

    // Provide in descending order (hi, lo) — should fail
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_hi));
    sigs.push_back(BytesN::from_array(&env, &sig_lo));

    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(hi_pub);
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::SignersNotOrdered)));
}

#[test]
fn test_verify_multi_duplicate_signer() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig = sign_envelope(&lo_key, message);

    // Same signer twice — not strictly ascending
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig));
    sigs.push_back(BytesN::from_array(&env, &sig));

    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(lo_pub.clone());
    pubs.push_back(lo_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::SignersNotOrdered)));
}

#[test]
fn test_verify_multi_one_invalid_signature() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (_, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let valid_sig = sign_envelope(&hi_key, message);
    let bad_sig = [0xAA; 65];

    // First sig is garbage, second is valid
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &bad_sig));
    sigs.push_back(BytesN::from_array(&env, &valid_sig));

    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(lo_pub);
    pubs.push_back(hi_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::InvalidSignature)));
}

#[test]
fn test_verify_multi_signer_not_registered() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let key3 = make_signing_key(3);
    let pk3 = compressed_pubkey(&env, &key3);

    let (lo_key, lo_pub, _, _) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = sign_envelope(&lo_key, message);
    let sig_3 = sign_envelope(&key3, message);

    // Order them correctly by pubkey bytes
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    let mut pubs: Vec<PubKey> = Vec::new(&env);
    if lo_pub.to_array() < pk3.to_array() {
        sigs.push_back(BytesN::from_array(&env, &sig_lo));
        sigs.push_back(BytesN::from_array(&env, &sig_3));
        pubs.push_back(lo_pub);
        pubs.push_back(pk3);
    } else {
        sigs.push_back(BytesN::from_array(&env, &sig_3));
        sigs.push_back(BytesN::from_array(&env, &sig_lo));
        pubs.push_back(pk3);
        pubs.push_back(lo_pub);
    }

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    // key3 is not registered — should fail (may hit on first or second position)
    assert_eq!(result, Err(Ok(VerifyError::SignerNotRegistered)));
}

#[test]
fn test_verify_multi_insufficient_total_weight() {
    let env = Env::default();
    let admin = soroban_sdk::Address::generate(&env);

    let key1 = make_signing_key(1);
    let key2 = make_signing_key(2);
    let pk1 = compressed_pubkey(&env, &key1);
    let pk2 = compressed_pubkey(&env, &key2);

    // threshold 90/100 — both signers needed at very high threshold
    let security_id = env.register(Security, (&admin, 90u64, 100u64));
    let security = SecurityClient::new(&env, &security_id);

    // key1 weight 10, key2 weight 10 — total 20, required = 20 * 90 / 100 = 18
    // Actually that passes. Use weight 5 each: total 10, required = 10 * 90 / 100 = 9 — passes too.
    // Let's use: key1 weight 1, key2 not added. Single signer, weight 1, threshold 90%.
    // required = 1 * 90 / 100 = 0 — hmm, integer math.
    // Better: use the default setup (key1=100, key2=200, threshold 55%).
    // required = 300 * 55 / 100 = 165. key1 alone = 100 < 165.
    // So just use key1 alone in multi-verify.
    let security_id2 = env.register(Security, (&admin, 55u64, 100u64));
    let security2 = SecurityClient::new(&env, &security_id2);
    security2.mock_all_auths().add_signer(&pk1, &100);
    security2.mock_all_auths().add_signer(&pk2, &200);

    let verification_id = env.register(Verification, (&admin, &security_id2));
    let verification = VerificationClient::new(&env, &verification_id);

    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);
    let sig1 = sign_envelope(&key1, message);

    // Only key1 (weight 100) — required is 165
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig1));

    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(pk1);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Err(Ok(VerifyError::InsufficientWeight)));
}

#[test]
fn test_verify_multi_success_combined_weight() {
    let env = Env::default();
    let (verification, _) = setup_contracts(&env);

    let (lo_key, lo_pub, hi_key, hi_pub) = ordered_keys(&env);
    let message = b"hello world";
    let envelope = Bytes::from_slice(&env, message);

    let sig_lo = sign_envelope(&lo_key, message);
    let sig_hi = sign_envelope(&hi_key, message);

    // Both signers: weight 100 + 200 = 300 >= required 165
    let mut sigs: Vec<BytesN<65>> = Vec::new(&env);
    sigs.push_back(BytesN::from_array(&env, &sig_lo));
    sigs.push_back(BytesN::from_array(&env, &sig_hi));

    let mut pubs: Vec<PubKey> = Vec::new(&env);
    pubs.push_back(lo_pub);
    pubs.push_back(hi_pub);

    let result = verification.try_verify(&envelope, &sigs, &pubs);
    assert_eq!(result, Ok(Ok(())));
}
