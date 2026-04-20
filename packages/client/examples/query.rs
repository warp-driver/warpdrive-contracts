// This file runs some queries on the testnet deploy

use ed25519_dalek::SigningKey;
use soroban_rs::{Account, ClientContractConfigs, Env, EnvConfigs, Signer};
use warpdrive_client::loader::testnet;

use warpdrive_client::ethereum_handler::EthereumHandlerClient;
use warpdrive_client::project_root::ProjectRootClient;
use warpdrive_client::secp256k1_verification::Secp256k1VerificationClient;

const TESTNET_RPC: &str = "https://soroban-testnet.stellar.org";

// const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

#[tokio::main]
async fn main() {
    let cfg = testnet();
    println!("{}", cfg);

    let rpc_url = std::env::var("XLM_RPC_URL").unwrap_or_else(|_| TESTNET_RPC.to_string());
    let env_config = EnvConfigs {
        rpc_url: rpc_url.clone(),
        network_passphrase: TESTNET_PASSPHRASE.to_string(),
    };
    // TODO: any placeholder for queries (invalid key)
    let account = Account::single(Signer::new(SigningKey::from_bytes(&[1; 32])));

    let mut client: ClientContractConfigs = ClientContractConfigs {
        contract_id: cfg.project_root.clone(),
        env: Env::new(env_config.clone()).unwrap(),
        source_account: account.clone(),
    };

    // Query the Project Root
    let pr_client = ProjectRootClient::new(client.clone());
    let typ = pr_client.verification_type().await.unwrap();
    println!("Verification Type {:?}", typ);
    let ver = pr_client.verification_contract().await.unwrap();
    println!("Verification Contract {}", ver);
    assert_eq!(ver, cfg.secp256k1_verification);

    // TODO: Query the Security Contract

    // Check in the verification contract
    client.contract_id = cfg.secp256k1_verification;
    let ver_client = Secp256k1VerificationClient::new(client.clone());
    let sec = ver_client.security_contract().await.unwrap();
    println!("Security Contract {}", sec);
    assert_eq!(sec, cfg.secp256k1_security);
    let required = ver_client.required_weight().await.unwrap();
    println!("Required Weight: {}", required);

    // Check in the handler
    client.contract_id = cfg.ethereum_handler;
    let hand_client = EthereumHandlerClient::new(client.clone());
    let ver = hand_client.verification_contract().await.unwrap();
    assert_eq!(ver, cfg.secp256k1_verification);
}
