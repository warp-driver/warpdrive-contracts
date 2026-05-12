//! Std/client-side mirror of `warpdrive_shared::interfaces::handler::MessageWithId`.
//!
//! The on-chain struct is defined with `#[contracttype]` in `warpdrive-shared`,
//! but that crate pulls in `soroban-sdk` and cannot be linked into the std-only
//! client. This module reimplements the same wire format (XDR-encoded
//! `ScVal::Map` with alphabetically-sorted symbol keys) so off-chain code can
//! build and parse `MessageWithId` payloads. The roundtrip is covered by
//! `tests.rs::message_with_id_*`.
use wasi_soroban_rs::SorobanHelperError;
use wasi_soroban_rs::xdr::{Limits, ReadXdr, ScBytes, ScSymbol, ScVal, WriteXdr};

use crate::scval::{IntoScValExt, struct_map};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageWithId {
    pub trigger_id: u64,
    pub message: Vec<u8>,
}

impl MessageWithId {
    pub fn to_xdr_bytes(&self) -> Result<Vec<u8>, SorobanHelperError> {
        self.to_scval()?
            .to_xdr(Limits::none())
            .map_err(|e| SorobanHelperError::XdrEncodingFailed(e.to_string()))
    }

    pub fn from_xdr_bytes(bytes: &[u8]) -> Result<Self, SorobanHelperError> {
        let scval = ScVal::from_xdr(bytes, Limits::none())
            .map_err(|e| SorobanHelperError::XdrEncodingFailed(e.to_string()))?;
        Self::from_scval(&scval)
    }

    pub(crate) fn to_scval(&self) -> Result<ScVal, SorobanHelperError> {
        struct_map(vec![
            ("message", self.message.clone().into_val_ext()?),
            ("trigger_id", ScVal::U64(self.trigger_id)),
        ])
    }

    pub(crate) fn from_scval(scval: &ScVal) -> Result<Self, SorobanHelperError> {
        let entries = match scval {
            ScVal::Map(Some(m)) => &m.0,
            _ => {
                return Err(SorobanHelperError::XdrEncodingFailed(
                    "MessageWithId: expected ScVal::Map".to_string(),
                ));
            }
        };

        let mut trigger_id: Option<u64> = None;
        let mut message: Option<Vec<u8>> = None;
        for entry in entries.iter() {
            let key = match &entry.key {
                ScVal::Symbol(ScSymbol(s)) => s.to_string(),
                _ => continue,
            };
            match (key.as_str(), &entry.val) {
                ("trigger_id", ScVal::U64(v)) => trigger_id = Some(*v),
                ("message", ScVal::Bytes(ScBytes(b))) => message = Some(b.to_vec()),
                _ => {}
            }
        }

        Ok(Self {
            trigger_id: trigger_id.ok_or_else(|| {
                SorobanHelperError::XdrEncodingFailed("MessageWithId: missing trigger_id".into())
            })?,
            message: message.ok_or_else(|| {
                SorobanHelperError::XdrEncodingFailed("MessageWithId: missing message".into())
            })?,
        })
    }
}
