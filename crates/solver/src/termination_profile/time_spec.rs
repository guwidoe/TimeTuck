//! Lossless configuration inputs, normalized once before solver binding.
use super::TerminationProfileError;
use std::time::Duration;

/// Additive time units. None is absent; Some(0) is an explicit zero budget.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeLimitUnits {
    pub milliseconds: Option<u64>,
    pub seconds: Option<u64>,
    pub minutes: Option<u64>,
    pub hours: Option<u64>,
    pub days: Option<u64>,
}
/// Keep input representation for configuration inspection and replacement.
/// Runtime search receives only the resolved Duration; no unit work is repeated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeLimitSpec {
    Duration(Duration),
    Units(TimeLimitUnits),
}
impl TimeLimitSpec {
    pub fn units(self) -> Option<TimeLimitUnits> {
        match self {
            Self::Units(units) => Some(units),
            Self::Duration(_) => None,
        }
    }
    /// Empty units disable the budget. Overflow is rejected before binding.
    pub fn resolve(self) -> Result<Option<Duration>, TerminationProfileError> {
        let u = match self {
            Self::Duration(duration) => return Ok(Some(duration)),
            Self::Units(units) => units,
        };
        let configured = [u.milliseconds, u.seconds, u.minutes, u.hours, u.days]
            .iter()
            .any(Option::is_some);
        if !configured {
            return Ok(None);
        }
        let mut duration = Duration::from_millis(u.milliseconds.unwrap_or(0));
        for (value, multiplier) in [
            (u.seconds, 1),
            (u.minutes, 60),
            (u.hours, 3600),
            (u.days, 86400),
        ] {
            if let Some(value) = value {
                let part = Duration::from_secs(value)
                    .checked_mul(multiplier)
                    .ok_or(TerminationProfileError::TimeOverflow)?;
                duration = duration
                    .checked_add(part)
                    .ok_or(TerminationProfileError::TimeOverflow)?;
            }
        }
        Ok(Some(duration))
    }
}
