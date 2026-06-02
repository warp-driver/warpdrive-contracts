use soroban_sdk::{
    Address, BytesN, Env, String, Symbol, Vec, contractclient, contracterror, contractevent,
    contracttype,
};

use super::security::SecurityError;
use super::warpdrive::WarpDriveInterface;

// ── Error ────────────────────────────────────────────────────────────

// Namespacing: ProjectRoot errors are from 100-199

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ProjectRootError {
    NotOurContract = 101,
    /// The target is part of this project but is not a handler (e.g. the
    /// security or verification contract), so it can't be registered in the
    /// handler set.
    NotAHandler = 102,

    // Mapped from SecurityError (use same enum values from their space)
    ZeroDenominator = 201,
    NumeratorExceedsDenominator = 202,
    ZeroNumerator = 203,
    ZeroWeight = 204,
}

impl From<SecurityError> for ProjectRootError {
    fn from(value: SecurityError) -> Self {
        match value {
            SecurityError::ZeroDenominator => ProjectRootError::ZeroDenominator,
            SecurityError::NumeratorExceedsDenominator => {
                ProjectRootError::NumeratorExceedsDenominator
            }
            SecurityError::ZeroNumerator => ProjectRootError::ZeroNumerator,
            SecurityError::ZeroWeight => ProjectRootError::ZeroWeight,
        }
    }
}

/// Classifies a forwarding target relative to this project: the registered
/// security or verification contract, or a handler that reports this
/// project's verification contract. Returned by the internal
/// `ensure_our_contract` guard so the admin forwarders can apply
/// handler-specific bookkeeping (tracking/untracking it in the handler set).
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractType {
    Security,
    Verification,
    Handler,
}

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

/// Emitted when a handler joins this project's tracked handler set — either
/// explicitly via `register_handler`, or implicitly when `accept_contract_admin`
/// takes over a handler's admin. Re-registering an already-tracked handler is a
/// no-op and does not re-emit. The `handler` address is a topic so off-chain
/// consumers can filter membership changes by handler.
#[contractevent]
pub struct HandlerRegistered {
    #[topic]
    pub handler: Address,
}

impl HandlerRegistered {
    pub fn new(handler: Address) -> Self {
        Self { handler }
    }
}

/// Emitted when a handler is removed from the tracked set via
/// `unregister_handler`. Removal is always explicit: rotating a handler's admin
/// away with `propose_contract_admin` does **not** untrack it. Unregistering a
/// handler that isn't tracked is a no-op and does not emit.
#[contractevent]
pub struct HandlerRemoved {
    #[topic]
    pub handler: Address,
}

impl HandlerRemoved {
    pub fn new(handler: Address) -> Self {
        Self { handler }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "ProjectRootClient")]
pub trait ProjectRootInterface: WarpDriveInterface {
    // State Changing Operations
    fn update_project_spec_repo(env: Env, repo: String);

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
    /// must be `target`'s admin. Errors with `NotOurContract` when `target`
    /// is not part of this project.
    fn upgrade_contract(
        env: Env,
        target: Address,
        new_wasm_hash: BytesN<32>,
        new_version: String,
    ) -> Result<(), ProjectRootError>;
    /// Forward `propose_admin(new_admin)` to `target`. ProjectRoot must be
    /// `target`'s admin. Use this to begin rotating the admin of a downstream
    /// contract away from ProjectRoot. Rotating a handler's admin away does
    /// **not** untrack it — call `unregister_handler` explicitly for that.
    /// Errors with `NotOurContract` when `target` is not part of this project.
    fn propose_contract_admin(
        env: Env,
        target: Address,
        new_admin: Address,
    ) -> Result<(), ProjectRootError>;
    /// Forward `accept_admin()` to `target`. ProjectRoot must be `target`'s
    /// pending admin. Use this to take over admin of a downstream contract;
    /// accepting a handler also records it in the tracked handler set (emitting
    /// `HandlerRegistered`). Errors with `NotOurContract` when `target` is not
    /// part of this project.
    fn accept_contract_admin(env: Env, target: Address) -> Result<(), ProjectRootError>;

    // ── Handler set management ─────────────────────────────────────────

    /// Add `handler` to this project's tracked handler set. Admin-only.
    /// `handler` must be a handler that reports this project's verification
    /// contract (`NotOurContract` otherwise; `NotAHandler` if it is the
    /// security or verification contract). Idempotent — re-registering an
    /// already-tracked handler is a no-op. Emits `HandlerRegistered` on a
    /// first registration.
    ///
    /// This is the canonical way to track a handler that was deployed with
    /// ProjectRoot already set as its admin: deploy the handler, then call
    /// `register_handler`. (A handler whose admin is handed over via the
    /// propose/`accept_contract_admin` dance is tracked automatically.)
    fn register_handler(env: Env, handler: Address) -> Result<(), ProjectRootError>;

    /// Remove `handler` from the tracked set. Admin-only. Idempotent — a no-op
    /// if `handler` isn't tracked. Emits `HandlerRemoved` when it actually
    /// removes an entry. This is the only way a handler leaves the set;
    /// rotating its admin away does not.
    fn unregister_handler(env: Env, handler: Address);

    // Queries
    fn security_contract(env: Env) -> Address;
    fn verification_contract(env: Env) -> Address;
    fn project_spec_repo(env: Env) -> String;
    /// Returns which interface is used by security_contract and verification_contract
    fn verification_type(env: Env) -> VerificationType;
    /// Returns the handler contracts this project currently tracks. A handler
    /// joins the set via `register_handler` (or implicitly when its admin is
    /// taken over by `accept_contract_admin`) and leaves only via
    /// `unregister_handler`. Empty until the first handler is registered.
    fn list_handlers(env: Env) -> Vec<Address>;
}
