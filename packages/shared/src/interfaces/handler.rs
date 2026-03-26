use enum_repr::EnumRepr;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, String, Vec, contracterror, contractevent, contracttype,
};

use crate::interfaces::PubKey;

use super::verification::VerifyError;

// ── Error ────────────────────────────────────────────────────────────

#[contracterror]
#[EnumRepr(type = "u32")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HandlerError {
    // Errors from the handler itself
    EventAlreadySeen = 1,
    InvalidReferenceBlock = 2,
    InvalidEnvelope = 3,

    // Some numbers intentionally skipped...
    UnknownVerificationError = 20,
    // Mapped from VerifyError
    InvalidSignature = 21,
    SignerNotRegistered = 22,
    InsufficientWeight = 23,
    EmptySignatures = 24,
    LengthMismatch = 25,
    SignersNotOrdered = 26,
}

impl From<VerifyError> for HandlerError {
    fn from(value: VerifyError) -> Self {
        match value {
            VerifyError::InvalidSignature => HandlerError::InvalidSignature,
            VerifyError::SignerNotRegistered => HandlerError::SignerNotRegistered,
            VerifyError::InsufficientWeight => HandlerError::InsufficientWeight,
            VerifyError::EmptySignatures => HandlerError::EmptySignatures,
            VerifyError::LengthMismatch => HandlerError::LengthMismatch,
            VerifyError::SignersNotOrdered => HandlerError::SignersNotOrdered,
        }
    }
}

// ── Types ────────────────────────────────────────────────────────────

#[contracttype]
pub struct SignatureData {
    pub signers: Vec<PubKey>,
    pub signatures: Vec<BytesN<65>>,
    pub reference_block: u32,
}

#[contracttype]
pub struct XlmEnvelope {
    pub event_id: BytesN<20>,
    pub ordering: BytesN<12>,
    pub payload: Bytes,
}

// ── Events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct Verified {
    #[topic]
    pub event_id: BytesN<20>,
}

impl Verified {
    pub fn new(event_id: BytesN<20>) -> Self {
        Self { event_id }
    }
}

#[contractevent]
pub struct HandlerUpgraded {
    pub version: String,
}

impl HandlerUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

pub trait HandlerInterface {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String);
    fn admin(env: Env) -> Address;
    fn pending_admin(env: Env) -> Option<Address>;
    fn propose_admin(env: Env, new_admin: Address);
    fn accept_admin(env: Env);
    fn version(env: Env) -> String;
    fn verification_contract(env: Env) -> Address;
    fn payload(env: Env, event_id: BytesN<20>) -> Option<Bytes>;
    fn verify_eth(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError>;
    fn verify_xlm(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError>;
}
