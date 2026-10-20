//! Deterministic thresholds with exact or explicit binary64 rain accumulation.
use crate::{AcceptanceError, BigInt, Decimal, Score, ScoreLevel};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GreatDelugeRise<S> {
    /// Nonnegative ordinary increments, added after each completed step.
    Score(S),
    /// Positive rational fraction of the absolute initial water level per step.
    /// Fractional rain accumulates before rounding, rather than rounding each drop.
    Ratio { numerator: u64, denominator: u64 },
    /// Explicit binary64 accumulation and integer-floor thresholds. Initial
    /// water components must have scale zero and fit i64. Exact Ratio remains
    /// the default configured policy. See [`GreatDeluge`] for numeric limits.
    Binary64Ratio { numerator: u64, denominator: u64 },
}
#[derive(Clone, Debug)]
enum Rain {
    Fixed(Vec<Decimal>),
    Ratio {
        base: Vec<Decimal>,
        drop: Vec<BigInt>,
        accumulated: Vec<BigInt>,
        denominator: BigInt,
    },
    Binary64Ratio {
        base: Vec<f64>,
        drop: f64,
        accumulated: f64,
        steps: u64,
    },
}
/// Candidates meeting the water level, or strictly improving the current score,
/// are accepted. Each completed step advances the rain, even a rejected fallback.
/// Trials and changes in the retained best do not change the water level.
///
/// Score and Ratio arithmetic avoid integer overflow and binary-floating drift. Ratio rain
/// is rounded down only when forming the threshold, at each initial level's scale
/// (at least zero). This preserves fractional accumulation for integer scores and
/// decimal precision for decimal scores. Java floating-point/overflow equivalence
/// outside the documented comparison cases is not claimed.
///
/// The opt-in Binary64Ratio instead adds the binary64 ratio once per completed
/// step, then separately multiplies each absolute initial component, adds the
/// initial component and floors. Integer thresholds are decoded exactly, without
/// narrowing to i64. Initial water must have scale zero and fit i64. At most
/// u64::MAX rises are performed; later calls leave water unchanged. This bounds
/// every floating operation to finite values, even for extreme valid ratios.
/// Initial conversion may round down, so the first binary64 threshold can be
/// below the exact initial water. Subsequent thresholds are nondecreasing.
/// This policy does not establish general Java arithmetic or trace equivalence.
///
/// ```
/// use timetuck::{GreatDeluge, GreatDelugeRise, SimpleScore as S};
/// let mut a = GreatDeluge::new(None, GreatDelugeRise::Ratio {
///     numerator: 1, denominator: 10
/// }, S::new(-8))?;
/// a.step_ended();
/// assert!(a.accepts(S::new(-5), S::new(-8)));
/// a.step_ended();
/// assert!(!a.accepts(S::new(-6), S::new(-8)));
/// assert!(a.accepts(S::new(-6), S::new(-7)));
/// # Ok::<(), timetuck::AcceptanceError>(())
/// ```
#[derive(Clone, Debug)]
pub struct GreatDeluge<S> {
    score_type: std::marker::PhantomData<S>,
    water: Vec<Decimal>,
    rain: Rain,
}
fn levels<S: Score>(s: &S) -> Result<Vec<Decimal>, AcceptanceError> {
    if s.structural() != 0 {
        return Err(AcceptanceError::InvalidWaterLevel);
    }
    s.ordinary_levels()
        .map(|v| {
            v.exact_decimal()
                .ok_or(AcceptanceError::UnsupportedScoreArithmetic)
        })
        .collect()
}
fn binary64_base(water: &[Decimal]) -> Result<Vec<f64>, AcceptanceError> {
    water
        .iter()
        .map(|level| {
            if level.scale() != 0 {
                return Err(AcceptanceError::UnsupportedScoreArithmetic);
            }
            i64::try_from(level.coefficient())
                .map(|n| n as f64)
                .map_err(|_| AcceptanceError::UnsupportedScoreArithmetic)
        })
        .collect()
}
fn binary64_integer(value: f64) -> BigInt {
    debug_assert!(value.is_finite() && value.fract() == 0.0);
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        return BigInt::ZERO;
    }
    let mantissa = BigInt::from((bits & ((1u64 << 52) - 1)) | (1u64 << 52));
    let shift = exponent - 1023 - 52;
    let integer = if shift >= 0 {
        mantissa << shift as usize
    } else {
        mantissa >> (-shift) as usize
    };
    if bits >> 63 == 0 { integer } else { -integer }
}
impl<S: Score> GreatDeluge<S> {
    pub(crate) fn validate(
        initial: Option<&S>,
        rise: &GreatDelugeRise<S>,
    ) -> Result<(), AcceptanceError> {
        if S::Level::zero().exact_decimal().is_none() {
            return Err(AcceptanceError::UnsupportedScoreArithmetic);
        }
        if let Some(initial) = initial {
            let water = levels(initial)?;
            if matches!(rise, GreatDelugeRise::Binary64Ratio { .. }) {
                binary64_base(&water)?;
            }
        }
        match rise {
            GreatDelugeRise::Score(s) => {
                if levels(s)?
                    .iter()
                    .any(|v| v.numeric_cmp(&Decimal::ZERO).is_lt())
                {
                    return Err(AcceptanceError::InvalidWaterLevelIncrement);
                }
            }
            GreatDelugeRise::Ratio {
                numerator,
                denominator,
            }
            | GreatDelugeRise::Binary64Ratio {
                numerator,
                denominator,
            } => {
                if *numerator == 0 || *denominator == 0 {
                    return Err(AcceptanceError::InvalidWaterLevelIncrement);
                }
            }
        }
        Ok(())
    }
    pub fn new(
        initial: Option<S>,
        rise: GreatDelugeRise<S>,
        initial_score: S,
    ) -> Result<Self, AcceptanceError> {
        Self::validate(initial.as_ref(), &rise)?;
        levels(&initial_score)?;
        let water = levels(initial.as_ref().unwrap_or(&initial_score))?;
        let rain = match rise {
            GreatDelugeRise::Score(s) => Rain::Fixed(levels(&s)?),
            GreatDelugeRise::Ratio {
                numerator,
                denominator,
            } => {
                // Normalize negative scales to unit precision, as score multiplication
                // does. This does not round the initial numeric value.
                let base: Vec<_> = water
                    .iter()
                    .map(|v| {
                        v.multiply_floor(&Decimal::from(1))
                            .expect("normalizing an i32 decimal scale to zero fits u32")
                    })
                    .collect();
                let drop = base
                    .iter()
                    .map(|v| {
                        BigInt::from(v.coefficient().magnitude().clone()) * BigInt::from(numerator)
                    })
                    .collect();
                Rain::Ratio {
                    accumulated: vec![BigInt::ZERO; base.len()],
                    base,
                    drop,
                    denominator: denominator.into(),
                }
            }
            GreatDelugeRise::Binary64Ratio {
                numerator,
                denominator,
            } => Rain::Binary64Ratio {
                base: binary64_base(&water)?,
                drop: numerator as f64 / denominator as f64,
                accumulated: 0.0,
                steps: 0,
            },
        };
        Ok(Self {
            water,
            rain,
            score_type: std::marker::PhantomData,
        })
    }
    pub fn water_levels(&self) -> &[Decimal] {
        &self.water
    }
    pub fn accepts(&self, current: S, candidate: S) -> bool {
        if candidate.structural() != 0 {
            return false;
        }
        if candidate > current {
            return true;
        }
        for (value, water) in candidate.ordinary_levels().zip(&self.water) {
            let value = value
                .exact_decimal()
                .expect("ScoreLevel arithmetic capability must be consistent");
            match value.numeric_cmp(water) {
                Ordering::Greater => return true,
                Ordering::Less => return false,
                Ordering::Equal => {}
            }
        }
        true
    }
    pub fn step_ended(&mut self) {
        match &mut self.rain {
            Rain::Fixed(drop) => {
                for (water, drop) in self.water.iter_mut().zip(drop) {
                    *water = water.add(drop);
                }
            }
            Rain::Ratio {
                base,
                drop,
                accumulated,
                denominator,
            } => {
                for (((water, base), drop), sum) in
                    self.water.iter_mut().zip(base).zip(drop).zip(accumulated)
                {
                    *sum += &*drop;
                    *water = base.add(&Decimal::new(&*sum / &*denominator, base.scale()));
                }
            }
            Rain::Binary64Ratio {
                base,
                drop,
                accumulated,
                steps,
            } => {
                if *steps == u64::MAX {
                    return;
                }
                *steps += 1;
                *accumulated += *drop;
                for (water, base) in self.water.iter_mut().zip(base) {
                    let rise = base.abs() * *accumulated;
                    *water = Decimal::new(binary64_integer((*base + rise).floor()), 0);
                }
            }
        }
    }
}

#[cfg(test)]
mod binary64_tests {
    use super::*;
    #[test]
    fn completed_step_saturation_keeps_finite_water_and_freezes_all_rain_state() {
        let mut a = GreatDeluge::new(
            None,
            GreatDelugeRise::Binary64Ratio {
                numerator: u64::MAX,
                denominator: 1,
            },
            crate::SimpleScore::new(i64::MIN),
        )
        .unwrap();
        let Rain::Binary64Ratio {
            steps,
            accumulated,
            drop,
            ..
        } = &mut a.rain
        else {
            unreachable!()
        };
        *steps = u64::MAX - 1;
        *accumulated = *drop * (u64::MAX - 1) as f64;
        a.step_ended();
        let saved = a.water.clone();
        let Rain::Binary64Ratio {
            steps, accumulated, ..
        } = &a.rain
        else {
            unreachable!()
        };
        assert_eq!(*steps, u64::MAX);
        assert!(accumulated.is_finite());
        let sum = *accumulated;
        a.step_ended();
        assert_eq!(a.water, saved);
        let Rain::Binary64Ratio {
            steps, accumulated, ..
        } = &a.rain
        else {
            unreachable!()
        };
        assert_eq!(*steps, u64::MAX);
        assert_eq!(*accumulated, sum);
    }
}
