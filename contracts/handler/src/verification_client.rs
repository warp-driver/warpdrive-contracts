use enum_repr::EnumRepr;
use soroban_sdk::{Bytes, BytesN, Env, Vec, contractclient, contracterror};

type PubKey = BytesN<33>;

// FIXME: this is cut and pasted from verification/contracts.rs cuz I cannot import that file without cycles.
// Figure out a cleaner way to handle this
#[contracterror]
#[EnumRepr(type = "u32")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum VerifyError {
    InvalidSignature = 1,
    SignerNotRegistered = 2,
    InsufficientWeight = 3,
    EmptySignatures = 4,
    LengthMismatch = 5,
    SignersNotOrdered = 6,
}

#[contractclient(name = "VerificationClient")]
#[allow(dead_code)]
pub trait VerificationInterface {
    fn verify(env: Env, envelope: Bytes, signatures: Vec<BytesN<65>>, signer_pubkeys: Vec<PubKey>);
}
