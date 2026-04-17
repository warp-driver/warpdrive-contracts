use soroban_rs::xdr::{ScAddress, ScString, ScVal};
use soroban_rs::{
    ClientContractConfigs, IntoScVal, SorobanHelperError, SorobanTransactionResponse,
};

use crate::utils::{execute, query, unexpected};

#[allow(async_fn_in_trait)]
pub trait WarpdriveClient {
    fn get_client_configs(&self) -> &ClientContractConfigs;
    fn mut_client_configs(&mut self) -> &mut ClientContractConfigs;

    async fn upgrade(
        &mut self,
        new_wasm_hash: [u8; 32],
        new_version: String,
    ) -> Result<SorobanTransactionResponse, SorobanHelperError> {
        let args = vec![new_wasm_hash.into_val(), new_version.into_val()];
        execute(self.mut_client_configs(), "upgrade", args).await
    }

    async fn propose_admin(&mut self, new_admin: ScAddress) -> Result<(), SorobanHelperError> {
        let args = vec![ScVal::Address(new_admin)];
        execute(self.mut_client_configs(), "propose_admin", args).await?;
        Ok(())
    }

    async fn accept_admin(&mut self) -> Result<(), SorobanHelperError> {
        execute(self.mut_client_configs(), "accept_admin", vec![]).await?;
        Ok(())
    }

    async fn admin(&self) -> Result<ScAddress, SorobanHelperError> {
        let res = query(self.get_client_configs(), "admin", vec![]).await?;
        if let ScVal::Address(address) = res {
            return Ok(address);
        }
        Err(unexpected(&res))
    }

    async fn pending_admin(&self) -> Result<Option<ScAddress>, SorobanHelperError> {
        let res = query(self.get_client_configs(), "pending_admin", vec![]).await?;
        match res {
            ScVal::Void => Ok(None),
            ScVal::Address(address) => Ok(Some(address)),
            other => Err(unexpected(&other)),
        }
    }

    async fn version(&self) -> Result<String, SorobanHelperError> {
        let res = query(self.get_client_configs(), "version", vec![]).await?;
        if let ScVal::String(ScString(ref s)) = res {
            return String::from_utf8(s.as_vec().clone()).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("version not utf-8".to_string())
            });
        }
        Err(unexpected(&res))
    }
}
