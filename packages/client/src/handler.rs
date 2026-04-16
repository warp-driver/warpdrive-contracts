use soroban_rs::xdr::ScVal;
use soroban_rs::{Account, ClientContractConfigs, ContractId, Env, EnvConfigs, Signer};

use crate::utils::{execute, query};
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
        envelope_bytes: soroban_rs::xdr::ScVal,
        sig_data: soroban_rs::xdr::ScVal,
    ) -> Result<soroban_rs::SorobanTransactionResponse, soroban_rs::SorobanHelperError> {
        execute(
            &mut self.client_configs,
            "verify_eth",
            vec![envelope_bytes, sig_data],
        )
        .await
    }

    pub async fn upgrade(
        &mut self,
        new_wasm_hash: soroban_rs::xdr::ScVal,
        new_version: soroban_rs::xdr::ScVal,
    ) -> Result<soroban_rs::SorobanTransactionResponse, soroban_rs::SorobanHelperError> {
        execute(
            &mut self.client_configs,
            "upgrade",
            vec![new_wasm_hash, new_version],
        )
        .await
    }

    pub async fn propose_admin(
        &mut self,
        new_admin: soroban_rs::xdr::ScVal,
    ) -> Result<soroban_rs::SorobanTransactionResponse, soroban_rs::SorobanHelperError> {
        execute(&mut self.client_configs, "propose_admin", vec![new_admin]).await
    }

    pub async fn accept_admin(
        &mut self,
    ) -> Result<soroban_rs::SorobanTransactionResponse, soroban_rs::SorobanHelperError> {
        execute(&mut self.client_configs, "accept_admin", vec![]).await
    }

    pub async fn admin(&mut self) -> Result<Vec<ScVal>, soroban_rs::SorobanHelperError> {
        query(&mut self.client_configs, "admin", vec![]).await
    }

    pub async fn pending_admin(&mut self) -> Result<Vec<ScVal>, soroban_rs::SorobanHelperError> {
        query(&mut self.client_configs, "pending_admin", vec![]).await
    }

    pub async fn version(&mut self) -> Result<Vec<ScVal>, soroban_rs::SorobanHelperError> {
        query(&mut self.client_configs, "version", vec![]).await
    }
    pub async fn verification_contract(
        &mut self,
    ) -> Result<Vec<ScVal>, soroban_rs::SorobanHelperError> {
        query(&mut self.client_configs, "verification_contract", vec![]).await
    }
    pub async fn payload(
        &mut self,
        event_id: soroban_rs::xdr::ScVal,
    ) -> Result<Vec<ScVal>, soroban_rs::SorobanHelperError> {
        query(&mut self.client_configs, "payload", vec![event_id]).await
    }
}

const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

use ed25519_dalek::SigningKey;
use stellar_strkey::ed25519::PrivateKey;

#[allow(dead_code)]
pub fn mock_signer1() -> Signer {
    let pk = PrivateKey::from_string("SD3C2X7WPTUYX4YHL2G34PX75JZ35QJDFKM6SXDLYHWIPOWPIQUXFVLE")
        .unwrap();
    Signer::new(SigningKey::from_bytes(&pk.0))
}

pub async fn demo_query(env_config: EnvConfigs, contract_id: ContractId) {
    let env = Env::new(env_config).unwrap();

    // TODO: any placeholder for queries (invalid key)
    let account = Account::single(Signer::new(SigningKey::from_bytes(&[1; 32])));

    let cfg: ClientContractConfigs = ClientContractConfigs {
        contract_id,
        env,
        source_account: account,
    };

    let mut hc = EthereumHandlerClient::new(&cfg);
    let ver = hc.version().await.unwrap();
    println!("{:?}", ver)
}

pub async fn demo_execute(env_config: EnvConfigs, contract_id: ContractId, account: Account) {
    let env = Env::new(env_config).unwrap();

    let cfg: ClientContractConfigs = ClientContractConfigs {
        contract_id,
        env,
        source_account: account,
    };

    let mut hc = EthereumHandlerClient::new(&cfg);
    let ver = hc.version().await.unwrap();
    println!("{:?}", ver)
}

pub async fn demo_main() {
    let rpc_url = std::env::var("XLM_RPC_URL").unwrap();
    let contract_str = std::env::var("XLM_CONTRACT").unwrap();

    let contract_id = ContractId::from_string(&contract_str).unwrap();

    let env_config = EnvConfigs {
        rpc_url: rpc_url.clone(),
        network_passphrase: MAINNET_PASSPHRASE.to_string(),
    };

    demo_query(env_config.clone(), contract_id).await;

    // TODO: use real account to sign
    let account = Account::single(mock_signer1());

    demo_execute(env_config.clone(), contract_id, account).await;
}
