#![no_std]

mod contract;
pub mod envelope;
pub mod storage;
mod verification_client;

pub use contract::Handler;
pub use contract::HandlerClient;
pub use contract::HandlerError;
pub use contract::SignatureData;

#[cfg(test)]
mod tests;
