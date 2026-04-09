use crate::{ProjectRoot, ProjectRootClient};
use soroban_sdk::{Address, BytesN, Env, String, testutils::Address as _};
use warpdrive_secp256k1_security::Secp256k1Security;
use warpdrive_secp256k1_verification::Secp256k1Verification;
use warpdrive_shared::interfaces::project_root::VerificationType;

mod contract_wasm {
    use warpdrive_shared::interfaces::{CompressedSecpPubKey, Ed25519PubKey};

    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_project_root.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (ProjectRootClient<'a>, Address) {
    let admin = Address::generate(env);

    let security_id = env.register(Secp256k1Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let repo = String::from_str(env, "https://github.com/example/spec");

    let contract_id = env.register(
        ProjectRoot,
        (
            &admin,
            &security_id,
            &verification_id,
            &repo,
            VerificationType::Ethereum,
        ),
    );
    let client = ProjectRootClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
