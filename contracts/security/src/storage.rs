use soroban_sdk::{contracttype, Address, Env, String};

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    Count,
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

pub fn get_count(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::Count).unwrap_or(0)
}

pub fn inc_count(env: &Env, amount: impl Into<Option<u64>>) {
    let amount = amount.into().unwrap_or(1);
    let value = get_count(env) + amount;
    env.storage().instance().set(&DataKey::Count, &value)
}
