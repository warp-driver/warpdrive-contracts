//! Network + wasm-dir configuration. The only place that knows how to turn
//! `RPC_URL` / `NETWORK_PASSPHRASE` into a soroban `Env`.

use std::path::PathBuf;

use wasi_soroban_rs::{Account, ClientContractConfigs, ContractId, Env, EnvConfigs};

use crate::error::{DeployerError, Result};

/// Default location the contract wasm is baked into the docker image.
pub const DEFAULT_WASM_DIR: &str = "/warpdrive/wasm";

/// Network coordinates needed to talk to a Soroban RPC and sign transactions.
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub rpc_url: String,
    pub network_passphrase: String,
}

impl NetworkConfig {
    pub fn new(rpc_url: String, network_passphrase: String) -> Self {
        Self {
            rpc_url,
            network_passphrase,
        }
    }

    /// Build a soroban `Env` from these coordinates.
    pub fn env(&self) -> Result<Env> {
        Env::new(EnvConfigs {
            rpc_url: self.rpc_url.clone(),
            network_passphrase: self.network_passphrase.clone(),
        })
        .map_err(DeployerError::from)
    }
}

/// Resolve the wasm directory: explicit `--wasm-dir`, else `WASM_DIR` env, else
/// the docker-image default. Never hardcodes a path at a call site.
pub fn resolve_wasm_dir(flag: Option<PathBuf>) -> PathBuf {
    flag.or_else(|| std::env::var_os("WASM_DIR").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_WASM_DIR))
}

/// Build the client config for invoking `contract_id` as `account`.
pub fn client_configs(
    net: &NetworkConfig,
    account: &Account,
    contract_id: ContractId,
) -> Result<ClientContractConfigs> {
    Ok(ClientContractConfigs {
        contract_id,
        env: net.env()?,
        source_account: account.clone(),
    })
}
