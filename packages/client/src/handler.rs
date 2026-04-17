use soroban_rs::xdr::{
    BytesM, ContractId as XdrContractId, Hash, ScAddress, ScBytes, ScMap, ScMapEntry, ScString,
    ScSymbol, ScVal, ScVec, StringM, VecM,
};
use soroban_rs::{ContractId, IntoScVal, SorobanHelperError, SorobanTransactionResponse};

use crate::utils::{execute, query};

pub struct SignatureData {
    pub signers: Vec<[u8; 33]>,
    pub signatures: Vec<[u8; 65]>,
    pub reference_block: u32,
}

pub struct EthereumHandlerClient {
    client_configs: soroban_rs::ClientContractConfigs,
}

impl EthereumHandlerClient {
    pub fn new(client_configs: &soroban_rs::ClientContractConfigs) -> Self {
        Self {
            client_configs: client_configs.clone(),
        }
    }

    pub async fn verify_eth(
        &mut self,
        envelope_bytes: Vec<u8>,
        sig_data: SignatureData,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![
            bytes_to_scval(&envelope_bytes)?,
            sig_data_to_scval(&sig_data)?,
        ];
        execute(&mut self.client_configs, "verify_eth", args).await
    }

    pub async fn upgrade(
        &mut self,
        new_wasm_hash: [u8; 32],
        new_version: String,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![new_wasm_hash.into_val(), new_version.into_val()];
        execute(&mut self.client_configs, "upgrade", args).await
    }

    pub async fn propose_admin(&mut self, new_admin: ScAddress) -> Result<(), SorobanHelperError> {
        let args = vec![ScVal::Address(new_admin)];
        execute(&mut self.client_configs, "propose_admin", args).await?;
        Ok(())
    }

    pub async fn accept_admin(&mut self) -> Result<(), SorobanHelperError> {
        execute(&mut self.client_configs, "accept_admin", vec![]).await?;
        Ok(())
    }

    pub async fn admin(&self) -> Result<ScAddress, SorobanHelperError> {
        let res = query(&self.client_configs, "admin", vec![]).await?;
        if let ScVal::Address(address) = res {
            return Ok(address);
        }
        Err(unexpected(&res))
    }

    pub async fn pending_admin(&self) -> Result<Option<ScAddress>, SorobanHelperError> {
        let res = query(&self.client_configs, "pending_admin", vec![]).await?;
        match res {
            ScVal::Void => Ok(None),
            ScVal::Address(address) => Ok(Some(address)),
            other => Err(unexpected(&other)),
        }
    }

    pub async fn version(&self) -> Result<String, SorobanHelperError> {
        let res = query(&self.client_configs, "version", vec![]).await?;
        if let ScVal::String(ScString(ref s)) = res {
            return String::from_utf8(s.as_vec().clone()).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("version not utf-8".to_string())
            });
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

    pub async fn payload(&self, event_id: [u8; 20]) -> Result<Option<Vec<u8>>, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "payload",
            vec![bytes_to_scval(&event_id)?],
        )
        .await?;
        match res {
            ScVal::Void => Ok(None),
            ScVal::Bytes(ScBytes(ref bm)) => Ok(Some(bm.to_vec())),
            other => Err(unexpected(&other)),
        }
    }
}

fn unexpected(res: &ScVal) -> SorobanHelperError {
    SorobanHelperError::TransactionSimulationFailed(format!("Unexpected result: {:?}", res))
}

fn bytes_to_scval(b: &[u8]) -> Result<ScVal, SorobanHelperError> {
    let bm = BytesM::<{ u32::MAX }>::try_from(b)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("bytes too long".to_string()))?;
    Ok(ScVal::Bytes(ScBytes::from(bm)))
}

fn vec_to_scval(v: Vec<ScVal>) -> Result<ScVal, SorobanHelperError> {
    let vm = VecM::try_from(v)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("vec too long".to_string()))?;
    Ok(ScVal::Vec(Some(ScVec::from(vm))))
}

fn symbol(key: &str) -> Result<ScVal, SorobanHelperError> {
    let sm = StringM::<32>::try_from(key)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("symbol too long".to_string()))?;
    Ok(ScVal::Symbol(ScSymbol(sm)))
}

fn sig_data_to_scval(sig: &SignatureData) -> Result<ScVal, SorobanHelperError> {
    let signers: Vec<ScVal> = sig
        .signers
        .iter()
        .map(|s| bytes_to_scval(s))
        .collect::<Result<_, _>>()?;
    let signatures: Vec<ScVal> = sig
        .signatures
        .iter()
        .map(|s| bytes_to_scval(s))
        .collect::<Result<_, _>>()?;

    // Contract struct ScVal::Map keys must be sorted alphabetically by field name.
    let entries = vec![
        ScMapEntry {
            key: symbol("reference_block")?,
            val: ScVal::U32(sig.reference_block),
        },
        ScMapEntry {
            key: symbol("signatures")?,
            val: vec_to_scval(signatures)?,
        },
        ScMapEntry {
            key: symbol("signers")?,
            val: vec_to_scval(signers)?,
        },
    ];
    let vm = VecM::try_from(entries)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("map too long".to_string()))?;
    Ok(ScVal::Map(Some(ScMap(vm))))
}
