#![no_std]

mod contract;
pub mod storage;

pub use contract::ProjectRoot;
pub use contract::ProjectRootClient;

#[cfg(test)]
mod tests;
