use soroban_sdk::{Address, Env, String, contractclient};

use super::warpdrive::WarpDriveInterface;

// ── Interface trait (compile-time contract conformance) ──────────────

#[contractclient(name = "ProjectRootClient")]
pub trait ProjectRootInterface: WarpDriveInterface {
    // State Changing Operations
    fn update_project_spec_repo(env: Env, repo: String);

    // Queries
    fn security_contract(env: Env) -> Address;
    fn verification_contract(env: Env) -> Address;
    fn project_spec_repo(env: Env) -> String;
}
