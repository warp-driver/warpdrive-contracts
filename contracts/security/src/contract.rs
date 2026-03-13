use soroban_sdk::{Address, BytesN, Env, String, Vec, contract, contractimpl};

use crate::storage::{self, PubKey, SignerInfo};

#[contract]
pub struct Security;

#[contractimpl]
impl Security {
    pub fn __constructor(env: Env, admin: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::init_signers(&env);
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

    pub fn add_signer(env: Env, key: PubKey, weight: u64) {
        storage::get_admin(&env).require_auth();
        storage::add_signer(&env, key, weight);
    }

    pub fn remove_signer(env: Env, key: PubKey) {
        storage::get_admin(&env).require_auth();
        storage::remove_signer(&env, key);
    }

    pub fn get_total_weight(env: Env) -> u64 {
        storage::get_total_weight(&env)
    }

    pub fn get_signer_weight(env: Env, key: PubKey) -> u64 {
        storage::get_signer_weight(&env, key).unwrap_or(0)
    }

    pub fn list_signers(env: Env) -> Vec<SignerInfo> {
        storage::list_signers(&env)
    }
}
