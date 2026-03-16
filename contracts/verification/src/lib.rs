#![no_std]

mod contract;
pub mod storage;
pub mod utils;

pub use contract::Verification;
pub use contract::VerificationClient;

#[cfg(test)]
mod tests;
