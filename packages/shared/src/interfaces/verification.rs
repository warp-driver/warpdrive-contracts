use soroban_sdk::{Address, Bytes, Env, Vec, contractclient, contracterror};

use super::warpdrive::WarpDriveInterface;
use super::{CompressedSecpPubKey, Ed25519PubKey, Ed25519Signature, SecpSignature};

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

#[contractclient(name = "Secp256k1VerificationClient")]
pub trait Secp256k1VerificationInterface: WarpDriveInterface {
    // Queries
    fn security_contract(env: Env) -> Address;
    fn required_weight(env: Env) -> u64;
    fn signer_weight(env: Env, signer_pubkey: CompressedSecpPubKey) -> u64;

    /// Verify one signature, return the weight of this signer if the signature is valid
    fn check_one(
        env: Env,
        envelope: Bytes,
        signature: SecpSignature,
        signer_pubkey: CompressedSecpPubKey,
        reference_block: Option<u32>,
    ) -> Result<u64, VerifyError>;

    /// Verify a set of signatures, which must be sorted by pubkeys.
    /// Returns error on any invalid signatures, or if the total weight of signers does not meet the threshold (required_weight)
    fn verify(
        env: Env,
        envelope: Bytes,
        signatures: Vec<SecpSignature>,
        signer_pubkeys: Vec<CompressedSecpPubKey>,
        reference_block: u32,
    ) -> Result<(), VerifyError>;
}

#[contractclient(name = "Ed25519VerificationClient")]
pub trait Ed25519VerificationInterface: WarpDriveInterface {
    // Queries
    fn security_contract(env: Env) -> Address;
    fn required_weight(env: Env) -> u64;
    fn signer_weight(env: Env, signer_pubkey: Ed25519PubKey) -> u64;

    /// Verify one signature, return the weight of this signer if the signature is valid
    fn check_one(
        env: Env,
        envelope: Bytes,
        signature: Ed25519Signature,
        signer_pubkey: Ed25519PubKey,
        reference_block: Option<u32>,
    ) -> Result<u64, VerifyError>;

    /// Verify a set of signatures, which must be sorted by pubkeys.
    /// Returns error on any invalid signatures, or if the total weight of signers does not meet the threshold (required_weight)
    fn verify(
        env: Env,
        envelope: Bytes,
        signatures: Vec<Ed25519Signature>,
        signer_pubkeys: Vec<Ed25519PubKey>,
        reference_block: u32,
    ) -> Result<(), VerifyError>;
}
