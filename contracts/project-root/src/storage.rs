use soroban_sdk::{Address, Env, String, contracttype};
use warpdrive_shared::ttl;

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    SecurityContract,
    VerificationContract,
    ProjectSpecRepo,
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_version(env: &Env) -> String {
    env.storage().instance().get(&DataKey::Version).unwrap()
}

pub fn set_version(env: &Env, version: &String) {
    env.storage().instance().set(&DataKey::Version, version);
}

pub fn get_security_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::SecurityContract)
        .unwrap()
}

pub fn set_security_contract(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::SecurityContract, addr);
}

pub fn get_verification_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::VerificationContract)
        .unwrap()
}

pub fn set_verification_contract(env: &Env, addr: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::VerificationContract, addr);
}

pub fn get_project_spec_repo(env: &Env) -> String {
    env.storage()
        .instance()
        .get(&DataKey::ProjectSpecRepo)
        .unwrap()
}

pub fn set_project_spec_repo(env: &Env, repo: &String) {
    env.storage()
        .instance()
        .set(&DataKey::ProjectSpecRepo, repo);
}

pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(ttl::INSTANCE_RENEWAL_THRESHOLD, ttl::INSTANCE_TARGET_TTL);
}
