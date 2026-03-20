use soroban_sdk::{Address, BytesN, Env, String, contract, contractimpl};

use crate::storage;

#[contract]
pub struct Handler;

#[contractimpl]
impl Handler {
    pub fn __constructor(env: Env, admin: Address, verification_contract: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_verification_contract(&env, &verification_contract);
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

    pub fn verification_contract(env: Env) -> Address {
        storage::get_verification_contract(&env)
    }
}
