use soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress, ScBytes, ScVal};
use soroban_rs::{
    ClientContractConfigs, ContractId, SorobanHelperError, SorobanTransactionResponse,
};

use crate::scval::{IntoScValExt, struct_map};
use crate::utils::{execute, query, unexpected};
use crate::warpdrive::WarpdriveClient;

pub struct EthereumHandlerClient {
    client_configs: ClientContractConfigs,
}

impl WarpdriveClient for EthereumHandlerClient {
    fn get_client_configs(&self) -> &ClientContractConfigs {
        &self.client_configs
    }

    fn mut_client_configs(&mut self) -> &mut ClientContractConfigs {
        &mut self.client_configs
    }
}

impl EthereumHandlerClient {
    pub fn new(client_configs: ClientContractConfigs) -> Self {
        Self { client_configs }
    }

    pub async fn verify_eth(
        &mut self,
        envelope_bytes: Vec<u8>,
        sig_data: SignatureData,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![envelope_bytes.into_val_ext()?, sig_data.into_scval()?];
        execute(&mut self.client_configs, "verify_eth", args).await
    }

    // TODO: I only see Address return value from handler contract, but I know it must be another contract. Is there a way to asset this in contract code, not just client?
    pub async fn verification_contract(&self) -> Result<ContractId, SorobanHelperError> {
        let res = query(&self.client_configs, "verification_contract", vec![]).await?;
        if let ScVal::Address(ScAddress::Contract(XdrContractId(Hash(bytes)))) = res {
            return Ok(ContractId(bytes));
        }
        Err(unexpected(&res))
    }

    pub async fn payload(&self, event_id: [u8; 20]) -> Result<Option<Vec<u8>>, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "payload",
            vec![event_id.into_val_ext()?],
        )
        .await?;
        match res {
            ScVal::Void => Ok(None),
            ScVal::Bytes(ScBytes(ref bm)) => Ok(Some(bm.to_vec())),
            other => Err(unexpected(&other)),
        }
    }
}

pub struct SignatureData {
    pub signers: Vec<[u8; 33]>,
    pub signatures: Vec<[u8; 65]>,
    pub reference_block: u32,
}

impl SignatureData {
    pub(crate) fn into_scval(self) -> Result<ScVal, SorobanHelperError> {
        // Contract struct ScVal::Map keys must be sorted alphabetically.
        struct_map(vec![
            ("reference_block", ScVal::U32(self.reference_block)),
            ("signatures", self.signatures.into_val_ext()?),
            ("signers", self.signers.into_val_ext()?),
        ])
    }
}
