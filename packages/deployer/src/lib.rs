//! `warpdrive-deployer` library. Every subcommand is a thin wrapper over a
//! typed function in one of these modules; `main.rs` only parses argv and
//! prints. The typed functions never touch argv/stdout, so they're unit- and
//! integration-testable directly.

pub mod cli;
pub mod config;
pub mod deploy;
pub mod error;
pub mod governance;
pub mod identity;
pub mod ledger;
pub mod manifest;
pub mod project_root;
pub mod retry;
pub mod signers;

use wasi_soroban_rs::SorobanTransactionResponse;

/// Extract a printable transaction hash from a submitted-transaction response.
pub fn tx_hash(resp: &SorobanTransactionResponse) -> String {
    resp.response
        .tx_hash
        .clone()
        .unwrap_or_else(|| "<no tx hash>".to_string())
}
