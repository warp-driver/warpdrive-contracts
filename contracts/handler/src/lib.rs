#![no_std]

mod contract;
pub mod storage;

pub use contract::Handler;
pub use contract::HandlerClient;

#[cfg(test)]
mod tests;
