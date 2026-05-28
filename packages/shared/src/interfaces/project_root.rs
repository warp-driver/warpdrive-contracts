use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Val, Vec, contractclient, contractevent, contracttype,
};

use super::security::SecurityError;
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

#[contractevent]
pub struct Forwarded {
    pub target: Address,
    pub function: Symbol,
}

impl Forwarded {
    pub fn new(target: Address, function: Symbol) -> Self {
        Self { target, function }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "ProjectRootClient")]
pub trait ProjectRootInterface: WarpDriveInterface {
    // State Changing Operations
    fn update_project_spec_repo(env: Env, repo: String);

    /// Admin-gated proxy: invoke `function` on `target` with `args`, returning
    /// the inner call's return value. Errors from the inner call propagate.
    ///
    /// Set this contract as the admin of downstream contracts to use it as a
    /// single rotation point for the deployment.
    fn forward(env: Env, target: Address, function: Symbol, args: Vec<Val>) -> Val;

    // ── Typed forward helpers to the registered security_contract ──────

    /// Forward `add_signer(key, weight)` to the registered security contract
    /// using the secp256k1 (BytesN<33>) key shape.
    fn add_secp256k1_signer(env: Env, key: BytesN<33>, weight: u64) -> Result<(), SecurityError>;
    /// Forward `remove_signer(key)` to the registered security contract using
    /// the secp256k1 (BytesN<33>) key shape.
    fn remove_secp256k1_signer(env: Env, key: BytesN<33>);
    /// Forward `add_signer(key, weight)` to the registered security contract
    /// using the ed25519 (BytesN<32>) key shape.
    fn add_ed25519_signer(env: Env, key: BytesN<32>, weight: u64) -> Result<(), SecurityError>;
    /// Forward `remove_signer(key)` to the registered security contract using
    /// the ed25519 (BytesN<32>) key shape.
    fn remove_ed25519_signer(env: Env, key: BytesN<32>);
    /// Forward `set_threshold(numerator, denominator)` to the registered
    /// security contract. The signature is the same for both schemes.
    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError>;

    // ── Typed WarpDriveInterface forwarders (any target) ───────────────

    /// Forward `upgrade(new_wasm_hash, new_version)` to `target`. ProjectRoot
    /// must be `target`'s admin.
    fn upgrade_contract(env: Env, target: Address, new_wasm_hash: BytesN<32>, new_version: String);
    /// Forward `propose_admin(new_admin)` to `target`. ProjectRoot must be
    /// `target`'s admin. Use this to begin rotating the admin of a downstream
    /// contract away from ProjectRoot.
    fn propose_contract_admin(env: Env, target: Address, new_admin: Address);
    /// Forward `accept_admin()` to `target`. ProjectRoot must be `target`'s
    /// pending admin. Use this to take over admin of a downstream contract.
    fn accept_contract_admin(env: Env, target: Address);

    // Queries
    fn security_contract(env: Env) -> Address;
    fn verification_contract(env: Env) -> Address;
    fn project_spec_repo(env: Env) -> String;
    /// Returns which interface is used by security_contract and verification_contract
    fn verification_type(env: Env) -> VerificationType;
}
