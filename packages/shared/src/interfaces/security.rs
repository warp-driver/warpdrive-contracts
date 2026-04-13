use soroban_sdk::{Env, Vec, contractclient, contracterror, contractevent, contracttype};

use crate::interfaces::Ed25519PubKey;

use super::CompressedSecpPubKey;
use super::warpdrive::WarpDriveInterface;

// ── Error ────────────────────────────────────────────────────────────

// Namespacing: Security errors are from 200-299

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum SecurityError {
    ZeroDenominator = 201,
    NumeratorExceedsDenominator = 202,
    ZeroNumerator = 203,
    ZeroWeight = 204,
}

// ── Secp256k1 types ─────────────────────────────────────────────────

#[contracttype]
pub struct SignerInfo {
    pub key: CompressedSecpPubKey,
    pub weight: u64,
}

#[contractevent]
pub struct SignerAdded {
    #[topic]
    pub key: CompressedSecpPubKey,
    pub weight: u64,
}

impl SignerAdded {
    pub fn new(key: CompressedSecpPubKey, weight: u64) -> Self {
        Self { key, weight }
    }
}

#[contractevent]
pub struct SignerRemoved {
    #[topic]
    pub key: CompressedSecpPubKey,
}

impl SignerRemoved {
    pub fn new(key: CompressedSecpPubKey) -> Self {
        Self { key }
    }
}

// ── Ed25519 types ───────────────────────────────────────────────────

#[contracttype]
pub struct Ed25519SignerInfo {
    pub key: Ed25519PubKey,
    pub weight: u64,
}

#[contractevent]
pub struct Ed25519SignerAdded {
    #[topic]
    pub key: Ed25519PubKey,
    pub weight: u64,
}

impl Ed25519SignerAdded {
    pub fn new(key: Ed25519PubKey, weight: u64) -> Self {
        Self { key, weight }
    }
}

#[contractevent]
pub struct Ed25519SignerRemoved {
    #[topic]
    pub key: Ed25519PubKey,
}

impl Ed25519SignerRemoved {
    pub fn new(key: Ed25519PubKey) -> Self {
        Self { key }
    }
}

// ── Shared events ───────────────────────────────────────────────────

#[contractevent]
pub struct ThresholdSet {
    pub numerator: u64,
    pub denominator: u64,
}

impl ThresholdSet {
    pub fn new(numerator: u64, denominator: u64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }
}

// ── Interface traits (compile-time contract conformance) ────────────

#[contractclient(name = "Secp256k1SecurityClient")]
pub trait Secp256k1SecurityInterface: WarpDriveInterface {
    // State Changing Operations
    fn add_signer(env: Env, key: CompressedSecpPubKey, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: CompressedSecpPubKey);
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // Queries
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: CompressedSecpPubKey) -> u64;
    fn get_signer_weight_at(env: Env, key: CompressedSecpPubKey, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<CompressedSecpPubKey>) -> Vec<u64>;
    fn get_signer_weights_at(
        env: Env,
        keys: Vec<CompressedSecpPubKey>,
        reference_block: u32,
    ) -> Vec<u64>;
    fn get_total_weight_at(env: Env, reference_block: u32) -> u64;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
    fn list_signers(env: Env) -> Vec<SignerInfo>;
    fn threshold_numerator(env: Env) -> u64;
    fn threshold_denominator(env: Env) -> u64;
    fn required_weight(env: Env) -> u64;
}

#[contractclient(name = "Ed25519SecurityClient")]
pub trait Ed25519SecurityInterface: WarpDriveInterface {
    // State Changing Operations
    fn add_signer(env: Env, key: Ed25519PubKey, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: Ed25519PubKey);
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // Queries
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: Ed25519PubKey) -> u64;
    fn get_signer_weight_at(env: Env, key: Ed25519PubKey, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<Ed25519PubKey>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<Ed25519PubKey>, reference_block: u32) -> Vec<u64>;
    fn get_total_weight_at(env: Env, reference_block: u32) -> u64;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
    fn list_signers(env: Env) -> Vec<Ed25519SignerInfo>;
    fn threshold_numerator(env: Env) -> u64;
    fn threshold_denominator(env: Env) -> u64;
    fn required_weight(env: Env) -> u64;
}
