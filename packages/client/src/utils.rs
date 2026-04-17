use soroban_rs::xdr::{
    ScVal, SorobanCredentials, Transaction, TransactionEnvelope, TransactionV1Envelope, VecM,
};
use soroban_rs::{
    ClientContractConfigs, Env, Operations, SorobanHelperError, SorobanTransactionResponse,
    TransactionBuilder,
};

/// Default transaction fee in stroops (0.00001 XLM)
pub const DEFAULT_TRANSACTION_FEES: u32 = 100;

// Queries via simulation and returns the ScVal response
pub async fn query(
    cfg: &mut ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<Option<ScVal>, SorobanHelperError> {
    let (results, _) = simulate_tx(cfg, function_name, args).await?;
    Ok(results)
}

// You can get the ScVal returned from the contract via execute(...).await?.get_return_value()
pub async fn execute(
    cfg: &mut ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<SorobanTransactionResponse, SorobanHelperError> {
    // Simulate to check failures and calculate proper gas
    let (_, invoke_tx) = simulate_tx(cfg, function_name, args).await?;

    let env: Env = cfg.env.clone();
    let tx_envelope = cfg
        .source_account
        .sign_transaction(&invoke_tx, &env.network_id())?;

    env.send_transaction(&tx_envelope).await
}

async fn build_tx(
    cfg: &mut ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<Transaction, SorobanHelperError> {
    let client_configs = cfg;

    let contract_id = client_configs.contract_id;
    let env: Env = client_configs.env.clone();

    let invoke_operation = Operations::invoke_contract(&contract_id, function_name, args)?;

    TransactionBuilder::new(&client_configs.source_account, &env)
        .add_operation(invoke_operation)
        .build()
        .await
}

async fn simulate_tx(
    cfg: &mut ClientContractConfigs,
    function_name: &str,
    args: Vec<ScVal>,
) -> Result<(Option<ScVal>, Transaction), SorobanHelperError> {
    let mut tx = build_tx(cfg, function_name, args).await?;

    let tx_envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx.clone(),
        signatures: VecM::default(),
    });
    let simulation = cfg.env.simulate_transaction(&tx_envelope).await?;

    // Handle error
    if let Some(err) = simulation.error {
        return Err(SorobanHelperError::TransactionSimulationFailed(err));
    }

    let sim_results = simulation.results().unwrap_or_default();
    // Check for auth issues
    // TODO: do we need this? taken from soroban-rs "simulate_and_build" but I don't get it
    for result in &sim_results {
        for auth in &result.auth {
            if matches!(auth.credentials, SorobanCredentials::Address(_)) {
                return Err(SorobanHelperError::NotSupported(
                    "Address authorization not yet supported".to_string(),
                ));
            }
        }
    }

    // Determine real fee needed if it will be exeucted
    let updated_fee = DEFAULT_TRANSACTION_FEES.max(
        u32::try_from(
            (tx.operations.len() as u64 * DEFAULT_TRANSACTION_FEES as u64)
                + simulation.min_resource_fee,
        )
        .map_err(|_| SorobanHelperError::InvalidArgument("Transaction fee too high".to_string()))?,
    );
    tx.fee = updated_fee;

    // I assume those are also subcall results, we just return the top-level result which is what we get on a real call
    let val = sim_results.first().map(|r| r.xdr.clone());
    Ok((val, tx))
}
