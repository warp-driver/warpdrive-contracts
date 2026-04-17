use soroban_rs::xdr::{ScVal, Transaction, TransactionEnvelope, TransactionV1Envelope, VecM};
use soroban_rs::{
    ClientContractConfigs, Env, Operations, SorobanHelperError, SorobanTransactionResponse,
    TransactionBuilder,
};

/// Default transaction fee in stroops (0.00001 XLM)
pub const DEFAULT_TRANSACTION_FEE: u32 = 100;

/// Runs a read-only simulation and returns the top-level return value of the
/// invoked contract function. Errors if simulation fails or if the host does
/// not return exactly one operation result. A function returning `()` yields
/// `ScVal::Void` (which is distinct from the "no results" error).
pub async fn query(
    cfg: &ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<ScVal, SorobanHelperError> {
    let tx = build_tx(cfg, function_name, args).await?;
    let tx_envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::default(),
    });
    let simulation = cfg.env.simulate_transaction(&tx_envelope).await?;

    if let Some(err) = simulation.error {
        return Err(SorobanHelperError::TransactionSimulationFailed(err));
    }

    let sim_results = simulation.results().unwrap_or_default();
    let first = sim_results.first().ok_or_else(|| {
        SorobanHelperError::TransactionSimulationFailed(
            "simulation returned no results".to_string(),
        )
    })?;
    Ok(first.xdr.clone())
}

/// Simulates to compute the fee, then signs and submits the transaction.
pub async fn execute(
    cfg: &mut ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<SorobanTransactionResponse, SorobanHelperError> {
    let mut tx = build_tx(cfg, function_name, args).await?;

    let tx_envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: VecM::default(),
    });
    let simulation = cfg.env.simulate_transaction(&tx_envelope).await?;

    if let Some(err) = simulation.error {
        return Err(SorobanHelperError::TransactionSimulationFailed(err));
    }

    let updated_fee = DEFAULT_TRANSACTION_FEE.max(
        u32::try_from(
            (tx.operations.len() as u64 * DEFAULT_TRANSACTION_FEE as u64)
                + simulation.min_resource_fee,
        )
        .map_err(|_| {
            SorobanHelperError::InvalidArgument("Transaction fee overflows u32".to_string())
        })?,
    );
    tx.fee = updated_fee;

    let env = cfg.env.clone();
    let signed = cfg
        .source_account
        .sign_transaction(&tx, &env.network_id())?;
    env.send_transaction(&signed).await
}

async fn build_tx(
    cfg: &ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<Transaction, SorobanHelperError> {
    let contract_id = cfg.contract_id;
    let env: Env = cfg.env.clone();

    let invoke_operation = Operations::invoke_contract(&contract_id, function_name, args)?;

    TransactionBuilder::new(&cfg.source_account, &env)
        .add_operation(invoke_operation)
        .build()
        .await
}
