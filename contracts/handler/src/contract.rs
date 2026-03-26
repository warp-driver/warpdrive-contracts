use enum_repr::EnumRepr;
use soroban_sdk::{
    Address, Bytes, BytesN, Env, String, Vec, contract, contracterror, contractevent, contractimpl,
    contracttype, xdr::FromXdr,
};

use crate::envelope::Envelope as EthEnvelope;
use crate::storage;
use crate::verification_client::{VerificationClient, VerifyError};

/// Maximum age (in ledgers) allowed for a reference block.
const MAX_REFERENCE_BLOCK_AGE: u32 = 200;

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
#[EnumRepr(type = "u32")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HandlerError {
    // Errors from the handler itself
    EventAlreadySeen = 1,
    InvalidReferenceBlock = 2,
    InvalidEnvelope = 3,

    // Some numbers intentionally skipped...
    UnknownVerificationError = 20,
    // Mapped from VerifyError
    InvalidSignature = 21,
    SignerNotRegistered = 22,
    InsufficientWeight = 23,
    EmptySignatures = 24,
    LengthMismatch = 25,
    SignersNotOrdered = 26,
}

#[contractevent]
pub struct Verified {
    #[topic]
    pub event_id: BytesN<20>,
}

impl Verified {
    pub fn new(event_id: BytesN<20>) -> Self {
        Self { event_id }
    }
}

#[contractevent]
pub struct HandlerUpgraded {
    pub version: String,
}

impl HandlerUpgraded {
    pub fn new(version: String) -> Self {
        Self { version }
    }
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

/// Validates that `reference_block` is strictly in the past and within the allowed age window.
fn validate_reference_block(env: &Env, reference_block: u32) -> Result<(), HandlerError> {
    let current = env.ledger().sequence();
    if reference_block >= current {
        return Err(HandlerError::InvalidReferenceBlock);
    }
    if current - reference_block > MAX_REFERENCE_BLOCK_AGE {
        return Err(HandlerError::InvalidReferenceBlock);
    }
    Ok(())
}

/// Maps a `try_verify` result from the verification contract into a `HandlerError`.
fn map_verify_result(
    res: Result<
        Result<(), soroban_sdk::ConversionError>,
        Result<soroban_sdk::Error, soroban_sdk::InvokeError>,
    >,
) -> Result<(), HandlerError> {
    match res {
        Ok(Ok(())) => Ok(()),
        Ok(Err(_conversion)) => Err(HandlerError::UnknownVerificationError),
        Err(Ok(e)) => {
            if let Ok(soroban_sdk::xdr::ScError::Contract(code)) =
                soroban_sdk::xdr::ScError::try_from(e)
            {
                let err =
                    VerifyError::from_repr(code).ok_or(HandlerError::UnknownVerificationError)?;
                Err(HandlerError::from(err))
            } else {
                Err(HandlerError::UnknownVerificationError)
            }
        }
        Err(Err(_invoke_err)) => Err(HandlerError::UnknownVerificationError),
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
    }

    pub fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        storage::set_version(&env, &new_version);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        HandlerUpgraded::new(new_version).publish(&env);
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

    pub fn payload(env: Env, event_id: BytesN<20>) -> Option<Bytes> {
        storage::get_payload(&env, event_id)
    }

    /// Verifies the packet, assuming the envelope is ABI-encoded (Ethereum format) for compatibility.
    ///
    /// No caller authorization is required — this is intentional. Security is enforced entirely
    /// through cryptographic signature verification of the envelope contents.
    pub fn verify_eth(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        validate_reference_block(&env, sig_data.reference_block)?;

        // Parse the ABI-encoded envelope
        let envelope =
            EthEnvelope::abi_decode_from(&envelope_bytes).ok_or(HandlerError::InvalidEnvelope)?;
        let event_id = BytesN::from_array(&env, &envelope.eventId.0);

        // Check for duplicate event
        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        // Verify signatures via the verification contract
        let verification_addr = storage::get_verification_contract(&env);
        let verification = VerificationClient::new(&env, &verification_addr);
        let res = verification.try_verify(
            &envelope_bytes,
            &sig_data.signatures,
            &sig_data.signers,
            &sig_data.reference_block,
        );
        map_verify_result(res)?;

        // Mark event as seen
        storage::mark_event_seen(&env, &event_id);

        // Save payload
        let payload = Bytes::from_slice(&env, envelope.payload.as_ref());
        storage::save_payload(&env, event_id.clone(), payload);

        Verified::new(event_id).publish(&env);

        Ok(())
    }

    /// Verifies the packet, assuming the envelope is XDR-encoded (Soroban native format).
    ///
    /// No caller authorization is required — this is intentional. Security is enforced entirely
    /// through cryptographic signature verification of the envelope contents.
    pub fn verify_xlm(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError> {
        validate_reference_block(&env, sig_data.reference_block)?;

        // Parse XDR bytes into the typed envelope
        let envelope = XlmEnvelope::from_xdr(&env, &envelope_bytes)
            .map_err(|_| HandlerError::InvalidEnvelope)?;
        let event_id = envelope.event_id;

        // Check for duplicate event
        if storage::is_event_seen(&env, &event_id) {
            return Err(HandlerError::EventAlreadySeen);
        }

        // Verify signatures against the raw envelope bytes (what was actually signed)
        let verification_addr = storage::get_verification_contract(&env);
        let verification = VerificationClient::new(&env, &verification_addr);
        let res = verification.try_verify(
            &envelope_bytes,
            &sig_data.signatures,
            &sig_data.signers,
            &sig_data.reference_block,
        );
        map_verify_result(res)?;

        // Mark event as seen
        storage::mark_event_seen(&env, &event_id);

        // Save payload
        storage::save_payload(&env, event_id.clone(), envelope.payload);

        Verified::new(event_id).publish(&env);

        Ok(())
    }
}
