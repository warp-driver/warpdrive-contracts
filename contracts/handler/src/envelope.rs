extern crate alloc;

use alloy_sol_types::sol;
use soroban_sdk::{Bytes, Env};

sol! {
    struct Envelope {
        bytes20 eventId;
        bytes12 ordering;
        bytes payload;
    }
}

impl Envelope {
    pub fn abi_decode_from(data: &Bytes) -> Self {
        let mut buf = alloc::vec![0u8; data.len() as usize];
        data.copy_into_slice(&mut buf);
        <Envelope as alloy_sol_types::SolValue>::abi_decode(&buf)
            .expect("invalid ABI-encoded Envelope")
    }

    pub fn abi_encode_to(&self, env: &Env) -> Bytes {
        let encoded = <Envelope as alloy_sol_types::SolValue>::abi_encode(self);
        Bytes::from_slice(env, &encoded)
    }
}
