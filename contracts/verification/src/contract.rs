use soroban_sdk::{Address, Bytes, BytesN, Env, String, Vec, contract, contractimpl};
pub use warpdrive_shared::VerifyError;

use crate::security_client::SecurityClient;

use crate::storage;
use crate::utils::{self, PubKey};

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

    /// Checks a single signature and returns the signer's weight if valid.
    /// Does NOT check against the threshold — use `verify` for full multi-sig validation.
    pub fn check_one(
        env: Env,
        envelope: Bytes,
        signature: BytesN<65>,
        signer_pubkey: PubKey,
    ) -> Result<u64, VerifyError> {
        if !utils::is_valid_signature(&env, &envelope, &signature, &signer_pubkey) {
            return Err(VerifyError::InvalidSignature);
        }

        let security_addr = storage::get_security_contract(&env);
        let security = SecurityClient::new(&env, &security_addr);

        let weight = security.get_signer_weight(&signer_pubkey);
        if weight == 0 {
            return Err(VerifyError::SignerNotRegistered);
        }

        Ok(weight)
    }

    pub fn verify(
        env: Env,
        envelope: Bytes,
        signatures: Vec<BytesN<65>>,
        signer_pubkeys: Vec<PubKey>,
        reference_block: u32,
    ) -> Result<(), VerifyError> {
        if signatures.is_empty() {
            return Err(VerifyError::EmptySignatures);
        }

        if signatures.len() != signer_pubkeys.len() {
            return Err(VerifyError::LengthMismatch);
        }

        // Batch-fetch all signer weights and required weight in two cross-contract calls
        // instead of one per signer.
        let security_addr = storage::get_security_contract(&env);
        let security = SecurityClient::new(&env, &security_addr);
        let weights = security.get_signer_weights_at(&signer_pubkeys, &reference_block);
        let required = security.required_weight_at(&reference_block);

        let mut total_weight: u64 = 0;
        let mut prev_pubkey: Option<PubKey> = None;

        for i in 0..signatures.len() {
            let sig = signatures.get(i).unwrap();
            let pubkey = signer_pubkeys.get(i).unwrap();

            // Enforce strict ascending order of pubkeys (no duplicates)
            if let Some(ref prev) = prev_pubkey
                && pubkey.to_array() <= prev.to_array()
            {
                return Err(VerifyError::SignersNotOrdered);
            }
            prev_pubkey = Some(pubkey.clone());

            if !utils::is_valid_signature(&env, &envelope, &sig, &pubkey) {
                return Err(VerifyError::InvalidSignature);
            }

            let weight = weights.get(i).unwrap();
            if weight == 0 {
                return Err(VerifyError::SignerNotRegistered);
            }

            total_weight = total_weight.checked_add(weight).expect("weight overflow");
        }

        if total_weight < required {
            return Err(VerifyError::InsufficientWeight);
        }

        Ok(())
    }
}
