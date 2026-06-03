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
//!   quorum. The relayer (the deployer) submits the transaction and signs the
//!   account's `Address`-credential auth entry with both ed25519 keys. This is
//!   the `authorizeEntry` flow `wasi-soroban-rs` should expose as a first-class
//!   helper (SOROBAN_RS.md); `call_with_multisig` / `sign_auth_entry` below are
//!   a local stand-in built from its public building blocks, mirroring the
//!   recipe proven in `contracts/multisig-account`'s unit tests.
//!
//! * **Step 3 — the governance gap (commented out for now).** A contract
//!   account can never be a transaction *source*, so `accept_admin` (finishing
//!   a handover to it) needs the owner's `Address` credential. Routed through
//!   the client's `execute()`, that is rejected with `NotSupported`. Re-enable
//!   it once the call path uses the multisig signing above.
//!
//! ```bash
//! RPC_URL=https://soroban-testnet.stellar.org \
//! NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
//!   cargo test --test multisig_contract -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use ed25519_dalek::{Signer as _, SigningKey};
use sha2::{Digest, Sha256};
use warpdrive_client::project_root::{ProjectRootClient, VerificationType};
use warpdrive_client::warpdrive::WarpdriveClient;
use warpdrive_deployer::config::{NetworkConfig, client_configs};
use warpdrive_deployer::deploy::{DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_pipeline};
use warpdrive_deployer::error::DeployerError;
use warpdrive_deployer::governance::handover;
#[allow(unused_imports)] // used by the (currently commented) step 3 below
use warpdrive_deployer::governance::{Target, accept_admin};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::ledger::get_latest_ledger;
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::retry::RetryConfig;
use wasi_soroban_rs::wasi_stellar_rpc_client::Client;
use wasi_soroban_rs::xdr::{
    Asset, ContractId as XdrContractId, ContractIdPreimage, Hash, HashIdPreimage,
    HashIdPreimageContractId, HashIdPreimageSorobanAuthorization, Int128Parts,
    InvokeHostFunctionOp, Limits, Operation, OperationBody, ScAddress, ScBytes, ScMap, ScMapEntry,
    ScSymbol, ScVal, ScVec, SorobanAddressCredentials, SorobanAuthorizationEntry,
    SorobanCredentials, TransactionEnvelope, TransactionExt, TransactionV1Envelope, VecM, WriteXdr,
};
use wasi_soroban_rs::{
    Account, Contract, ContractId, Env, Operations, SorobanHelperError, SorobanTransactionResponse,
    TransactionBuilder,
};

/// The 2-of-2 ed25519 smart-account fixture (`contracts/multisig-account`).
const SMART_ACCOUNT_WASM: &str = "warpdrive_multisig_account.wasm";

/// Base inclusion fee per operation (stroops).
const BASE_FEE: u64 = 100;
/// Padding over the simulated resource fee to cover the bytes added by the
/// attached multisig signatures (which simulation, run unsigned, doesn't see).
const FEE_BUFFER: u64 = 200_000;

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the multisig test"))
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}

// ── ScVal builders ───────────────────────────────────────────────────────────

fn sym(s: &str) -> ScVal {
    ScVal::Symbol(ScSymbol(s.try_into().expect("symbol")))
}

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

/// One signer's `Ed25519Signature` as the host-encoded `ScVal` — a struct map
/// with keys sorted (`public_key` before `signature`), matching the fixture's
/// `Signature = Vec<Ed25519Signature>`.
fn ed25519_sig_scval(sk: &SigningKey, digest: &[u8; 32]) -> ScVal {
    let entries = std::vec![
        ScMapEntry {
            key: sym("public_key"),
            val: bytes_scval(&sk.verifying_key().to_bytes()),
        },
        ScMapEntry {
            key: sym("signature"),
            val: bytes_scval(&sk.sign(digest).to_bytes()),
        },
    ];
    ScVal::Map(Some(ScMap(entries.try_into().expect("sig map"))))
}

// ── The auth-entry signing wasi-soroban-rs should provide (SOROBAN_RS.md) ─────
//
// Given a simulation-returned auth entry, if it's an `Address` credential for
// `smart_account`, populate its `signature_expiration_ledger` + `signature` by
// signing the `SorobanAuthorization` preimage with `signers`. Every other entry
// (a `SourceAccount`, or an `Address` for a different address) passes through.

fn sign_auth_entry(
    entry: &SorobanAuthorizationEntry,
    smart_account: &ScAddress,
    signers: &[SigningKey],
    valid_until_ledger: u32,
    network_id: &Hash,
) -> SorobanAuthorizationEntry {
    let SorobanCredentials::Address(creds) = &entry.credentials else {
        return entry.clone();
    };
    if &creds.address != smart_account {
        return entry.clone();
    }

    // The digest the host recomputes and feeds to the account's `__check_auth`.
    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id: network_id.clone(),
        nonce: creds.nonce,
        signature_expiration_ledger: valid_until_ledger,
        invocation: entry.root_invocation.clone(),
    });
    let digest: [u8; 32] = Sha256::digest(preimage.to_xdr(Limits::none()).expect("xdr")).into();

    let signature = ScVal::Vec(Some(ScVec(
        signers
            .iter()
            .map(|sk| ed25519_sig_scval(sk, &digest))
            .collect::<std::vec::Vec<_>>()
            .try_into()
            .expect("sig vec"),
    )));

    SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(SorobanAddressCredentials {
            address: creds.address.clone(),
            nonce: creds.nonce,
            signature_expiration_ledger: valid_until_ledger,
            signature,
        }),
        root_invocation: entry.root_invocation.clone(),
    }
}

/// Invoke `fn_name(args)` on `contract` with `source` as the tx source/fee
/// payer, signing every `Address` auth entry that belongs to `smart_account`
/// with `signers`. This is what `wasi-soroban-rs`'s `execute()` does, minus the
/// blanket `Address`-credential rejection and plus [`sign_auth_entry`].
#[allow(clippy::too_many_arguments)]
async fn call_with_multisig(
    env: &Env,
    source: &Account,
    contract: ContractId,
    fn_name: &str,
    args: Vec<ScVal>,
    smart_account: &ScAddress,
    signers: &[SigningKey],
    valid_until_ledger: u32,
) -> Result<SorobanTransactionResponse, SorobanHelperError> {
    let op = Operations::invoke_contract(&contract, fn_name, args)?;
    let mut tx = TransactionBuilder::new(source, env)
        .add_operation(op)
        .build()
        .await?;

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: VecM::default(),
    });
    let sim = env.simulate_transaction(&envelope).await?;
    if let Some(err) = sim.error {
        return Err(SorobanHelperError::TransactionSimulationFailed(err));
    }
    let results = sim.results().unwrap_or_default();
    let network_id = env.network_id();

    // Attach signed auth to each invoke-host-function op.
    let mut ops: std::vec::Vec<Operation> = tx.operations.iter().cloned().collect();
    let mut idx = 0usize;
    for op in ops.iter_mut() {
        if let OperationBody::InvokeHostFunction(InvokeHostFunctionOp { auth, .. }) = &mut op.body {
            let result = results.get(idx).ok_or_else(|| {
                SorobanHelperError::TransactionSimulationFailed(
                    "simulation result count does not match operations".to_string(),
                )
            })?;
            let signed: std::vec::Vec<SorobanAuthorizationEntry> = result
                .auth
                .iter()
                .map(|e| {
                    sign_auth_entry(e, smart_account, signers, valid_until_ledger, &network_id)
                })
                .collect();
            *auth = VecM::try_from(signed).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("too many auth entries".to_string())
            })?;
            idx += 1;
        }
    }
    tx.operations = VecM::try_from(ops)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("too many operations".to_string()))?;

    // Re-simulate *with the signed auth attached*. The first (recording) run
    // skips `__check_auth`; this one runs it (enforce mode, since auth entries
    // are now present), so the returned footprint + resource fee account for the
    // account's signature verification — otherwise submission hits
    // ResourceLimitExceeded.
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: VecM::default(),
    });
    let sim = env.simulate_transaction(&envelope).await?;
    if let Some(err) = sim.error {
        return Err(SorobanHelperError::TransactionSimulationFailed(err));
    }

    tx.ext = TransactionExt::V1(sim.transaction_data().map_err(|e| {
        SorobanHelperError::TransactionFailed(format!("failed to get transaction data: {e}"))
    })?);
    tx.fee =
        u32::try_from(tx.operations.len() as u64 * BASE_FEE + sim.min_resource_fee + FEE_BUFFER)
            .map_err(|_| SorobanHelperError::InvalidArgument("fee overflows u32".to_string()))?;

    let mut src = source.clone();
    let signed = src.sign_transaction(&tx, &network_id)?;
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

// ── Helpers for the (currently commented) governance-gap step 3 ──────────────

/// `true` when `e` is the `SorobanCredentials::Address` rejection from
/// `wasi-soroban-rs` — the exact gap documented in `SOROBAN_RS.md`.
#[allow(dead_code)]
fn is_address_auth_gap(e: &DeployerError) -> bool {
    matches!(
        e,
        DeployerError::Soroban(SorobanHelperError::NotSupported(_))
    )
}

/// Asserts a governance call routed through the client is blocked by the
/// Address-credential gap. Used by step 3 once it is re-enabled.
#[allow(dead_code)]
fn expect_blocked_by_address_auth_gap<T>(result: Result<T, DeployerError>, ctx: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) if is_address_auth_gap(&e) => panic!(
            "EXPECTED-FAILING (SOROBAN_RS.md gap): {ctx} is blocked because the client \
             rejects the owner's Address-credential authorization: {e}\n\
             This test should turn green once wasi-soroban-rs can sign Address auth entries."
        ),
        Err(e) => panic!("{ctx}: unexpected (non-gap) error: {e}"),
    }
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
    let valid_until = get_latest_ledger(&net.rpc_url)
        .await
        .expect("latest ledger")
        + 1000;
    call_with_multisig(
        &env,
        &deployer, // relayer / tx source / fee payer
        sac,
        "transfer",
        std::vec![
            ScVal::Address(owner_contract_addr.clone()),
            ScVal::Address(deployer_addr.clone()),
            i128_scval(half),
        ],
        &owner_contract_addr,
        &[signer_a, signer_b],
        valid_until,
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

    // TODO: uncomment this out later once owner multisig is working properly

    // // ── 3. Finish the handover — blocked by the Address-credential gap ───────
    // // accept_admin calls `pending_admin.require_auth()`. The pending admin is a
    // // contract account, which can never be the tx source, so simulation returns
    // // Address(owner) and the client rejects it. A relayer (the deployer here)
    // // submitting on the smart account's behalf — and signing its 2-of-2 auth
    // // entry — is exactly the flow the upstream fix must enable. Expected to
    // // fail today.
    // let accepted = accept_admin(&env, &deployer, &manifest, Target::ProjectRoot, retry_cfg).await;
    // expect_blocked_by_address_auth_gap(
    //     accepted,
    //     "accept-admin finishing handover to a 2-of-2 smart-account owner",
    // );

    // // Post-fix assertion (only reached once the gap is closed).
    // assert_eq!(
    //     pr.admin().await.unwrap(),
    //     owner_contract_addr,
    //     "smart account should become project_root's admin once Address auth is supported"
    // );
}
