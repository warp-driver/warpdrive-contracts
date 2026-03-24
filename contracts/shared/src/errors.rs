use enum_repr::EnumRepr;
use soroban_sdk::contracterror;

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
