use soroban_sdk::{
    Address, BytesN, Env, IntoVal, String, Symbol, Val, Vec, contract, contractimpl, vec,
};

use warpdrive_shared::interfaces::{
    project_root::{Forwarded, ProjectRootInterface, UpdatedSpecRepo},
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

impl ProjectRoot {
    /// Shared core for every forward path: admin gate, TTL, audit event, and
    /// the cross-contract call. `forward` and the typed helpers all funnel
    /// through here so the auth check and event are written once.
    fn proxy(env: &Env, target: &Address, function: Symbol, args: Vec<Val>) -> Val {
        storage::get_admin(env).require_auth();
        storage::extend_instance_ttl(env);
        Forwarded::new(target.clone(), function.clone()).publish(env);
        env.invoke_contract::<Val>(target, &function, args)
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

    // ── Typed helpers: registered security_contract ────────────────────

    fn add_secp256k1_signer(env: Env, key: BytesN<33>, weight: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "add_signer");
        let args = vec![&env, key.to_val(), weight.into_val(&env)];
        Self::proxy(&env, &target, function, args);
        Ok(())
    }

    fn remove_secp256k1_signer(env: Env, key: BytesN<33>) {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "remove_signer");
        let args = vec![&env, key.to_val()];
        Self::proxy(&env, &target, function, args);
    }

    fn add_ed25519_signer(env: Env, key: BytesN<32>, weight: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "add_signer");
        let args = vec![&env, key.to_val(), weight.into_val(&env)];
        Self::proxy(&env, &target, function, args);
        Ok(())
    }

    fn remove_ed25519_signer(env: Env, key: BytesN<32>) {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "remove_signer");
        let args = vec![&env, key.to_val()];
        Self::proxy(&env, &target, function, args);
    }

    fn set_threshold(env: Env, numerator: u64, denominator: u64) -> Result<(), SecurityError> {
        let target = storage::get_security_contract(&env);
        let function = Symbol::new(&env, "set_threshold");
        let args = vec![&env, numerator.into_val(&env), denominator.into_val(&env)];
        Self::proxy(&env, &target, function, args);
        Ok(())
    }

    // ── Typed helpers: WarpDriveInterface on any target ────────────────

    fn upgrade_contract(env: Env, target: Address, new_wasm_hash: BytesN<32>, new_version: String) {
        let function = Symbol::new(&env, "upgrade");
        let args = vec![&env, new_wasm_hash.to_val(), new_version.to_val()];
        Self::proxy(&env, &target, function, args);
    }

    fn propose_contract_admin(env: Env, target: Address, new_admin: Address) {
        let function = Symbol::new(&env, "propose_admin");
        let args = vec![&env, new_admin.to_val()];
        Self::proxy(&env, &target, function, args);
    }

    fn accept_contract_admin(env: Env, target: Address) {
        let function = Symbol::new(&env, "accept_admin");
        let args = vec![&env];
        Self::proxy(&env, &target, function, args);
    }
}
