use soroban_sdk::{Address, Bytes, BytesN, Env, String, contract, contracterror, contractimpl};

use crate::security_client::SecurityClient;

use crate::storage;
use crate::utils::{self, PubKey};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VerifyError {
    InvalidSignature = 1,
    SignerNotRegistered = 2,
    InsufficientWeight = 3,
}

#[contract]
pub struct Verification;

#[contractimpl]
impl Verification {
    pub fn __constructor(env: Env, admin: Address, security_contract: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_security_contract(&env, &security_contract);
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

    pub fn security_contract(env: Env) -> Address {
        storage::get_security_contract(&env)
    }

    pub fn required_weight(env: Env) -> u64 {
        let security_addr = storage::get_security_contract(&env);
        SecurityClient::new(&env, &security_addr).required_weight()
    }

    pub fn signer_weight(env: Env, signer_pubkey: PubKey) -> u64 {
        let security_addr = storage::get_security_contract(&env);
        SecurityClient::new(&env, &security_addr).get_signer_weight(&signer_pubkey)
    }

    pub fn verify(
        env: Env,
        envelope: Bytes,
        signature: BytesN<65>,
        signer_pubkey: PubKey,
    ) -> Result<(), VerifyError> {
        // 1. Verify the cryptographic signature
        if !utils::is_valid_signature(&env, &envelope, &signature, &signer_pubkey) {
            return Err(VerifyError::InvalidSignature);
        }

        // 2. Check signer is registered in the security contract
        let security_addr = storage::get_security_contract(&env);
        let security = SecurityClient::new(&env, &security_addr);

        let weight = security.get_signer_weight(&signer_pubkey);
        if weight == 0 {
            return Err(VerifyError::SignerNotRegistered);
        }

        // 3. Check if signer's weight meets the required threshold
        let required = security.required_weight();
        if weight < required {
            return Err(VerifyError::InsufficientWeight);
        }

        Ok(())
    }
}
