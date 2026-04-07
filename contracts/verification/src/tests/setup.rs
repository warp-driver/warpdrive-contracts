use crate::{Verification, VerificationClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};
use warpdrive_security::Security;

mod contract_wasm {
    use warpdrive_shared::interfaces::PubKey;

    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_verification.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (VerificationClient<'a>, Address) {
    let admin = Address::generate(env);

    // Deploy security contract first
    let security_id = env.register(Security, (&admin, 2u64, 3u64));

    let contract_id = env.register(Verification, (&admin, &security_id));
    let client = VerificationClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
