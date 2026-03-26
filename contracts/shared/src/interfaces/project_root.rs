use soroban_sdk::{Address, BytesN, Env, String, contractevent};

// ── Events ───────────────────────────────────────────────────────────

#[contractevent]
pub struct ProjectRootUpgraded {
    pub version: String,
}

impl ProjectRootUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

// ── Interface trait (compile-time contract conformance) ──────────────

pub trait ProjectRootInterface {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String);
    fn admin(env: Env) -> Address;
    fn pending_admin(env: Env) -> Option<Address>;
    fn propose_admin(env: Env, new_admin: Address);
    fn accept_admin(env: Env);
    fn version(env: Env) -> String;
}
