//! Validated fixed and population-relative tabu sizing.
use crate::AcceptanceError;
use std::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Sizing {
    Fixed(usize),
    Ratio(u64),
}
/// A validated fixed tenure or finite ratio in [0, 1]. Ratios use nearest-integer
/// rounding with positive halves rounded up, then clamp to [1, population - 1].
/// Empty/singleton populations resolve to one. Counts include pinned entries;
/// this sizes completed-step retention, not the number of stored keys.
///
/// Ratios use binary64 multiplication, matching the representation of Java
/// configuration values. Above binary64's exact-integer range, multiplication
/// may round the population; the final integer cap is still exact.
///
/// Fixed signed inputs make zero/negative rejection explicit. Existing usize
/// acceptance configurations remain available for larger fixed native tenures.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct TabuSize(Sizing);
impl TabuSize {
    pub fn fixed(size: i64) -> Result<Self, AcceptanceError> {
        if size <= 0 {
            return Err(AcceptanceError::EmptyTabu);
        }
        let size = usize::try_from(size).map_err(|_| AcceptanceError::TabuTenureOverflow)?;
        Ok(Self(Sizing::Fixed(size)))
    }
    pub fn ratio(ratio: f64) -> Result<Self, AcceptanceError> {
        if !ratio.is_finite() || !(0.0..=1.0).contains(&ratio) {
            return Err(AcceptanceError::InvalidTabuRatio);
        }
        // Canonicalize signed zero so equality reflects the sizing policy.
        Ok(Self(Sizing::Ratio(if ratio == 0.0 {
            0
        } else {
            ratio.to_bits()
        })))
    }
    pub fn resolve(self, population: usize) -> usize {
        match self.0 {
            Sizing::Fixed(size) => size,
            Sizing::Ratio(bits) => {
                let rounded = (population as f64 * f64::from_bits(bits)).round() as usize;
                rounded.max(1).min(population.saturating_sub(1).max(1))
            }
        }
    }
}
impl fmt::Debug for TabuSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Sizing::Fixed(size) => f.debug_tuple("Fixed").field(&size).finish(),
            Sizing::Ratio(bits) => f.debug_tuple("Ratio").field(&f64::from_bits(bits)).finish(),
        }
    }
}
