//! Reproduction test: a **contract-account (smart wallet)** `project_root`
//! owner and the `wasi-soroban-rs` Address-credential gap (see `SOROBAN_RS.md`).
//!
//! Opt-in (`#[ignore]`). Runs against a live protocol-26 RPC (testnet). Prefer
//! `task test-deployer-multisig`.
//!
//! ## What this proves
//!
//! The intended `project_root` owner is a contract account — a `C…` address
//! with a custom `__check_auth`. Here that owner is a real 2-of-2 ed25519
//! multisig smart account (`contracts/multisig-account`).
//!
//! * **Step 0b — the smart account works end-to-end.** It is friendbot-funded,
//!   then moves half its XLM to the deployer, authorized by its *own* 2-of-2
//!   quorum. The relayer (the deployer) submits the transaction;
//!   `wasi_soroban_rs::simulate_transaction_with_auth` signs the account's
//!   `Address`-credential auth entry with both ed25519 keys (the `authorizeEntry`
//!   flow — SOROBAN_RS.md), mirroring the recipe proven in
//!   `contracts/multisig-account`'s unit tests.
//!
//! * **Steps 1–4 — the smart account governs the pipeline.** The deployer
//!   deploys the pipeline and hands `project_root` over to the smart account,
//!   which then *finishes* the handover (`accept_admin`) and *governs*
//!   (`add_secp256k1_signer` via `project_root`). Each is a contract-account
//!   `require_auth` the deployer relays and the account's 2-of-2 quorum signs
//!   through `call_with_multisig`.
//!
//! ```bash
//! RPC_URL=https://soroban-testnet.stellar.org \
//! NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
//!   cargo test --test multisig_contract -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};
use warpdrive_client::project_root::{ProjectRootClient, VerificationType};
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::warpdrive::WarpdriveClient;
use warpdrive_deployer::config::{NetworkConfig, client_configs};
use warpdrive_deployer::deploy::{DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_pipeline};
use warpdrive_deployer::governance::handover;
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::ledger::get_latest_ledger;
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::retry::RetryConfig;
use wasi_soroban_rs::wasi_stellar_rpc_client::Client;
use wasi_soroban_rs::xdr::{
    Asset, ContractId as XdrContractId, ContractIdPreimage, Hash, HashIdPreimage,
    HashIdPreimageContractId, Int128Parts, Limits, ScAddress, ScBytes, ScVal, ScVec, WriteXdr,
};
use wasi_soroban_rs::{
    Account, Contract, ContractId, Env, Operations, Signer, SorobanHelperError,
    SorobanTransactionResponse, TransactionBuilder, simulate_transaction_with_auth,
};

/// The 2-of-2 ed25519 smart-account fixture (`contracts/multisig-account`).
const SMART_ACCOUNT_WASM: &str = "warpdrive_multisig_account.wasm";

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the multisig test"))
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}

/// A secp256k1 pubkey of the form `02 11 11 …` (33 bytes) seeded by `tag`.
fn secp_key(tag: u8) -> [u8; 33] {
    let mut k = [tag; 33];
    k[0] = 0x02;
    k
}

// ── ScVal builders ───────────────────────────────────────────────────────────

fn bytes_scval(b: &[u8]) -> ScVal {
    ScVal::Bytes(ScBytes(b.to_vec().try_into().expect("bytes")))
}

fn i128_scval(v: i128) -> ScVal {
    ScVal::I128(Int128Parts {
        hi: (v >> 64) as i64,
        lo: v as u64,
    })
}

fn i128_from_scval(v: &ScVal) -> i128 {
    match v {
        ScVal::I128(p) => ((p.hi as i128) << 64) + (p.lo as i128),
        other => panic!("expected i128, got {other:?}"),
    }
}

/// Ledgers the smart account's auth signatures stay valid for (~83 min on
/// testnet) — comfortably longer than a slow testnet round trip.
const SIGNATURE_VALIDITY_LEDGERS: u32 = 1000;

/// Invoke `fn_name(args)` on `contract` with `source` as the tx source/fee
/// payer, authorizing the invocation with `smart_account`'s 2-of-2 quorum.
///
/// `wasi-soroban-rs`'s `simulate_transaction_with_auth` does the whole dance —
/// simulate, sign + attach the smart account's `Address` auth entries,
/// re-simulate (enforce) and re-price — so this just resolves the signature
/// expiration ledger, wraps the keys as `Signer`s, builds the call, and submits.
#[allow(clippy::too_many_arguments)]
async fn call_with_multisig(
    env: &Env,
    net: &NetworkConfig,
    source: &Account,
    contract: ContractId,
    fn_name: &str,
    args: Vec<ScVal>,
    smart_account: &ScAddress,
    signers: &[SigningKey],
) -> Result<SorobanTransactionResponse, SorobanHelperError> {
    let signers: Vec<Signer> = signers.iter().cloned().map(Signer::new).collect();
    let valid_until = get_latest_ledger(&net.rpc_url)
        .await
        .map_err(|e| SorobanHelperError::NetworkRequestFailed(format!("latest ledger: {e}")))?
        + SIGNATURE_VALIDITY_LEDGERS;

    let op = Operations::invoke_contract(&contract, fn_name, args)?;
    let tx = TransactionBuilder::new(source, env)
        .add_operation(op)
        .build()
        .await?;
    let tx = simulate_transaction_with_auth(tx, env, smart_account, &signers, valid_until).await?;

    let mut src = source.clone();
    let signed = src.sign_transaction(&tx, &env.network_id())?;
    env.send_transaction(&signed).await
}

// ── Native asset (XLM) helpers ────────────────────────────────────────────────

/// The native-asset Stellar Asset Contract id for this network.
fn native_sac_id(env: &Env) -> ContractId {
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id: env.network_id(),
        contract_id_preimage: ContractIdPreimage::Asset(Asset::Native),
    });
    let hash: [u8; 32] = Sha256::digest(preimage.to_xdr(Limits::none()).expect("xdr")).into();
    ContractId(hash)
}

/// Read `who`'s native balance via the SAC (read-only simulation).
async fn native_balance(env: &Env, source: &Account, sac: ContractId, who: &ScAddress) -> i128 {
    let cfg = client_configs(env, source, sac);
    let res =
        warpdrive_client::utils::query(&cfg, "balance", std::vec![ScVal::Address(who.clone())])
            .await
            .expect("native balance query");
    i128_from_scval(&res)
}

/// Poll `who`'s native balance until it's positive (friendbot funding has
/// landed), or give up after a bounded number of tries.
async fn poll_balance(env: &Env, source: &Account, sac: ContractId, who: &ScAddress) -> i128 {
    for _ in 0..15 {
        let b = native_balance(env, source, sac, who).await;
        if b > 0 {
            return b;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    native_balance(env, source, sac, who).await
}

/// Friendbot-fund any address (classic or contract). Testnet friendbot credits
/// contract `C…` addresses via the native SAC.
async fn fund_via_friendbot(net: &NetworkConfig, friendbot: Option<&str>, addr: &str) {
    let base = match friendbot {
        Some(u) => u.to_string(),
        None => Client::new(&net.rpc_url)
            .expect("rpc client")
            .friendbot_url()
            .await
            .expect("derive friendbot url"),
    };
    let resp = reqwest::get(format!("{base}?addr={addr}"))
        .await
        .expect("friendbot request");
    let code = resp.status().as_u16();
    // 200 = funded; 400 = already funded (idempotent re-runs) — both fine.
    assert!(
        resp.status().is_success() || code == 400,
        "friendbot funding {addr} failed: HTTP {code}"
    );
}

#[tokio::test]
#[ignore = "requires a protocol-26 RPC (testnet); run with --ignored"]
async fn contract_account_owner_handover() {
    let net = NetworkConfig::new(env_or_skip("RPC_URL"), env_or_skip("NETWORK_PASSPHRASE"));
    let friendbot = std::env::var("FRIENDBOT_URL").ok();
    let retry_cfg = RetryConfig::default();

    let dir = tempfile::tempdir().unwrap();

    // The deployer: tx source for the deploys and the relayer that submits the
    // smart account's transactions on its behalf.
    let deployer_key = dir.path().join("deployer.secret");
    keygen_and_fund(&net, friendbot.as_deref(), &deployer_key, retry_cfg)
        .await
        .expect("keygen deployer");
    let deployer = account_from_secret(&read_key_file(&deployer_key).unwrap()).unwrap();
    let env = net.env().unwrap();

    // ── 0. Deploy the 2-of-2 ed25519 smart account (the contract owner) ──────
    // Two registered signers, threshold 2 — both must sign to authorize. The
    // deployer/relayer holds both wallet keys so it can satisfy the quorum.
    let signer_a = SigningKey::from_bytes(&[0xA1; 32]);
    let signer_b = SigningKey::from_bytes(&[0xB2; 32]);
    let signers_scval = ScVal::Vec(Some(ScVec(
        std::vec![
            bytes_scval(&signer_a.verifying_key().to_bytes()),
            bytes_scval(&signer_b.verifying_key().to_bytes()),
        ]
        .try_into()
        .expect("signers vec"),
    )));
    let ctor_args = std::vec![signers_scval, ScVal::U32(2)];
    // The relayer holds both wallet keys, so it can satisfy the 2-of-2 quorum
    // for every call the smart account must authorize below.
    let signers = [signer_a, signer_b];

    let owner_wasm = wasm_dir().join(SMART_ACCOUNT_WASM);
    let mut deployer_for_owner = deployer.clone();
    let owner_contract = Contract::new(owner_wasm.to_str().unwrap(), None)
        .expect("read smart-account wasm")
        .deploy(&env, &mut deployer_for_owner, Some(ctor_args))
        .await
        .expect("deploy 2-of-2 smart account")
        .contract_id()
        .expect("owner contract id");
    let owner_contract_addr = ScAddress::Contract(XdrContractId(Hash(owner_contract.0)));
    eprintln!("contract-account (smart wallet) owner: {owner_contract}");

    // ── 0b. Prove the smart account works: fund it, then move half its XLM ───
    // to the deployer under its own 2-of-2 quorum (no governance involved).
    fund_via_friendbot(&net, friendbot.as_deref(), &owner_contract.to_string()).await;

    let sac = native_sac_id(&env);
    let deployer_addr = ScAddress::Account(deployer.account_id());
    let before = poll_balance(&env, &deployer, sac, &owner_contract_addr).await;
    assert!(before > 0, "friendbot should have funded the smart account");

    let half = before / 2;
    call_with_multisig(
        &env,
        &net,
        &deployer, // relayer / tx source / fee payer
        sac,
        "transfer",
        std::vec![
            ScVal::Address(owner_contract_addr.clone()),
            ScVal::Address(deployer_addr.clone()),
            i128_scval(half),
        ],
        &owner_contract_addr,
        &signers,
    )
    .await
    .expect("2-of-2 multisig transfer of half the smart account's XLM");

    let after = native_balance(&env, &deployer, sac, &owner_contract_addr).await;
    assert_eq!(
        after,
        before - half,
        "smart account should have sent exactly half its XLM"
    );
    eprintln!("smart account moved {half} stroops to the deployer via its 2-of-2 quorum");

    // ── 1. Deploy the pipeline (deployer is project_root's admin) ────────────
    let manifest_path = dir.path().join("deploy.json");
    let manifest = deploy_pipeline(
        &env,
        &net,
        &deployer,
        &DeployParams {
            variant: Variant::Ethereum,
            wasm_dir: wasm_dir(),
            project_spec_repo: DEFAULT_PROJECT_SPEC_REPO.to_string(),
            threshold: (2, 3),
            verification_type: VerificationType::Ethereum,
        },
        &manifest_path,
        retry_cfg,
    )
    .await
    .expect("deploy");

    let project_root = manifest.project_root().unwrap();
    let pr = ProjectRootClient::new(client_configs(&env, &deployer, project_root));

    // ── 2. Propose the smart account as project_root's admin ─────────────────
    // propose_admin is authorized by the *current* admin (the deployer, the tx
    // source), so this SourceAccount-credential call succeeds.
    handover(
        &env,
        &deployer,
        &manifest,
        &owner_contract.to_string(),
        retry_cfg,
    )
    .await
    .expect("propose smart-account owner as project_root admin");
    assert_eq!(
        pr.pending_admin().await.unwrap(),
        Some(owner_contract_addr.clone()),
        "smart account should be project_root's pending admin"
    );

    // ── 3. Finish the handover — the smart account accepts via its quorum ────
    // accept_admin calls `pending_admin.require_auth()`, and the pending admin
    // is the smart account (never a tx source). The deployer relays the
    // transaction; the account's Address auth entry is signed by its 2-of-2
    // keys via `call_with_multisig`.
    call_with_multisig(
        &env,
        &net,
        &deployer,
        project_root,
        "accept_admin",
        std::vec![],
        &owner_contract_addr,
        &signers,
    )
    .await
    .expect("smart account accept-admin via 2-of-2 quorum");

    assert_eq!(
        pr.admin().await.unwrap(),
        owner_contract_addr,
        "smart account should now be project_root's admin"
    );

    // ── 4. The owner governs: add a security signer via project_root ─────────
    // project_root.add_secp256k1_signer forwards to the security contract and
    // requires project_root's admin (now the smart account) to authorize — the
    // same 2-of-2 quorum, again relayed by the deployer. (This is the multisig
    // equivalent of the relayed add-signer at the end of multisig_native.rs.)
    let new_signer = secp_key(0x55);
    call_with_multisig(
        &env,
        &net,
        &deployer,
        project_root,
        "add_secp256k1_signer",
        std::vec![bytes_scval(&new_signer), ScVal::U64(42)],
        &owner_contract_addr,
        &signers,
    )
    .await
    .expect("owner adds a security signer via project_root (2-of-2)");

    let security = manifest.security().unwrap();
    let sec = Secp256k1SecurityClient::new(client_configs(&env, &deployer, security));
    assert_eq!(
        sec.get_signer_weight(new_signer).await.unwrap(),
        42,
        "owner-added signer weight should be set"
    );
    eprintln!("smart account added a security signer via project_root (2-of-2)");
}
