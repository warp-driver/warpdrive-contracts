use soroban_sdk::{
    Address, Bytes, BytesN, Env, String, Vec, contract, contracterror, contractimpl, contracttype,
};

use crate::envelope::Envelope;
use crate::storage;

#[contracttype]
pub struct SignatureData {
    pub signers: Vec<BytesN<20>>,
    pub signatures: Vec<BytesN<65>>,
    pub reference_block: u32,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum HandlerError {
    EventAlreadySeen = 1,
    VerificationFailed = 2,
}

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

    pub fn verify(
        env: Env,
        envelope_bytes: Bytes,
        _sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        // Parse the ABI-encoded envelope
        let envelope = Envelope::abi_decode_from(&envelope_bytes);
        let event_id = BytesN::from_array(&env, &envelope.eventId.0);

        // Check for duplicate event
        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        // Mark event as seen
        storage::mark_event_seen(&env, &event_id);

        // TODO: For each signer/signature pair, call the verification contract
        // to verify the envelope was signed correctly.
        // let verification_addr = storage::get_verification_contract(&env);
        // let verification = VerificationClient::new(&env, &verification_addr);
        // for i in 0..sig_data.signatures.len() {
        //     verification.verify(&envelope_bytes, &sig_data.signatures.get(i), &pubkey)?;
        // }

        Ok(())
    }
}
