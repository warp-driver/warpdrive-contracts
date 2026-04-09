use soroban_sdk::{
    Address, Bytes, BytesN, Env, Vec, contractclient, contracterror, contractevent, contracttype,
};

use super::verification::VerifyError;
use super::warpdrive::WarpDriveInterface;

// ── Error ────────────────────────────────────────────────────────────

// Namespacing: Handler errors are from 500-599

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HandlerError {
    // Errors from the handler itself
    EventAlreadySeen = 501,
    InvalidReferenceBlock = 502,
    InvalidEnvelope = 503,
    // These are unknown errors when calling the verification contract
    UnknownVerificationError = 504,
    OtherInvocationError = 505,

    // Some numbers intentionally skipped...
    // Mapped from VerifyError (use same enum values from their space
    InvalidSignature = 301,
    SignerNotRegistered = 302,
    InsufficientWeight = 303,
    EmptySignatures = 304,
    LengthMismatch = 305,
    SignersNotOrdered = 306,
    ZeroRequiredWeight = 307,
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
            VerifyError::ZeroRequiredWeight => HandlerError::ZeroRequiredWeight,
        }
    }
}

// ── Types ────────────────────────────────────────────────────────────

#[contracttype]
pub struct SignatureData {
    pub signers: Vec<BytesN<33>>,
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

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "HandlerClient")]
pub trait HandlerInterface: WarpDriveInterface {
    // State Changing Operations (if verification succeeds)
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

    // Queries
    fn verification_contract(env: Env) -> Address;
    fn payload(env: Env, event_id: BytesN<20>) -> Option<Bytes>;
}
