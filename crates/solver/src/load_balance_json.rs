//! Exact numeric JSON snapshots of native load balancing observations.
//! Requires serde_json's arbitrary_precision feature for decimal readback.
use crate::{Decimal, JsonValue, LoadBalance};
use std::{convert::Infallible, error::Error, fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadBalanceJsonError {
    ExpectedObject,
    MissingUnfairness,
    ExpectedNumericUnfairness,
    InvalidDecimal,
    LoadsUnavailable,
}
impl fmt::Display for LoadBalanceJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ExpectedObject => "load balance snapshot must be a JSON object",
            Self::MissingUnfairness => "load balance snapshot is missing unfairness",
            Self::ExpectedNumericUnfairness => "load balance unfairness must be a JSON number",
            Self::InvalidDecimal => "load balance unfairness is outside the native decimal format",
            Self::LoadsUnavailable => "loads are unavailable in a reduced load balance snapshot",
        })
    }
}
impl Error for LoadBalanceJsonError {}

/// A readback view contains unfairness only; it cannot reconstruct load keys,
/// multiplicities or loads from an aggregate scalar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReducedLoadBalance {
    unfairness: Decimal,
}
impl ReducedLoadBalance {
    pub fn unfairness(&self) -> &Decimal {
        &self.unfairness
    }
    /// Always rejects access, explicitly distinguishing reduced readback from a
    /// collector-produced LoadBalance with its retained loads. Infallible makes
    /// it impossible to mistake this view for a successfully decoded load map.
    pub fn loads(&self) -> Result<Infallible, LoadBalanceJsonError> {
        Err(LoadBalanceJsonError::LoadsUnavailable)
    }
}

/// Serialize the actual collector observation, retaining its exact decimal as
/// a JSON number. No load identities or load map are emitted.
pub fn load_balance_to_json<K>(
    balance: &LoadBalance<K>,
) -> Result<JsonValue, LoadBalanceJsonError> {
    let number = serde_json::Number::from_str(&balance.unfairness().to_standard_string())
        .map_err(|_| LoadBalanceJsonError::InvalidDecimal)?;
    let mut fields = serde_json::Map::new();
    fields.insert("unfairness".into(), JsonValue::Number(number));
    Ok(JsonValue::Object(fields))
}

/// Read the reduced numeric snapshot. Exact decimal parsing preserves scale;
/// conversion through binary floating point is deliberately avoided.
pub fn load_balance_from_json(
    value: &JsonValue,
) -> Result<ReducedLoadBalance, LoadBalanceJsonError> {
    let fields = value
        .as_object()
        .ok_or(LoadBalanceJsonError::ExpectedObject)?;
    let number = fields
        .get("unfairness")
        .ok_or(LoadBalanceJsonError::MissingUnfairness)?;
    let JsonValue::Number(number) = number else {
        return Err(LoadBalanceJsonError::ExpectedNumericUnfairness);
    };
    let unfairness = number
        .to_string()
        .parse()
        .map_err(|_| LoadBalanceJsonError::InvalidDecimal)?;
    Ok(ReducedLoadBalance { unfairness })
}
