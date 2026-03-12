#![no_std]

mod contract;
pub mod storage;

pub use contract::Security;
pub use contract::SecurityClient;

#[cfg(test)]
mod tests;
