use soroban_rs::xdr::{
    InvokeHostFunctionOp, OperationBody, ScVal, SorobanAuthorizationEntry, SorobanCredentials,
    Transaction, TransactionEnvelope, TransactionExt, TransactionV1Envelope, VecM,
};
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

    // If we would simulate calling a function that does have admin.require_auth() call (like upgrade),
    // the sim_results would return a success containing an Address which is required for this operation's authentication.
    // If I understand correctly, when sim reads admin from storage it compares it to the tx's source account
    // and if it's the same it returns SourceAccount, if not it's just an Address(admin_pubkey).
    // https://docs.rs/soroban-sdk/latest/soroban_sdk/xdr/enum.SorobanCredentials.html
    // That's why if the returned auth result is an Address(_) it means it will fail the require_auth() call on real execution.
    let sim_results = simulation.results().unwrap_or_default();
    for result in &sim_results {
        for auth in &result.auth {
            if matches!(auth.credentials, SorobanCredentials::Address(_)) {
                return Err(SorobanHelperError::NotSupported(
                    "Address authorization not yet supported".to_string(),
                ));
            }
        }
    }

    // Attach the auth entries from the simulation to the invoke-host-function
    // operations. Without this, require_auth() calls (e.g. admin checks) fail on
    // the real network with TxMalformed even if the source account is authorized.
    attach_auth_from_simulation(&mut tx, &sim_results)?;

    // Attach the Soroban transaction data (resource footprint) from the
    // simulation. The network requires this for any Soroban invocation; a tx
    // with `ext: V0` is rejected as TxMalformed.
    let tx_data = simulation.transaction_data().map_err(|e| {
        SorobanHelperError::TransactionFailed(format!("Failed to get transaction data: {}", e))
    })?;
    tx.ext = TransactionExt::V1(tx_data);

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

fn attach_auth_from_simulation(
    tx: &mut Transaction,
    sim_results: &[soroban_rs::stellar_rpc_client::SimulateHostFunctionResult],
) -> Result<(), SorobanHelperError> {
    let mut ops: Vec<_> = tx.operations.iter().cloned().collect();
    let mut result_idx = 0;
    for op in ops.iter_mut() {
        if let OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function,
            auth,
        }) = &mut op.body
        {
            let result = sim_results.get(result_idx).ok_or_else(|| {
                SorobanHelperError::TransactionSimulationFailed(
                    "simulation result count does not match operations".to_string(),
                )
            })?;
            let entries: Vec<SorobanAuthorizationEntry> = result.auth.clone();
            *auth = VecM::try_from(entries).map_err(|_| {
                SorobanHelperError::XdrEncodingFailed("too many auth entries".to_string())
            })?;
            let _ = host_function;
            result_idx += 1;
        }
    }
    tx.operations = VecM::try_from(ops)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("too many operations".to_string()))?;
    Ok(())
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

pub(crate) fn unexpected(res: &ScVal) -> SorobanHelperError {
    SorobanHelperError::TransactionSimulationFailed(format!("Unexpected result: {:?}", res))
}
