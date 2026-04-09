// This includes all standard methods for all WarpDrive contracts

use soroban_sdk::{Address, BytesN, Env, String, contractevent};

// ── Events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct ContractUpgraded {
    pub version: String,
}

impl ContractUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

#[contractevent]
pub struct AdminProposed {
    pub old_admin: Address,
    pub new_admin: Address,
}

impl AdminProposed {
    pub fn new(old_admin: Address, new_admin: Address) -> Self {
        Self {
            old_admin,
            new_admin,
        }
    }
}

#[contractevent]
pub struct AdminAccepted {
    pub new_admin: Address,
}

impl AdminAccepted {
    pub fn new(new_admin: Address) -> Self {
        Self { new_admin }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

/// These are standard to all warpdrive contracts, upgrade, admin, version queries.
/// Place them as one trait shared by all contracts to make it clearer which is custom logic and guarantee compatibility on these.
pub trait WarpDriveInterface {
    // State-changing methods
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String);
    fn propose_admin(env: Env, new_admin: Address);
    fn accept_admin(env: Env);

    // Queries
    fn admin(env: Env) -> Address;
    fn pending_admin(env: Env) -> Option<Address>;
    fn version(env: Env) -> String;
}
