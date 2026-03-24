use soroban_sdk::{Address, BytesN, Env, String, Vec, contract, contractimpl};

use crate::storage::{self, PubKey, SignerInfo, Threshold};

#[contract]
pub struct Security;

#[contractimpl]
impl Security {
    pub fn __constructor(
        env: Env,
        admin: Address,
        threshold_numerator: u64,
        threshold_denominator: u64,
    ) {
        assert!(threshold_denominator > 0, "denominator must be > 0");
        assert!(
            threshold_numerator <= threshold_denominator,
            "numerator must be <= denominator"
        );
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_threshold(
            &env,
            &Threshold {
                numerator: threshold_numerator,
                denominator: threshold_denominator,
            },
        );
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

    pub fn set_threshold(env: Env, numerator: u64, denominator: u64) {
        storage::get_admin(&env).require_auth();
        assert!(denominator > 0, "denominator must be > 0");
        assert!(numerator <= denominator, "numerator must be <= denominator");
        storage::set_threshold(
            &env,
            &Threshold {
                numerator,
                denominator,
            },
        );
    }

    pub fn threshold_numerator(env: Env) -> u64 {
        storage::get_threshold(&env).numerator
    }

    pub fn threshold_denominator(env: Env) -> u64 {
        storage::get_threshold(&env).denominator
    }

    pub fn required_weight(env: Env) -> u64 {
        let total = storage::get_total_weight(&env);
        let threshold = storage::get_threshold(&env);
        ((total as u128) * (threshold.numerator as u128) / (threshold.denominator as u128)) as u64
    }
}
