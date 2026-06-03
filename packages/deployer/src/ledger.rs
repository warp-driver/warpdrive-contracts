//! `get-ledger`: fetch the latest ledger sequence over JSON-RPC.

use wasi_soroban_rs::wasi_stellar_rpc_client::Client;

use crate::error::{DeployerError, Result};

/// Return the current ledger sequence from the configured RPC.
pub async fn get_latest_ledger(rpc_url: &str) -> Result<u32> {
    let client =
        Client::new(rpc_url).map_err(|e| DeployerError::Http(format!("rpc client: {e}")))?;
    let resp = client
        .get_latest_ledger()
        .await
        .map_err(|e| DeployerError::Http(format!("getLatestLedger failed: {e}")))?;
    Ok(resp.sequence)
}
