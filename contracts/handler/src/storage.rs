use soroban_sdk::{Address, BytesN, Env, String, contracttype};

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    VerificationContract,
    EventSeen(BytesN<20>),
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

pub fn get_verification_contract(env: &Env) -> Address {
    env.storage()
        .instance()
        .get(&DataKey::VerificationContract)
        .unwrap()
}

pub fn set_verification_contract(env: &Env, address: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::VerificationContract, address);
}

pub fn is_event_seen(env: &Env, event_id: &BytesN<20>) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::EventSeen(event_id.clone()))
}

pub fn mark_event_seen(env: &Env, event_id: &BytesN<20>) {
    env.storage()
        .persistent()
        .set(&DataKey::EventSeen(event_id.clone()), &true);
}
