use crate::{Verification, VerificationClient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/warpdrive_verification.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (VerificationClient<'a>, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(Verification, (&admin,));
    let client = VerificationClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
