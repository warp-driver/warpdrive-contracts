use enum_repr::EnumRepr;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, String, Vec, contractclient, contracterror, contractevent,
};

use super::PubKey;

// ── Error ────────────────────────────────────────────────────────────

#[contracterror]
#[EnumRepr(type = "u32")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VerifyError {
    InvalidSignature = 1,
    SignerNotRegistered = 2,
    InsufficientWeight = 3,
    EmptySignatures = 4,
    LengthMismatch = 5,
    SignersNotOrdered = 6,
    ZeroRequiredWeight = 7,
}

// ── Events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct VerificationUpgraded {
    pub version: String,
}

impl VerificationUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "VerificationClient")]
pub trait VerificationInterface {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String);
    fn admin(env: Env) -> Address;
    fn pending_admin(env: Env) -> Option<Address>;
    fn propose_admin(env: Env, new_admin: Address);
    fn accept_admin(env: Env);
    fn version(env: Env) -> String;
    fn security_contract(env: Env) -> Address;
    fn required_weight(env: Env) -> u64;
    fn signer_weight(env: Env, signer_pubkey: PubKey) -> u64;
    fn check_one(
        env: Env,
        envelope: Bytes,
        signature: BytesN<65>,
        signer_pubkey: PubKey,
        reference_block: Option<u32>,
    ) -> Result<u64, VerifyError>;
    fn verify(
        env: Env,
        envelope: Bytes,
        signatures: Vec<BytesN<65>>,
        signer_pubkeys: Vec<PubKey>,
        reference_block: u32,
    ) -> Result<(), VerifyError>;
}
