//! Extension trait filling in `IntoScVal` impls that soroban-rs hasn't
//! released yet, plus the ones we specifically need in this crate:
//! arbitrary-length byte slices, byte arrays of any size, and vectors
//! of convertible values.
//!
//! Import `IntoScValExt` and call `.into_val_ext()` to get a `ScVal`.

use soroban_rs::SorobanHelperError;
use soroban_rs::xdr::{
    BytesM, Int128Parts, Int256Parts, ScBytes, ScError, ScMap, ScMapEntry, ScSymbol, ScVal, ScVec,
    StringM, TimePoint, UInt128Parts, UInt256Parts, VecM,
};

pub trait IntoScValExt {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError>;
}

// ── Upstream-pending scalars ────────────────────────────────────────────

impl IntoScValExt for () {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::Void)
    }
}

impl IntoScValExt for u128 {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::U128(UInt128Parts {
            hi: (self >> 64) as u64,
            lo: self as u64,
        }))
    }
}

impl IntoScValExt for i128 {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::I128(Int128Parts {
            hi: (self >> 64) as i64,
            lo: self as u64,
        }))
    }
}

impl IntoScValExt for UInt256Parts {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::U256(self))
    }
}

impl IntoScValExt for Int256Parts {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::I256(self))
    }
}

impl IntoScValExt for ScError {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::Error(self))
    }
}

impl IntoScValExt for TimePoint {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::Timepoint(self))
    }
}

impl IntoScValExt for ScMap {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        Ok(ScVal::Map(Some(self)))
    }
}

// ── Bytes ───────────────────────────────────────────────────────────────

impl IntoScValExt for Vec<u8> {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        let bm = BytesM::<{ u32::MAX }>::try_from(self)
            .map_err(|_| SorobanHelperError::XdrEncodingFailed("bytes too long".to_string()))?;
        Ok(ScVal::Bytes(ScBytes::from(bm)))
    }
}

impl<const N: usize> IntoScValExt for [u8; N] {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        self.to_vec().into_val_ext()
    }
}

// ── Vectors ─────────────────────────────────────────────────────────────

impl<T: IntoScValExt> IntoScValExt for Vec<T> {
    fn into_val_ext(self) -> Result<ScVal, SorobanHelperError> {
        let vals: Vec<ScVal> = self
            .into_iter()
            .map(|v| v.into_val_ext())
            .collect::<Result<_, _>>()?;
        let vm = VecM::try_from(vals)
            .map_err(|_| SorobanHelperError::XdrEncodingFailed("vec too long".to_string()))?;
        Ok(ScVal::Vec(Some(ScVec::from(vm))))
    }
}

// ── Helpers for contract-struct encoding ────────────────────────────────

/// Build an `ScVal::Symbol` from a short string.
pub fn symbol(key: &str) -> Result<ScVal, SorobanHelperError> {
    let sm: StringM<32> = StringM::try_from(key)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("symbol too long".to_string()))?;
    Ok(ScVal::Symbol(ScSymbol(sm)))
}

/// Encode a contract struct as `ScVal::Map`.
pub(crate) fn struct_map(mut entries: Vec<(&str, ScVal)>) -> Result<ScVal, SorobanHelperError> {
    // Entries must be provided to Soroban in alphabetical field-name order
    entries.sort_by_key(|v| v.0);

    let map_entries: Vec<ScMapEntry> = entries
        .into_iter()
        .map(|(k, v)| {
            Ok(ScMapEntry {
                key: symbol(k)?,
                val: v,
            })
        })
        .collect::<Result<_, SorobanHelperError>>()?;
    let vm = VecM::try_from(map_entries)
        .map_err(|_| SorobanHelperError::XdrEncodingFailed("map too long".to_string()))?;
    ScMap(vm).into_val_ext()
}
