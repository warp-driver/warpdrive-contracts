use soroban_sdk::{Address, Env, contractevent, symbol_short};

#[contractevent]
pub struct AdminProposed {
    pub old_admin: Address,
    pub new_admin: Address,
}

impl AdminProposed {
    pub fn new(old_admin: Address, new_admin: Address) -> Self {
        Self {
            old_admin,
            new_admin,
        }
    }
}

#[contractevent]
pub struct AdminAccepted {
    pub new_admin: Address,
}

impl AdminAccepted {
    pub fn new(new_admin: Address) -> Self {
        Self { new_admin }
    }
}

/// Store a pending admin transfer. Requires auth from the current admin.
pub fn propose(env: &Env, current_admin: &Address, new_admin: Address) {
    current_admin.require_auth();
    env.storage()
        .instance()
        .set(&symbol_short!("pnd_admin"), &new_admin);
    AdminProposed::new(current_admin.clone(), new_admin).publish(env);
}

/// Accept a pending admin transfer. Requires auth from the pending admin.
/// Returns the new admin address so the caller can update its own storage.
pub fn accept(env: &Env) -> Address {
    let pending: Address = env
        .storage()
        .instance()
        .get(&symbol_short!("pnd_admin"))
        .expect("no pending admin");
    pending.require_auth();
    env.storage().instance().remove(&symbol_short!("pnd_admin"));
    AdminAccepted::new(pending.clone()).publish(env);
    pending
}

/// Get the pending admin, if any.
pub fn pending(env: &Env) -> Option<Address> {
    env.storage().instance().get(&symbol_short!("pnd_admin"))
}
