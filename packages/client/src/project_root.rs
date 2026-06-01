use std::str::FromStr;

use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress, ScString, ScVal};
use wasi_soroban_rs::{
    ClientContractConfigs, ContractId, IntoScVal, SorobanHelperError, SorobanTransactionResponse,
};

use crate::scval::IntoScValExt;
use crate::utils::{execute, query, unexpected};
use crate::warpdrive::WarpdriveClient;

fn contract_address(id: ContractId) -> ScVal {
    ScVal::Address(ScAddress::Contract(XdrContractId(Hash(id.0))))
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VerificationType {
    Ethereum = 1,
    Stellar = 2,
}

pub struct ProjectRootClient {
    client_configs: ClientContractConfigs,
}

impl WarpdriveClient for ProjectRootClient {
    fn get_client_configs(&self) -> &ClientContractConfigs {
        &self.client_configs
    }

    fn mut_client_configs(&mut self) -> &mut ClientContractConfigs {
        &mut self.client_configs
    }
}

impl ProjectRootClient {
    pub fn new(client_configs: ClientContractConfigs) -> Self {
        Self { client_configs }
    }

    pub async fn update_project_spec_repo(
        &mut self,
        repo: String,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        execute(
            &mut self.client_configs,
            "update_project_spec_repo",
            vec![repo.into_val()],
        )
        .await
    }

    // ── Typed helpers: registered security_contract ────────────────────

    /// Forward `add_signer(key, weight)` to the registered security contract,
    /// secp256k1 (33-byte key) variant.
    pub async fn add_secp256k1_signer(
        &mut self,
        key: [u8; 33],
        weight: u64,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?, ScVal::U64(weight)];
        execute(&mut self.client_configs, "add_secp256k1_signer", args).await
    }

    /// Forward `remove_signer(key)` to the registered security contract,
    /// secp256k1 (33-byte key) variant.
    pub async fn remove_secp256k1_signer(
        &mut self,
        key: [u8; 33],
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?];
        execute(&mut self.client_configs, "remove_secp256k1_signer", args).await
    }

    /// Forward `add_signer(key, weight)` to the registered security contract,
    /// ed25519 (32-byte key) variant.
    pub async fn add_ed25519_signer(
        &mut self,
        key: [u8; 32],
        weight: u64,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?, ScVal::U64(weight)];
        execute(&mut self.client_configs, "add_ed25519_signer", args).await
    }

    /// Forward `remove_signer(key)` to the registered security contract,
    /// ed25519 (32-byte key) variant.
    pub async fn remove_ed25519_signer(
        &mut self,
        key: [u8; 32],
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?];
        execute(&mut self.client_configs, "remove_ed25519_signer", args).await
    }

    /// Forward `set_threshold(numerator, denominator)` to the registered
    /// security contract. Same signature for both schemes.
    pub async fn set_threshold(
        &mut self,
        numerator: u64,
        denominator: u64,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![ScVal::U64(numerator), ScVal::U64(denominator)];
        execute(&mut self.client_configs, "set_threshold", args).await
    }

    // ── Typed WarpDriveInterface forwarders (any target) ───────────────

    /// Forward `upgrade(new_wasm_hash, new_version)` to `target`. ProjectRoot
    /// must currently be `target`'s admin.
    pub async fn upgrade_contract(
        &mut self,
        target: ContractId,
        new_wasm_hash: [u8; 32],
        new_version: String,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![
            contract_address(target),
            new_wasm_hash.into_val(),
            new_version.into_val(),
        ];
        execute(&mut self.client_configs, "upgrade_contract", args).await
    }

    /// Forward `propose_admin(new_admin)` to `target`. Use this to start
    /// rotating the admin of a downstream contract away from ProjectRoot.
    pub async fn propose_contract_admin(
        &mut self,
        target: ContractId,
        new_admin: &str,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let new_admin = ScAddress::from_str(new_admin)?;
        let args = vec![contract_address(target), ScVal::Address(new_admin)];
        execute(&mut self.client_configs, "propose_contract_admin", args).await
    }

    /// Forward `accept_admin()` to `target`. ProjectRoot must currently be
    /// `target`'s pending admin.
    pub async fn accept_contract_admin(
        &mut self,
        target: ContractId,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![contract_address(target)];
        execute(&mut self.client_configs, "accept_contract_admin", args).await
    }

    pub async fn security_contract(&self) -> Result<ContractId, SorobanHelperError> {
        let res = query(&self.client_configs, "security_contract", vec![]).await?;
        if let ScVal::Address(ScAddress::Contract(XdrContractId(Hash(bytes)))) = res {
            return Ok(ContractId(bytes));
        }
        Err(unexpected(&res))
    }

    pub async fn verification_contract(&self) -> Result<ContractId, SorobanHelperError> {
        let res = query(&self.client_configs, "verification_contract", vec![]).await?;
        if let ScVal::Address(ScAddress::Contract(XdrContractId(Hash(bytes)))) = res {
            return Ok(ContractId(bytes));
        }
        Err(unexpected(&res))
    }

    pub async fn project_spec_repo(&self) -> Result<String, SorobanHelperError> {
        let res = query(&self.client_configs, "project_spec_repo", vec![]).await?;
        if let ScVal::String(ScString(ref s)) = res {
            return String::from_utf8(s.as_vec().clone()).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("project_spec_repo not utf-8".to_string())
            });
        }
        Err(unexpected(&res))
    }

    pub async fn verification_type(&self) -> Result<VerificationType, SorobanHelperError> {
        let res = query(&self.client_configs, "verification_type", vec![]).await?;
        match res {
            ScVal::U32(1) => Ok(VerificationType::Ethereum),
            ScVal::U32(2) => Ok(VerificationType::Stellar),
            other => Err(unexpected(&other)),
        }
    }
}
