#![no_std]

mod contract;
pub mod storage;

pub use contract::Security;
pub use contract::SecurityClient;
pub use warpdrive_shared::interfaces::security::SecurityError;

#[cfg(test)]
mod tests;
