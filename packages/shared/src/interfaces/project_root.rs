use soroban_sdk::{Address, Env, String, contractclient, contractevent, contracttype};

use super::warpdrive::WarpDriveInterface;

// ── Types ────────────────────────────────────────────────────────────

/// Identifies which cryptographic scheme and encoding format the project's
/// security and verification contracts use.
///
/// This is set once at construction time and cannot be changed. It tells
/// off-chain tooling and other contracts which pipeline variant this
/// project uses:
///
/// - **`Ethereum`** — secp256k1 keys, EIP-191 signatures, ABI-encoded
///   envelopes. Use this when the same signed payloads need to be
///   verifiable on both Ethereum (or other EVM chains) and Stellar.
///
/// - **`Stellar`** — ed25519 keys, SEP-0053 signatures, XDR-encoded
///   envelopes. Use this for Soroban-native solutions that don't need
///   EVM compatibility, giving better efficiency and simpler DevX.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum VerificationType {
    /// Secp256k1 / EIP-191 / ABI — Ethereum-compatible multi-chain format.
    Ethereum = 1,
    /// Ed25519 / SEP-0053 / XDR — Soroban-native format.
    Stellar = 2,
}

// ── Shared events ───────────────────────────────────────────────────

#[contractevent]
pub struct UpdatedSpecRepo {
    pub repo: String,
}

impl UpdatedSpecRepo {
    pub fn new(repo: String) -> Self {
        Self { repo }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "ProjectRootClient")]
pub trait ProjectRootInterface: WarpDriveInterface {
    // State Changing Operations
    fn update_project_spec_repo(env: Env, repo: String);

    // Queries
    fn security_contract(env: Env) -> Address;
    fn verification_contract(env: Env) -> Address;
    fn project_spec_repo(env: Env) -> String;
    /// Returns which interface is used by security_contract and verification_contract
    fn verification_type(env: Env) -> VerificationType;
}
