use soroban_sdk::{Address, BytesN, Env, String, contract, contractevent, contractimpl};

use crate::storage;

#[contractevent]
pub struct ProjectRootUpgraded {
    pub version: String,
}

impl ProjectRootUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
}

#[contract]
pub struct ProjectRoot;

#[contractimpl]
impl ProjectRoot {
    pub fn __constructor(env: Env, admin: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        storage::set_version(&env, &new_version);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        ProjectRootUpgraded::new(new_version).publish(&env);
    }

    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    pub fn pending_admin(env: Env) -> Option<Address> {
        warpdrive_shared::admin::pending(&env)
    }

    pub fn propose_admin(env: Env, new_admin: Address) {
        warpdrive_shared::admin::propose(&env, &storage::get_admin(&env), new_admin);
    }

    pub fn accept_admin(env: Env) {
        let new_admin = warpdrive_shared::admin::accept(&env);
        storage::set_admin(&env, &new_admin);
    }

    pub fn version(env: Env) -> String {
        storage::get_version(&env)
    }
}
