# warpdrive-client

Async Rust clients for the WarpDrive Soroban contracts on Stellar
(`project_root`, `secp256k1_security`, `secp256k1_verification`,
`ethereum_handler`, `stellar_handler`).

It wraps [`soroban-rs`](https://crates.io/crates/soroban-rs) to provide a
typed interface for querying and invoking the deployed contracts from any
Rust application, without pulling in the on-chain crates.

## What you need

- A Stellar RPC endpoint and network passphrase (Testnet or Mainnet).
- The contract addresses for the network you target. The `loader` module
  ships a built-in `testnet()` config; for Mainnet, supply your own.
- For write operations: a Stellar account secret key (`S...`) funded on
  the target network and authorized for the call (e.g. the security
  admin for `add_signer`).
- A `tokio` runtime — every client method is `async`.

Add it to your `Cargo.toml`:

```toml
[dependencies]
warpdrive-client = "0.2"
soroban-rs = "0.2"
ed25519-dalek = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Example: query a contract

```rust
use ed25519_dalek::SigningKey;
use soroban_rs::{Account, ClientContractConfigs, Env, EnvConfigs, Signer};
use warpdrive_client::loader::testnet;
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;

const TESTNET_RPC: &str = "https://soroban-testnet.stellar.org";
const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

#[tokio::main]
async fn main() {
    let cfg = testnet();
    let env = Env::new(EnvConfigs {
        rpc_url: TESTNET_RPC.to_string(),
        network_passphrase: TESTNET_PASSPHRASE.to_string(),
    })
    .unwrap();

    // Read-only calls still need a source account; any key works.
    let account = Account::single(Signer::new(SigningKey::from_bytes(&[1; 32])));

    let client = ClientContractConfigs {
        contract_id: cfg.secp256k1_security.clone(),
        env,
        source_account: account,
    };

    let security = Secp256k1SecurityClient::new(client);
    println!("Total weight: {}", security.get_total_weight().await.unwrap());
    for s in security.list_signers().await.unwrap() {
        println!("  signer {}", s);
    }
}
```

## Runnable examples

Two end-to-end examples live in [`examples/`](./examples):

- `examples/query.rs` — read-only sanity checks across all contracts.
- `examples/execute.rs` — admin write path (proposes admin, adds a signer).

They both target a fresh testnet deploy created via
`task testnet:deploy && task testnet:setup-signers`. Run them with:

```bash
cargo run --example query
# or, for the write example:
export XLM_SECRET_KEY=$(stellar keys secret warpdrive-test)
cargo run --example execute
```

`XLM_RPC_URL` overrides the default testnet RPC; the network passphrase
is hardcoded to Testnet in both examples.
