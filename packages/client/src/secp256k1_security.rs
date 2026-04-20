use std::fmt;

use soroban_rs::xdr::{ScBytes, ScMap, ScMapEntry, ScVal, ScVec};
use soroban_rs::{ClientContractConfigs, SorobanHelperError, SorobanTransactionResponse};

use crate::scval::IntoScValExt;
use crate::utils::{execute, query, unexpected};
use crate::warpdrive::WarpdriveClient;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignerInfo {
    pub key: [u8; 33],
    pub weight: u64,
}

impl fmt::Display for SignerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pub_key: 0x")?;
        for byte in &self.key {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, ", weight: {}", self.weight)
    }
}

pub struct Secp256k1SecurityClient {
    client_configs: ClientContractConfigs,
}

impl WarpdriveClient for Secp256k1SecurityClient {
    fn get_client_configs(&self) -> &ClientContractConfigs {
        &self.client_configs
    }

    fn mut_client_configs(&mut self) -> &mut ClientContractConfigs {
        &mut self.client_configs
    }
}

impl Secp256k1SecurityClient {
    pub fn new(client_configs: ClientContractConfigs) -> Self {
        Self { client_configs }
    }

    // ── State-changing operations ───────────────────────────────────────

    pub async fn add_signer(
        &mut self,
        key: [u8; 33],
        weight: u64,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?, ScVal::U64(weight)];
        execute(&mut self.client_configs, "add_signer", args).await
    }

    pub async fn remove_signer(
        &mut self,
        key: [u8; 33],
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![key.into_val_ext()?];
        execute(&mut self.client_configs, "remove_signer", args).await
    }

    pub async fn set_threshold(
        &mut self,
        numerator: u64,
        denominator: u64,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![ScVal::U64(numerator), ScVal::U64(denominator)];
        execute(&mut self.client_configs, "set_threshold", args).await
    }

    // ── Queries ────────────────────────────────────────────────────────

    pub async fn get_total_weight(&self) -> Result<u64, SorobanHelperError> {
        let res = query(&self.client_configs, "get_total_weight", vec![]).await?;
        decode_u64(&res)
    }

    pub async fn get_total_weight_at(
        &self,
        reference_block: u32,
    ) -> Result<u64, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "get_total_weight_at",
            vec![ScVal::U32(reference_block)],
        )
        .await?;
        decode_u64(&res)
    }

    pub async fn get_signer_weight(&self, key: [u8; 33]) -> Result<u64, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "get_signer_weight",
            vec![key.into_val_ext()?],
        )
        .await?;
        decode_u64(&res)
    }

    pub async fn get_signer_weight_at(
        &self,
        key: [u8; 33],
        reference_block: u32,
    ) -> Result<u64, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "get_signer_weight_at",
            vec![key.into_val_ext()?, ScVal::U32(reference_block)],
        )
        .await?;
        decode_u64(&res)
    }

    pub async fn get_signer_weights(
        &self,
        keys: Vec<[u8; 33]>,
    ) -> Result<Vec<u64>, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "get_signer_weights",
            vec![keys.into_val_ext()?],
        )
        .await?;
        decode_u64_vec(&res)
    }

    pub async fn get_signer_weights_at(
        &self,
        keys: Vec<[u8; 33]>,
        reference_block: u32,
    ) -> Result<Vec<u64>, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "get_signer_weights_at",
            vec![keys.into_val_ext()?, ScVal::U32(reference_block)],
        )
        .await?;
        decode_u64_vec(&res)
    }

    pub async fn required_weight(&self) -> Result<u64, SorobanHelperError> {
        let res = query(&self.client_configs, "required_weight", vec![]).await?;
        decode_u64(&res)
    }

    pub async fn required_weight_at(
        &self,
        reference_block: u32,
    ) -> Result<u64, SorobanHelperError> {
        let res = query(
            &self.client_configs,
            "required_weight_at",
            vec![ScVal::U32(reference_block)],
        )
        .await?;
        decode_u64(&res)
    }

    pub async fn threshold_numerator(&self) -> Result<u64, SorobanHelperError> {
        let res = query(&self.client_configs, "threshold_numerator", vec![]).await?;
        decode_u64(&res)
    }

    pub async fn threshold_denominator(&self) -> Result<u64, SorobanHelperError> {
        let res = query(&self.client_configs, "threshold_denominator", vec![]).await?;
        decode_u64(&res)
    }

    pub async fn list_signers(&self) -> Result<Vec<SignerInfo>, SorobanHelperError> {
        let res = query(&self.client_configs, "list_signers", vec![]).await?;
        let ScVal::Vec(Some(ScVec(entries))) = &res else {
            return Err(unexpected(&res));
        };
        entries.iter().map(decode_signer_info).collect()
    }
}

fn decode_u64(res: &ScVal) -> Result<u64, SorobanHelperError> {
    if let ScVal::U64(w) = res {
        Ok(*w)
    } else {
        Err(unexpected(res))
    }
}

fn decode_u64_vec(res: &ScVal) -> Result<Vec<u64>, SorobanHelperError> {
    let ScVal::Vec(Some(ScVec(vm))) = res else {
        return Err(unexpected(res));
    };
    vm.iter().map(decode_u64).collect()
}

fn decode_signer_info(val: &ScVal) -> Result<SignerInfo, SorobanHelperError> {
    let ScVal::Map(Some(ScMap(entries))) = val else {
        return Err(unexpected(val));
    };
    // Contract-struct maps are emitted in alphabetical field-name order: key, weight.
    let [
        ScMapEntry {
            key: key_field,
            val: key_val,
        },
        ScMapEntry {
            key: weight_field,
            val: weight_val,
        },
    ] = entries.as_slice()
    else {
        return Err(unexpected(val));
    };
    if !is_symbol(key_field, "key") || !is_symbol(weight_field, "weight") {
        return Err(unexpected(val));
    }

    let ScVal::Bytes(ScBytes(bytes)) = key_val else {
        return Err(unexpected(val));
    };
    let key: [u8; 33] = bytes.as_slice().try_into().map_err(|_| unexpected(val))?;
    let ScVal::U64(weight) = weight_val else {
        return Err(unexpected(val));
    };
    Ok(SignerInfo {
        key,
        weight: *weight,
    })
}

fn is_symbol(v: &ScVal, expected: &str) -> bool {
    matches!(v, ScVal::Symbol(sym) if sym.0.as_slice() == expected.as_bytes())
}
