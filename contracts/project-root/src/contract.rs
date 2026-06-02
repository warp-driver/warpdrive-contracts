use soroban_sdk::{
    Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec, contract, contractimpl, vec,
};

use warpdrive_shared::interfaces::{
    project_root::{
        ContractType, Forwarded, HandlerRegistered, HandlerRemoved, ProjectRootError,
        ProjectRootInterface, UpdatedSpecRepo,
    },
    security::SecurityError,
    warpdrive::{ContractUpgraded, WarpDriveInterface},
};

use crate::storage::{self, VerificationType};

#[contract]
pub struct ProjectRoot;

#[contractimpl]
impl ProjectRoot {
    pub fn __constructor(
        env: Env,
        admin: Address,
        security_contract: Address,
        verification_contract: Address,
        project_spec_repo: String,
        verification_type: VerificationType,
    ) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, env!("CARGO_PKG_VERSION")));
        storage::set_security_contract(&env, &security_contract);
        storage::set_verification_contract(&env, &verification_contract);
        storage::set_project_spec_repo(&env, &project_spec_repo);
        storage::set_verification_type(&env, &verification_type);
        storage::extend_instance_ttl(&env);
    }
}

/// Maps a `try_verify` result from the security contract into a `SecurityError`.
/// You can later use `.into()` to convert to ProjectRootError if desired.
fn map_security_result(
    res: Result<
        Result<Val, soroban_sdk::ConversionError>,
        Result<SecurityError, soroban_sdk::InvokeError>,
    >,
) -> Result<Val, SecurityError> {
    match res {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(_conversion)) => panic!("ConversionError"),
        Err(Ok(e)) => Err(e),
        Err(Err(invoke_err)) => panic!("{:?}", invoke_err),
    }
}

impl ProjectRoot {
    /// Shared core for every forward path: admin gate, TTL, audit event, and
    /// the cross-contract call. `forward` and the typed helpers all funnel
    /// through here so the auth check and event are written once.
    ///
    /// Currently, the only proxied functions that return errors return SecurityError, so we can map to that as needed.
    /// Most calls can panic due to require_auth but that is different than a return error type.
    fn proxy(
        env: &Env,
        target: &Address,
        function: Symbol,
        args: Vec<Val>,
    ) -> Result<Val, SecurityError> {
        storage::get_admin(env).require_auth();
        storage::extend_instance_ttl(env);
        Forwarded::new(target.clone(), function.clone()).publish(env);
        let res = env.try_invoke_contract::<Val, SecurityError>(target, &function, args);
        map_security_result(res)
    }

    /// Confirms `target` belongs to this project before forwarding an
    /// admin-only call to it. The registered security and verification
    /// contracts always pass. Any other target must respond to the
    /// shared handler query `verification_contract()` with this project's
    /// registered verification contract address.
    fn ensure_our_contract(env: &Env, target: &Address) -> Result<ContractType, ProjectRootError> {
        let verification = storage::get_verification_contract(env);
        if target == &verification {
            return Ok(ContractType::Verification);
        }
        if target == &storage::get_security_contract(env) {
            return Ok(ContractType::Security);
        }

        // Otherwise, ensure this is a proper handler
        let function = Symbol::new(env, "verification_contract");
        let returned =
            env.try_invoke_contract::<Address, soroban_sdk::Error>(target, &function, vec![env]);
        if !matches!(&returned, Ok(Ok(addr)) if addr == &verification) {
            Err(ProjectRootError::NotOurContract)
        } else {
            Ok(ContractType::Handler)
        }
    }
}

#[contractimpl]
impl WarpDriveInterface for ProjectRoot {
    fn upgrade(env: Env, new_wasm_hash: BytesN<32>, new_version: String) {
        let admin = storage::get_admin(&env);
        admin.require_auth();

        storage::set_version(&env, &new_version);
        storage::extend_instance_ttl(&env);
        env.deployer().update_current_contract_wasm(new_wasm_hash);
        ContractUpgraded::new(new_version).publish(&env);
    }

    fn admin(env: Env) -> Address {
        storage::get_admin(&env)
    }

    fn pending_admin(env: Env) -> Option<Address> {
        warpdrive_shared::admin::pending(&env)
    }

    fn propose_admin(env: Env, new_admin: Address) {
        warpdrive_shared::admin::propose(&env, &storage::get_admin(&env), new_admin);
    }

    fn accept_admin(env: Env) {
        let new_admin = warpdrive_shared::admin::accept(&env);
        storage::set_admin(&env, &new_admin);
    }

    fn version(env: Env) -> String {
        storage::get_version(&env)
    }
}

#[contractimpl]
impl ProjectRootInterface for ProjectRoot {
    fn update_project_spec_repo(env: Env, repo: String) {
        storage::get_admin(&env).require_auth();
        storage::set_project_spec_repo(&env, &repo);
        UpdatedSpecRepo::new(repo).publish(&env);
    }

    fn security_contract(env: Env) -> Address {
        storage::get_security_contract(&env)
    }

    fn verification_contract(env: Env) -> Address {
        storage::get_verification_contract(&env)
    }

    fn project_spec_repo(env: Env) -> String {
        storage::get_project_spec_repo(&env)
    }

    fn verification_type(env: Env) -> VerificationType {
        storage::get_verification_type(&env)
    }

    fn list_handlers(env: Env) -> Vec<Address> {
        storage::get_handlers(&env)
    }

    // ── Typed helpers: registered security_contract ────────────────────

    fn add_secp256k1_signer(env: Env, key: BytesN<33>, weight: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "add_signer");
        let args = vec![&env, key.to_val(), weight.into_val(&env)];
        Self::proxy(&env, &target, function, args)?;
        Ok(())
    }

    fn remove_secp256k1_signer(env: Env, key: BytesN<33>) {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "remove_signer");
        let args = vec![&env, key.to_val()];
        // Currently, only errors on require_auth, so no other error to return
        Self::proxy(&env, &target, function, args).unwrap();
    }

    fn add_ed25519_signer(env: Env, key: BytesN<32>, weight: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "add_signer");
        let args = vec![&env, key.to_val(), weight.into_val(&env)];
        Self::proxy(&env, &target, function, args)?;
        Ok(())
    }

    fn remove_ed25519_signer(env: Env, key: BytesN<32>) {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "remove_signer");
        let args = vec![&env, key.to_val()];
        // Currently, only errors on require_auth, so no other error to return
        Self::proxy(&env, &target, function, args).unwrap();
    }

    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "set_threshold");
        let args = vec![&env, numerator.into_val(&env), denominator.into_val(&env)];
        Self::proxy(&env, &target, function, args)?;
        Ok(())
    }

    // ── Typed helpers: WarpDriveInterface on any target ────────────────

    fn upgrade_contract(
        env: Env,
        target: Address,
        new_wasm_hash: BytesN<32>,
        new_version: String,
    ) -> Result<(), ProjectRootError> {
        Self::ensure_our_contract(&env, &target)?;
        let function = Symbol::new(&env, "upgrade");
        let args = vec![&env, new_wasm_hash.to_val(), new_version.to_val()];
        Self::proxy(&env, &target, function, args)?;
        Ok(())
    }

    fn propose_contract_admin(
        env: Env,
        target: Address,
        new_admin: Address,
    ) -> Result<(), ProjectRootError> {
        Self::ensure_our_contract(&env, &target)?;
        let function = Symbol::new(&env, "propose_admin");
        let args = vec![&env, new_admin.to_val()];
        Self::proxy(&env, &target, function, args)?;
        // Rotating a handler's admin away does NOT untrack it — membership is
        // managed explicitly via unregister_handler so the set never drifts on
        // a propose that the new admin may never accept.
        Ok(())
    }

    fn accept_contract_admin(env: Env, target: Address) -> Result<(), ProjectRootError> {
        let ctype = Self::ensure_our_contract(&env, &target)?;
        let function = Symbol::new(&env, "accept_admin");
        let args = vec![&env];
        Self::proxy(&env, &target, function, args)?;
        // Taking over a handler's admin also tracks it as ours.
        if matches!(ctype, ContractType::Handler) && storage::add_handler(&env, &target) {
            HandlerRegistered::new(target).publish(&env);
        }
        Ok(())
    }

    fn register_handler(env: Env, handler: Address) -> Result<(), ProjectRootError> {
        storage::get_admin(&env).require_auth();
        // Only a handler reporting our verification contract can be tracked.
        match Self::ensure_our_contract(&env, &handler)? {
            ContractType::Handler => {}
            ContractType::Security | ContractType::Verification => {
                return Err(ProjectRootError::NotAHandler);
            }
        }
        storage::extend_instance_ttl(&env);
        if storage::add_handler(&env, &handler) {
            HandlerRegistered::new(handler).publish(&env);
        }
        Ok(())
    }

    fn unregister_handler(env: Env, handler: Address) {
        storage::get_admin(&env).require_auth();
        storage::extend_instance_ttl(&env);
        if storage::remove_handler(&env, &handler) {
            HandlerRemoved::new(handler).publish(&env);
        }
    }
}
