#![no_std]

mod contract;
pub mod storage;
pub mod utils;

pub use contract::Verification;
pub use contract::VerificationClient;
pub use warpdrive_shared::VerifyError;

#[cfg(test)]
mod tests;
