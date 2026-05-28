use wasi_soroban_rs::xdr::{
    ContractId as XdrContractId, Hash, ScAddress, ScString, ScVal, ScVec, VecM,
};
use wasi_soroban_rs::{
    ClientContractConfigs, ContractId, IntoScVal, SorobanHelperError, SorobanTransactionResponse,
};

use crate::scval::symbol;
use crate::utils::{execute, query, unexpected};
use crate::warpdrive::WarpdriveClient;

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

    /// Admin-gated proxy: invoke `function` on `target` with `args`. The caller
    /// is responsible for encoding `args` to match `target`'s ABI for
    /// `function`. Returns the submitted-transaction response; the inner
    /// call's return value is in the response's XDR result.
    pub async fn forward(
        &mut self,
        target: ContractId,
        function: &str,
        args: Vec<ScVal>,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let forward_args = vec![
            ScVal::Address(ScAddress::Contract(XdrContractId(Hash(target.0)))),
            symbol(function)?,
            ScVal::Vec(Some(ScVec(VecM::try_from(args).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("forward args too long".to_string())
            })?))),
        ];
        execute(&mut self.client_configs, "forward", forward_args).await
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
