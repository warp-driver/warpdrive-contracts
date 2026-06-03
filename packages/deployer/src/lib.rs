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

use wasi_soroban_rs::{SorobanHelperError, SorobanTransactionResponse};

use crate::error::{DeployerError, Result};

/// The submitted transaction's hash. A confirmed submission always carries one,
/// so a missing hash means the response wasn't a successful submission — this
/// errors rather than returning a placeholder that downstream code would read
/// as success.
pub fn tx_hash(resp: &SorobanTransactionResponse) -> Result<String> {
    resp.response.tx_hash.clone().ok_or_else(|| {
        DeployerError::Soroban(SorobanHelperError::TransactionFailed(
            "submitted transaction response carried no tx hash".to_string(),
        ))
    })
}
