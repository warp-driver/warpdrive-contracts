use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{Address, BytesN, Env, String, testutils::Address as _};
use warpdrive_security::Security;
use warpdrive_verification::Verification;

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_project_root.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (ProjectRootClient<'a>, Address) {
    let admin = Address::generate(env);

    let security_id = env.register(Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Verification, (&admin, &security_id));
    let repo = String::from_str(env, "https://github.com/example/spec");

    let contract_id = env.register(ProjectRoot, (&admin, &security_id, &verification_id, &repo));
    let client = ProjectRootClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
