// This file runs some queries on the testnet deploy

use ed25519_dalek::SigningKey;
use soroban_rs::{Account, ClientContractConfigs, Env, EnvConfigs, Signer};
use warpdrive_client::loader::testnet;

use warpdrive_client::project_root::ProjectRootClient;
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::secp256k1_verification::Secp256k1VerificationClient;
use warpdrive_client::warpdrive::WarpdriveClient;

use warpdrive_shared::testutils::make_secp256k1_key;

const TESTNET_RPC: &str = "https://soroban-testnet.stellar.org";

// const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

fn new_signer(seed: u8) -> [u8; 33] {
    let signing = make_secp256k1_key(seed);
    let mut arr = [0u8; 33];
    arr.copy_from_slice(&signing.verifying_key().to_sec1_bytes());
    arr
}

#[tokio::main]
async fn main() {
    let cfg = testnet();
    println!("{}", cfg);

    let rpc_url = std::env::var("XLM_RPC_URL").unwrap_or_else(|_| TESTNET_RPC.to_string());
    let env_config = EnvConfigs {
        rpc_url: rpc_url.clone(),
        network_passphrase: TESTNET_PASSPHRASE.to_string(),
    };

    // Get a real signing key from env
    let secret = std::env::var("XLM_SECRET_KEY").unwrap();
    let secret_key = stellar_strkey::ed25519::PrivateKey::from_string(secret.trim())
        .expect("XLM_SECRET_KEY is not a valid Stellar secret (S...)");
    let account = Account::single(Signer::new(SigningKey::from_bytes(&secret_key.0)));
    println!("Signing Account: {}", account.account_id());

    let mut client: ClientContractConfigs = ClientContractConfigs {
        contract_id: cfg.project_root.clone(),
        env: Env::new(env_config.clone()).unwrap(),
        source_account: account.clone(),
    };

    // Query the Project Root to ensure we are admin
    let mut pr_client = ProjectRootClient::new(client.clone());
    let admin = pr_client.admin().await.unwrap();
    println!("Project Admin {}", admin);
    println!("Proposing self-change");
    pr_client
        .propose_admin(&account.account_id().to_string())
        .await
        .unwrap();

    client.contract_id = cfg.secp256k1_security;
    let mut sec_client = Secp256k1SecurityClient::new(client.clone());
    client.contract_id = cfg.secp256k1_verification;
    let ver_client = Secp256k1VerificationClient::new(client.clone());

    // Define new signer to add
    let new_signer = new_signer(18);

    // Check previous state
    let total = sec_client.get_total_weight().await.unwrap();
    println!("Total Weight before: {}", total);
    let required = ver_client.required_weight().await.unwrap();
    println!("Required Weight before: {}", required);
    let s_weight = ver_client.signer_weight(new_signer).await.unwrap();
    println!("Signer Weight before: {}", s_weight);

    // Add a new signer to the security contract
    println!("Adding/updating signer...");
    sec_client
        .add_signer(new_signer, s_weight + 100)
        .await
        .unwrap();

    // Check final state
    let total = sec_client.get_total_weight().await.unwrap();
    println!("Total Weight after: {}", total);
    let required = ver_client.required_weight().await.unwrap();
    println!("Required Weight after: {}", required);
    let s_weight = ver_client.signer_weight(new_signer).await.unwrap();
    println!("Signer Weight after: {}", s_weight);
}
