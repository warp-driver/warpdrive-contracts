use soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress, ScVal};
use soroban_rs::{
    ClientContractConfigs, ContractId, SorobanHelperError, SorobanTransactionResponse,
};

use crate::scval::IntoScValExt;
use crate::utils::{execute, query, unexpected};
use crate::warpdrive::WarpdriveClient;

pub struct Secp256k1VerificationClient {
    client_configs: ClientContractConfigs,
}

impl WarpdriveClient for Secp256k1VerificationClient {
    fn get_client_configs(&self) -> &ClientContractConfigs {
        &self.client_configs
    }

    fn mut_client_configs(&mut self) -> &mut ClientContractConfigs {
        &mut self.client_configs
    }
}

impl Secp256k1VerificationClient {
    pub fn new(client_configs: ClientContractConfigs) -> Self {
        Self { client_configs }
    }

    pub async fn verify(
        &mut self,
        envelope: Vec<u8>,
        signatures: Vec<[u8; 65]>,
        signer_pubkeys: Vec<[u8; 33]>,
        reference_block: u32,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![
            envelope.into_val_ext()?,
            signatures.into_val_ext()?,
            signer_pubkeys.into_val_ext()?,
            ScVal::U32(reference_block),
        ];
        execute(&mut self.client_configs, "verify", args).await
    }

    pub async fn security_contract(&self) -> Result<ContractId, SorobanHelperError> {
        let res = query(&self.client_configs, "security_contract", vec![]).await?;
        if let ScVal::Address(ScAddress::Contract(XdrContractId(Hash(bytes)))) = res {
            return Ok(ContractId(bytes));
        }
        Err(unexpected(&res))
    }

    pub async fn required_weight(&self) -> Result<u64, SorobanHelperError> {
        let res = query(&self.client_configs, "required_weight", vec![]).await?;
        if let ScVal::U64(w) = res {
            return Ok(w);
        }
        Err(unexpected(&res))
    }

    pub async fn signer_weight(&self, signer_pubkey: [u8; 33]) -> Result<u64, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "signer_weight",
            vec![signer_pubkey.into_val_ext()?],
        )
        .await?;
        if let ScVal::U64(w) = res {
            return Ok(w);
        }
        Err(unexpected(&res))
    }

    /// `reference_block` is `Option<u32>` on the contract: `None` checks current weight,
    /// `Some(block)` checks the historical weight at that ledger.
    pub async fn check_one(
        &self,
        envelope: Vec<u8>,
        signature: [u8; 65],
        signer_pubkey: [u8; 33],
        reference_block: Option<u32>,
    ) -> Result<u64, SorobanHelperError> {
        let block_arg = match reference_block {
            Some(b) => ScVal::U32(b),
            None => ScVal::Void,
        };
        let args = vec![
            envelope.into_val_ext()?,
            signature.into_val_ext()?,
            signer_pubkey.into_val_ext()?,
            block_arg,
        ];
        let res = query(&self.client_configs, "check_one", args).await?;
        if let ScVal::U64(w) = res {
            return Ok(w);
        }
        Err(unexpected(&res))
    }
}
