use soroban_sdk::{Address, Env, String, Vec, contracttype};
use warpdrive_shared::ttl;

pub use warpdrive_shared::interfaces::project_root::VerificationType;

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    SecurityContract,
    VerificationContract,
    ProjectSpecRepo,
    VerificationType,
    Handlers,
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

pub fn get_verification_type(env: &Env) -> VerificationType {
    env.storage()
        .instance()
        .get(&DataKey::VerificationType)
        .unwrap()
}

pub fn set_verification_type(env: &Env, vtype: &VerificationType) {
    env.storage()
        .instance()
        .set(&DataKey::VerificationType, vtype);
}

/// Returns the registered handler contracts, or an empty `Vec` if none have
/// been added yet (the key is unset until the first `add_handler`).
pub fn get_handlers(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&DataKey::Handlers)
        .unwrap_or_else(|| Vec::new(env))
}

/// Registers `handler`, ignoring the call if it is already present so the set
/// stays free of duplicates. Returns `true` if the handler was newly added,
/// `false` if it was already tracked (so callers can avoid re-emitting events).
pub fn add_handler(env: &Env, handler: &Address) -> bool {
    let mut handlers = get_handlers(env);
    if handlers.first_index_of(handler).is_some() {
        return false;
    }
    handlers.push_back(handler.clone());
    env.storage().instance().set(&DataKey::Handlers, &handlers);
    true
}

/// Removes `handler` if present. Returns `true` if an entry was removed,
/// `false` when it wasn't tracked (so callers can avoid emitting events).
pub fn remove_handler(env: &Env, handler: &Address) -> bool {
    let mut handlers = get_handlers(env);
    if let Some(index) = handlers.first_index_of(handler) {
        handlers.remove(index);
        env.storage().instance().set(&DataKey::Handlers, &handlers);
        true
    } else {
        false
    }
}

pub fn extend_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(ttl::INSTANCE_RENEWAL_THRESHOLD, ttl::INSTANCE_TARGET_TTL);
}
