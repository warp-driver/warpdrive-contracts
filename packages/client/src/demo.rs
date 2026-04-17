use ed25519_dalek::SigningKey;
use soroban_rs::{Account, ClientContractConfigs, ContractId, Env, EnvConfigs, Signer};
use stellar_strkey::ed25519::PrivateKey;

use crate::ethereum_handler::EthereumHandlerClient;
use crate::warpdrive::WarpdriveClient;

const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

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

    let hc = EthereumHandlerClient::new(cfg);
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

    let hc = EthereumHandlerClient::new(cfg);
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
