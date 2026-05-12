use soroban_sdk::{
    Address, Bytes, BytesN, Env, Vec, contractclient, contracterror, contractevent, contracttype,
};

use super::verification::VerifyError;
use super::warpdrive::WarpDriveInterface;

// ── Error ────────────────────────────────────────────────────────────

// Namespacing: Handler errors are from 500-599

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum HandlerError {
    // Errors from the handler itself
    EventAlreadySeen = 501,
    InvalidReferenceBlock = 502,
    InvalidEnvelope = 503,
    // These are unknown errors when calling the verification contract
    UnknownVerificationError = 504,
    OtherInvocationError = 505,

    // Some numbers intentionally skipped...
    // Mapped from VerifyError (use same enum values from their space)
    InvalidSignature = 301,
    SignerNotRegistered = 302,
    InsufficientWeight = 303,
    EmptySignatures = 304,
    LengthMismatch = 305,
    SignersNotOrdered = 306,
    ZeroRequiredWeight = 307,
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
            VerifyError::ZeroRequiredWeight => HandlerError::ZeroRequiredWeight,
        }
    }
}

// ── Secp256k1 types ─────────────────────────────────────────────────

#[contracttype]
pub struct SignatureData {
    pub signers: Vec<BytesN<33>>,
    pub signatures: Vec<BytesN<65>>,
    pub reference_block: u32,
}

// ── Ed25519 types ───────────────────────────────────────────────────

#[contracttype]
pub struct Ed25519SignatureData {
    pub signers: Vec<BytesN<32>>,
    pub signatures: Vec<BytesN<64>>,
    pub reference_block: u32,
}

// ── Shared types ────────────────────────────────────────────────────

#[contracttype]
pub struct XlmEnvelope {
    pub event_id: BytesN<20>,
    pub ordering: BytesN<12>,
    pub payload: Bytes,
}

mod xlm_envelope_impl {
    use alloc::vec::Vec;
    use soroban_sdk::{
        Bytes, BytesN, Env,
        xdr::{FromXdr, ToXdr},
    };

    use super::{HandlerError, XlmEnvelope};

    impl XlmEnvelope {
        pub fn new(
            env: Option<&Env>,
            payload: Vec<u8>,
            event_id: [u8; 20],
            ordering: [u8; 12],
        ) -> Self {
            let env = env.cloned().unwrap_or_default();
            Self {
                event_id: BytesN::from_array(&env, &event_id),
                ordering: BytesN::from_array(&env, &ordering),
                payload: Bytes::from_slice(&env, &payload),
            }
        }

        pub fn encode(&self, env: Option<&Env>) -> Vec<u8> {
            // When the caller doesn't supply an env, reuse the one captured in
            // self.payload — every `Env::default()` is a separate host, so a
            // fresh one here wouldn't recognize the guest objects in `self`.
            let env = env.unwrap_or_else(|| self.payload.env());
            self.to_xdr(env).iter().collect()
        }

        pub fn decode(env: Option<&Env>, bytes: &[u8]) -> Result<Self, HandlerError> {
            let env = env.cloned().unwrap_or_default();
            let buf = Bytes::from_slice(&env, bytes);
            <XlmEnvelope as FromXdr>::from_xdr(&env, &buf)
                .map_err(|_| HandlerError::InvalidEnvelope)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use alloc::vec;

        const EVENT_ID: [u8; 20] = [
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0x00, 0x01, 0x02, 0x03, 0x04,
        ];
        const ORDERING: [u8; 12] = [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab,
        ];

        fn assert_fields(envelope: &XlmEnvelope, payload: &[u8]) {
            assert_eq!(envelope.event_id.to_array(), EVENT_ID);
            assert_eq!(envelope.ordering.to_array(), ORDERING);
            let got: Vec<u8> = envelope.payload.iter().collect();
            assert_eq!(got.as_slice(), payload);
        }

        #[test]
        fn roundtrip_with_no_env() {
            let payload = vec![0xde, 0xad, 0xbe, 0xef];
            let bytes = XlmEnvelope::new(None, payload.clone(), EVENT_ID, ORDERING).encode(None);
            let decoded = XlmEnvelope::decode(None, &bytes).expect("decode");
            assert_fields(&decoded, &payload);
        }

        #[test]
        fn roundtrip_with_explicit_env() {
            let env = Env::default();
            let payload = vec![0x01, 0x02, 0x03, 0x04, 0x05];
            let bytes = XlmEnvelope::new(Some(&env), payload.clone(), EVENT_ID, ORDERING)
                .encode(Some(&env));
            let decoded = XlmEnvelope::decode(Some(&env), &bytes).expect("decode");
            assert_fields(&decoded, &payload);
        }

        #[test]
        fn roundtrip_with_empty_payload() {
            let bytes = XlmEnvelope::new(None, Vec::new(), EVENT_ID, ORDERING).encode(None);
            let decoded = XlmEnvelope::decode(None, &bytes).expect("decode");
            assert_fields(&decoded, &[]);
        }
    }
}

/// Inner payload carried inside an `XlmEnvelope.payload`. Mirrors the
/// CosmWasm `MessageWithId` so the same logical struct is exchanged across
/// chains, but uses Soroban's native XDR encoding here.
#[contracttype]
pub struct MessageWithId {
    pub trigger_id: u64,
    pub message: Bytes,
}

// ── Events ───────────────────────────────────────────────────────────

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
pub struct Triggered {
    #[topic]
    pub trigger_id: u64,
    pub event_id: BytesN<20>,
}

impl Triggered {
    pub fn new(trigger_id: u64, event_id: BytesN<20>) -> Self {
        Self {
            trigger_id,
            event_id,
        }
    }
}

// ── Interface traits (compile-time contract conformance) ────────────

#[contractclient(name = "EthereumHandlerClient")]
pub trait EthereumHandlerInterface: WarpDriveInterface {
    // State Changing Operations (if verification succeeds)
    fn verify_eth(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: SignatureData,
    ) -> Result<(), HandlerError>;

    // Queries
    fn verification_contract(env: Env) -> Address;
    fn payload(env: Env, event_id: BytesN<20>) -> Option<Bytes>;
}

#[contractclient(name = "StellarHandlerClient")]
pub trait StellarHandlerInterface: WarpDriveInterface {
    // State Changing Operations (if verification succeeds)
    fn verify_xlm(
        env: Env,
        envelope_bytes: Bytes,
        sig_data: Ed25519SignatureData,
    ) -> Result<(), HandlerError>;

    // Queries
    fn verification_contract(env: Env) -> Address;
    fn payload(env: Env, event_id: BytesN<20>) -> Option<Bytes>;
}
