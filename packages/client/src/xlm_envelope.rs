//! Std/client-side mirror of `warpdrive_shared::interfaces::handler::XlmEnvelope`.
//!
//! The on-chain struct is defined with `#[contracttype]` in `warpdrive-shared`,
//! but that crate pulls in `soroban-sdk` and cannot be linked into the std-only
//! client. This module reimplements the same wire format (XDR-encoded
//! `ScVal::Map` with alphabetically-sorted symbol keys) so off-chain code can
//! build and parse `XlmEnvelope` payloads. The roundtrip is covered by
//! `tests.rs::xlm_envelope_*`.
use wasi_soroban_rs::SorobanHelperError;
use wasi_soroban_rs::xdr::{Limits, ReadXdr, ScBytes, ScSymbol, ScVal, WriteXdr};

use crate::scval::{IntoScValExt, struct_map};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XlmEnvelope {
    pub event_id: [u8; 20],
    pub ordering: [u8; 12],
    pub payload: Vec<u8>,
}

impl XlmEnvelope {
    pub fn new(
        payload: Vec<u8>,
        event_id: [u8; 20],
        ordering: [u8; 12],
    ) -> Self {
        Self {
            payload,
            event_id,
            ordering,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, SorobanHelperError> {
        self.to_xdr_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, SorobanHelperError> {
        Self::from_xdr_bytes(bytes)
    }

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
            ("event_id", self.event_id.into_val_ext()?),
            ("ordering", self.ordering.into_val_ext()?),
            ("payload", self.payload.clone().into_val_ext()?),
        ])
    }

    pub(crate) fn from_scval(scval: &ScVal) -> Result<Self, SorobanHelperError> {
        let entries = match scval {
            ScVal::Map(Some(m)) => &m.0,
            _ => {
                return Err(SorobanHelperError::XdrEncodingFailed(
                    "XlmEnvelope: expected ScVal::Map".to_string(),
                ));
            }
        };

        let mut event_id: Option<[u8; 20]> = None;
        let mut ordering: Option<[u8; 12]> = None;
        let mut payload: Option<Vec<u8>> = None;
        for entry in entries.iter() {
            let key = match &entry.key {
                ScVal::Symbol(ScSymbol(s)) => s.to_string(),
                _ => continue,
            };
            match (key.as_str(), &entry.val) {
                ("event_id", ScVal::Bytes(ScBytes(b))) => {
                    event_id = Some(b.as_slice().try_into().map_err(|_| {
                        SorobanHelperError::XdrEncodingFailed(
                            "XlmEnvelope: event_id must be 20 bytes".into(),
                        )
                    })?);
                }
                ("ordering", ScVal::Bytes(ScBytes(b))) => {
                    ordering = Some(b.as_slice().try_into().map_err(|_| {
                        SorobanHelperError::XdrEncodingFailed(
                            "XlmEnvelope: ordering must be 12 bytes".into(),
                        )
                    })?);
                }
                ("payload", ScVal::Bytes(ScBytes(b))) => payload = Some(b.to_vec()),
                _ => {}
            }
        }

        Ok(Self {
            event_id: event_id.ok_or_else(|| {
                SorobanHelperError::XdrEncodingFailed("XlmEnvelope: missing event_id".into())
            })?,
            ordering: ordering.ok_or_else(|| {
                SorobanHelperError::XdrEncodingFailed("XlmEnvelope: missing ordering".into())
            })?,
            payload: payload.ok_or_else(|| {
                SorobanHelperError::XdrEncodingFailed("XlmEnvelope: missing payload".into())
            })?,
        })
    }
}
