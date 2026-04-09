use soroban_sdk::{Address, Bytes, BytesN, Env, Vec, contractclient, contracterror};

use super::warpdrive::WarpDriveInterface;

// ── Error ────────────────────────────────────────────────────────────

// Namespacing: Verification errors are from 300-399

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VerifyError {
    InvalidSignature = 301,
    SignerNotRegistered = 302,
    InsufficientWeight = 303,
    EmptySignatures = 304,
    LengthMismatch = 305,
    SignersNotOrdered = 306,
    ZeroRequiredWeight = 307,
}

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "VerificationClient")]
pub trait VerificationInterface: WarpDriveInterface {
    // Queries
    fn security_contract(env: Env) -> Address;
    fn required_weight(env: Env) -> u64;
    fn signer_weight(env: Env, signer_pubkey: BytesN<33>) -> u64;
    fn check_one(
        env: Env,
        envelope: Bytes,
        signature: BytesN<65>,
        signer_pubkey: BytesN<33>,
        reference_block: Option<u32>,
    ) -> Result<u64, VerifyError>;
    fn verify(
        env: Env,
        envelope: Bytes,
        signatures: Vec<BytesN<65>>,
        signer_pubkeys: Vec<BytesN<33>>,
        reference_block: u32,
    ) -> Result<(), VerifyError>;
}
