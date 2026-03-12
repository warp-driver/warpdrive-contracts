use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, String};

use crate::storage;

#[contract]
pub struct Security;

#[contractimpl]
impl Security {
    pub fn __constructor(env: Env, admin: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        storage::set_version(&env, &new_version);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
    }

    pub fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    pub fn version(env: Env) -> String {
        storage::get_version(&env)
    }

    pub fn count(env: Env) -> u64 {
        storage::get_count(&env)
    }

    pub fn increment(env: Env) {
        let admin = storage::get_admin(&env);
        admin.require_auth();
        storage::inc_count(&env, None)
    }
}
