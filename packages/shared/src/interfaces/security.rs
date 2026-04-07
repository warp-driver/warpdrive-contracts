use soroban_sdk::{Env, Vec, contractclient, contracterror, contractevent, contracttype};

use super::PubKey;
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

// ── Types ────────────────────────────────────────────────────────────

#[contracttype]
pub struct SignerInfo {
    pub key: PubKey,
    pub weight: u64,
}

// ── Events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct SignerAdded {
    #[topic]
    pub key: PubKey,
    pub weight: u64,
}

impl SignerAdded {
    pub fn new(key: PubKey, weight: u64) -> Self {
        Self { key, weight }
    }
}

#[contractevent]
pub struct SignerRemoved {
    #[topic]
    pub key: PubKey,
}

impl SignerRemoved {
    pub fn new(key: PubKey) -> Self {
        Self { key }
    }
}

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

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "SecurityClient")]
pub trait SecurityInterface: WarpDriveInterface {
    // State Changing Operations
    fn add_signer(env: Env, key: PubKey, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: PubKey);
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // Queries
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: PubKey) -> u64;
    fn get_signer_weight_at(env: Env, key: PubKey, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<PubKey>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<PubKey>, reference_block: u32) -> Vec<u64>;
    fn get_total_weight_at(env: Env, reference_block: u32) -> u64;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
    fn list_signers(env: Env) -> Vec<SignerInfo>;
    fn threshold_numerator(env: Env) -> u64;
    fn threshold_denominator(env: Env) -> u64;
    fn required_weight(env: Env) -> u64;
}
