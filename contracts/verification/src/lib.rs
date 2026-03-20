#![no_std]

mod contract;
pub mod envelope;
mod security_client;
pub mod storage;
pub mod utils;

pub use contract::Verification;
pub use contract::VerificationClient;
pub use contract::VerifyError;

#[cfg(test)]
mod tests;
