use soroban_sdk::{
    Address, Bytes, BytesN, Env, String, Vec, contract, contracterror, contractimpl, contracttype,
    xdr::FromXdr,
};

use crate::envelope::Envelope as EthEnvelope;
use crate::storage;
use crate::verification_client::{VerificationClient, VerifyError};

#[contracttype]
pub struct SignatureData {
    pub signers: Vec<BytesN<33>>,
    pub signatures: Vec<BytesN<65>>,
    pub reference_block: u32,
}

#[contracttype]
pub struct XlmEnvelope {
    pub event_id: BytesN<20>,
    pub ordering: BytesN<12>,
    pub payload: Bytes,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum HandlerError {
    // Errors from the handler itself
    EventAlreadySeen = 1,

    // Some numbers intentionally skipped...
    UnknownVerificationError = 20,
    // Copied from VerifyError
    InvalidSignature = 21,
    SignerNotRegistered = 22,
    InsufficientWeight = 23,
    EmptySignatures = 24,
    LengthMismatch = 25,
    SignersNotOrdered = 26,
}

impl From<VerifyError> for HandlerError {
    fn from(value: VerifyError) -> Self {
        match value {
            VerifyError::InvalidSignature => HandlerError::InvalidSignature,
            VerifyError::SignerNotRegistered => HandlerError::SignerNotRegistered,
            VerifyError::InsufficientWeight => HandlerError::InsufficientWeight,
            VerifyError::EmptySignatures => HandlerError::EmptySignatures,
            VerifyError::LengthMismatch => HandlerError::LengthMismatch,
            VerifyError::SignersNotOrdered => HandlerError::SignersNotOrdered,
        }
    }
}

#[contract]
pub struct Handler;

#[contractimpl]
impl Handler {
    pub fn __constructor(env: Env, admin: Address, verification_contract: Address) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_verification_contract(&env, &verification_contract);
        storage::init_payloads(&env);
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

    pub fn payload(env: Env, event_id: BytesN<20>) -> Bytes {
        storage::get_payload(&env, event_id).unwrap()
    }

    /// This verifies the packet, assuming the envelope is abi-encoded (Ethereum format) for compatibility
    pub fn verify_eth(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        // Parse the ABI-encoded envelope
        let envelope = EthEnvelope::abi_decode_from(&envelope_bytes);
        let event_id = BytesN::from_array(&env, &envelope.eventId.0);

        // Check for duplicate event
        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        // Verify signatures via the verification contract.
        // Errors from the verification contract propagate directly as contract errors.
        let verification_addr = storage::get_verification_contract(&env);
        let verification = VerificationClient::new(&env, &verification_addr);
        let res = verification.try_verify(&envelope_bytes, &sig_data.signatures, &sig_data.signers);
        match res {
            Ok(_) => {}
            Err(Ok(e)) => {
                // Contract error from verification contract — extract the error code
                if let Ok(soroban_sdk::xdr::ScError::Contract(code)) =
                    soroban_sdk::xdr::ScError::try_from(e)
                {
                    let err = VerifyError::from_repr(code)
                        .ok_or(HandlerError::UnknownVerificationError)?;
                    return Err(HandlerError::from(err));
                }
                return Err(HandlerError::UnknownVerificationError);
            }
            Err(Err(_invoke_err)) => {
                return Err(HandlerError::UnknownVerificationError);
            }
        }

        // Mark event as seen
        storage::mark_event_seen(&env, &event_id);

        // Save payload
        let payload = Bytes::from_slice(&env, envelope.payload.as_ref());
        storage::save_payload(&env, event_id, payload);

        Ok(())
    }

    /// This verifies the packet, assuming the envelope is XDR-encoded (Soroban native format)
    pub fn verify_xlm(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        // Parse XDR bytes into the typed envelope
        let envelope = XlmEnvelope::from_xdr(&env, &envelope_bytes).expect("invalid XLM envelope");
        let event_id = envelope.event_id;

        // Check for duplicate event
        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        // Verify signatures against the raw envelope bytes (what was actually signed)
        let verification_addr = storage::get_verification_contract(&env);
        let verification = VerificationClient::new(&env, &verification_addr);
        let res = verification.try_verify(&envelope_bytes, &sig_data.signatures, &sig_data.signers);
        match res {
            Ok(_) => {}
            Err(Ok(e)) => {
                // Contract error from verification contract — extract the error code
                if let Ok(soroban_sdk::xdr::ScError::Contract(code)) =
                    soroban_sdk::xdr::ScError::try_from(e)
                {
                    let err = VerifyError::from_repr(code)
                        .ok_or(HandlerError::UnknownVerificationError)?;
                    return Err(HandlerError::from(err));
                }
                return Err(HandlerError::UnknownVerificationError);
            }
            Err(Err(_invoke_err)) => {
                return Err(HandlerError::UnknownVerificationError);
            }
        }

        // Mark event as seen
        storage::mark_event_seen(&env, &event_id);

        // Save payload
        storage::save_payload(&env, event_id, envelope.payload);

        Ok(())
    }
}
