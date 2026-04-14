use soroban_sdk::{BytesN, Env, Vec, contractclient, contracterror, contractevent, contracttype};

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
    pub key: BytesN<33>,
    pub weight: u64,
}

#[contractevent]
pub struct SignerAdded {
    #[topic]
    pub key: BytesN<33>,
    pub weight: u64,
}

impl SignerAdded {
    pub fn new(key: BytesN<33>, weight: u64) -> Self {
        Self { key, weight }
    }
}

#[contractevent]
pub struct SignerRemoved {
    #[topic]
    pub key: BytesN<33>,
}

impl SignerRemoved {
    pub fn new(key: BytesN<33>) -> Self {
        Self { key }
    }
}

// ── Ed25519 types ───────────────────────────────────────────────────

#[contracttype]
pub struct Ed25519SignerInfo {
    pub key: BytesN<32>,
    pub weight: u64,
}

#[contractevent]
pub struct Ed25519SignerAdded {
    #[topic]
    pub key: BytesN<32>,
    pub weight: u64,
}

impl Ed25519SignerAdded {
    pub fn new(key: BytesN<32>, weight: u64) -> Self {
        Self { key, weight }
    }
}

#[contractevent]
pub struct Ed25519SignerRemoved {
    #[topic]
    pub key: BytesN<32>,
}

impl Ed25519SignerRemoved {
    pub fn new(key: BytesN<32>) -> Self {
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
    fn add_signer(env: Env, key: BytesN<33>, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: BytesN<33>);
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // Queries
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: BytesN<33>) -> u64;
    fn get_signer_weight_at(env: Env, key: BytesN<33>, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<BytesN<33>>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<BytesN<33>>, reference_block: u32) -> Vec<u64>;
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
    fn add_signer(env: Env, key: BytesN<32>, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: BytesN<32>);
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // Queries
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: BytesN<32>) -> u64;
    fn get_signer_weight_at(env: Env, key: BytesN<32>, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<BytesN<32>>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<BytesN<32>>, reference_block: u32) -> Vec<u64>;
    fn get_total_weight_at(env: Env, reference_block: u32) -> u64;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
    fn list_signers(env: Env) -> Vec<Ed25519SignerInfo>;
    fn threshold_numerator(env: Env) -> u64;
    fn threshold_denominator(env: Env) -> u64;
    fn required_weight(env: Env) -> u64;
}
