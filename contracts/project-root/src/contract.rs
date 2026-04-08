use soroban_sdk::{Address, BytesN, Env, String, contract, contractimpl};

use warpdrive_shared::interfaces::{
    project_root::ProjectRootInterface,
    warpdrive::{ContractUpgraded, WarpDriveInterface},
};

use crate::storage;

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
    ) {
        storage::set_admin(&env, &admin);
        storage::set_version(&env, &String::from_str(&env, "0.0.1"));
        storage::set_security_contract(&env, &security_contract);
        storage::set_verification_contract(&env, &verification_contract);
        storage::set_project_spec_repo(&env, &project_spec_repo);
        storage::extend_instance_ttl(&env);
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
}
