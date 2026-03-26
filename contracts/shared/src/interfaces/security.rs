use enum_repr::EnumRepr;
use soroban_sdk::{
    Address, BytesN, Env, String, Vec, contractclient, contracterror, contractevent, contracttype,
};

use super::PubKey;

// ── Error ────────────────────────────────────────────────────────────

#[contracterror]
#[EnumRepr(type = "u32")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum SecurityError {
    ZeroDenominator = 1,
    NumeratorExceedsDenominator = 2,
    ZeroNumerator = 3,
    ZeroWeight = 4,
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

#[contractevent]
pub struct Upgraded {
    pub version: String,
}

impl Upgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

pub trait SecurityInterface {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String);
    fn admin(env: Env) -> Address;
    fn pending_admin(env: Env) -> Option<Address>;
    fn propose_admin(env: Env, new_admin: Address);
    fn accept_admin(env: Env);
    fn version(env: Env) -> String;
    fn add_signer(env: Env, key: PubKey, weight: u64) -> Result<(), SecurityError>;
    fn remove_signer(env: Env, key: PubKey);
    fn get_total_weight(env: Env) -> u64;
    fn get_signer_weight(env: Env, key: PubKey) -> u64;
    fn get_signer_weight_at(env: Env, key: PubKey, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<PubKey>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<PubKey>, reference_block: u32) -> Vec<u64>;
    fn get_total_weight_at(env: Env, reference_block: u32) -> u64;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
    fn list_signers(env: Env) -> Vec<SignerInfo>;
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;
    fn threshold_numerator(env: Env) -> u64;
    fn threshold_denominator(env: Env) -> u64;
    fn required_weight(env: Env) -> u64;
}

// ── Client trait (cross-contract calls) ──────────────────────────────

#[contractclient(name = "SecurityClient")]
#[allow(dead_code)]
pub trait SecurityClientInterface {
    fn get_signer_weight(env: Env, key: PubKey) -> u64;
    fn required_weight(env: Env) -> u64;
    fn get_signer_weight_at(env: Env, key: PubKey, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<PubKey>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<PubKey>, reference_block: u32) -> Vec<u64>;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
}
