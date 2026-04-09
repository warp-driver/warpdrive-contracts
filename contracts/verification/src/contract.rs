use soroban_sdk::{Address, Bytes, BytesN, Env, String, Vec, contract, contractimpl};

use warpdrive_shared::interfaces::{
    PubKey,
    security::SecurityClient,
    verification::{VerificationInterface, VerifyError},
    warpdrive::{ContractUpgraded, WarpDriveInterface},
};

use crate::storage;
use crate::utils;

#[contract]
pub struct Verification;

#[contractimpl]
impl Verification {
    pub fn __constructor(env: Env, admin: Address, security_contract: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_security_contract(&env, &security_contract);
    }
}

#[contractimpl]
impl WarpDriveInterface for Verification {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        storage::set_version(&env, &new_version);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        ContractUpgraded::new(new_version).publish(&env);
    }

    fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    fn pending_admin(env: Env) -> Option<Address> {
        warpdrive_shared::admin::pending(&env)
    }

    fn propose_admin(env: Env, new_admin: Address) {
        warpdrive_shared::admin::propose(&env, &storage::get_admin(&env), new_admin);
    }

    fn accept_admin(env: Env) {
        let new_admin = warpdrive_shared::admin::accept(&env);
        storage::set_admin(&env, &new_admin);
    }

    fn version(env: Env) -> String {
        storage::get_version(&env)
    }
}

#[contractimpl]
impl VerificationInterface for Verification {
    fn security_contract(env: Env) -> Address {
        storage::get_security_contract(&env)
    }

    fn required_weight(env: Env) -> u64 {
        let security_addr = storage::get_security_contract(&env);
        SecurityClient::new(&env, &security_addr).required_weight()
    }

    fn signer_weight(env: Env, signer_pubkey: PubKey) -> u64 {
        let security_addr = storage::get_security_contract(&env);
        SecurityClient::new(&env, &security_addr).get_signer_weight(&signer_pubkey)
    }

    fn check_one(
        env: Env,
        envelope: Bytes,
        signature: BytesN<65>,
        signer_pubkey: PubKey,
        reference_block: Option<u32>,
    ) -> Result<u64, VerifyError> {
        if !utils::is_valid_signature(&env, &envelope, &signature, &signer_pubkey) {
            return Err(VerifyError::InvalidSignature);
        }

        let security_addr = storage::get_security_contract(&env);
        let security = SecurityClient::new(&env, &security_addr);

        let weight = match reference_block {
            Some(block) => security.get_signer_weight_at(&signer_pubkey, &block),
            None => security.get_signer_weight(&signer_pubkey),
        };
        if weight == 0 {
            return Err(VerifyError::SignerNotRegistered);
        }

        Ok(weight)
    }

    fn verify(
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

        let security_addr = storage::get_security_contract(&env);
        let security = SecurityClient::new(&env, &security_addr);
        let weights = security.get_signer_weights_at(&signer_pubkeys, &reference_block);
        let required = security.required_weight_at(&reference_block);

        if required == 0 {
            return Err(VerifyError::ZeroRequiredWeight);
        }

        let mut total_weight: u64 = 0;
        let mut prev_pubkey: Option<PubKey> = None;

        for i in 0..signatures.len() {
            let sig = signatures.get(i).unwrap();
            let pubkey = signer_pubkeys.get(i).unwrap();

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
