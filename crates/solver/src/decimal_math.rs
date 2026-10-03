//! Decimal text and exact, scale-directed arithmetic. Independent native code.
use crate::{BigInt, Decimal};
use num_bigint::Sign;
use std::{error::Error, fmt, str::FromStr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDecimalError;
impl fmt::Display for ParseDecimalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid decimal or scale outside i32")
    }
}
impl Error for ParseDecimalError {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecimalArithmeticError {
    DivisionByZero,
    ScaleOverflow,
}
impl fmt::Display for DecimalArithmeticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DivisionByZero => "decimal division by zero",
            Self::ScaleOverflow => "decimal scale adjustment exceeds u32",
        })
    }
}
impl Error for DecimalArithmeticError {}
impl FromStr for Decimal {
    type Err = ParseDecimalError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (mantissa, exponent) = match text.split_once(['e', 'E']) {
            Some((m, e)) => (m, e.parse::<i64>().map_err(|_| ParseDecimalError)?),
            None => (text, 0),
        };
        let (negative, mantissa) = if let Some(m) = mantissa.strip_prefix('-') {
            (true, m)
        } else {
            (false, mantissa.strip_prefix('+').unwrap_or(mantissa))
        };
        let mut digits = String::new();
        let mut fractional = 0i64;
        let mut dot = false;
        for c in mantissa.bytes() {
            match c {
                b'0'..=b'9' => {
                    digits.push(c as char);
                    if dot {
                        fractional += 1;
                    }
                }
                b'.' if !dot => dot = true,
                _ => return Err(ParseDecimalError),
            }
        }
        if digits.is_empty() {
            return Err(ParseDecimalError);
        }
        let scale = fractional
            .checked_sub(exponent)
            .and_then(|s| i32::try_from(s).ok())
            .ok_or(ParseDecimalError)?;
        let coefficient = BigInt::parse_bytes(digits.as_bytes(), 10).ok_or(ParseDecimalError)?;
        Ok(Self::new(
            if negative { -coefficient } else { coefficient },
            scale,
        ))
    }
}
impl Decimal {
    /// Scale-preserving decimal/scientific text. Unlike Display's coefficient-e
    /// notation, this uses plain notation when scale >= 0 and adjusted exponent
    /// >= -6, and scientific notation otherwise. Parsing accepts ASCII digits.
    pub fn to_standard_string(&self) -> String {
        let digits = self.coefficient().magnitude().to_str_radix(10);
        let adjusted = digits.len() as i64 - 1 - i64::from(self.scale());
        let sign = if self.coefficient().sign() == Sign::Minus {
            "-"
        } else {
            ""
        };
        if self.scale() >= 0 && adjusted >= -6 {
            let point = digits.len() as i64 - i64::from(self.scale());
            if self.scale() == 0 {
                format!("{sign}{digits}")
            } else if point > 0 {
                format!(
                    "{sign}{}.{}",
                    &digits[..point as usize],
                    &digits[point as usize..]
                )
            } else {
                format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
            }
        } else {
            let suffix = if digits.len() > 1 {
                format!(".{}", &digits[1..])
            } else {
                String::new()
            };
            format!("{sign}{}{suffix}E{adjusted:+}", &digits[..1])
        }
    }
    pub(crate) fn numeric_hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash;
        if self.coefficient() == &BigInt::ZERO {
            ("0", 0i64).hash(state);
            return;
        }
        let digits = self.coefficient().to_string();
        let significant = digits.trim_end_matches('0');
        (
            significant,
            i64::from(self.scale()) - (digits.len() - significant.len()) as i64,
        )
            .hash(state);
    }
    pub(crate) fn multiply_floor(&self, factor: &Self) -> Result<Self, DecimalArithmeticError> {
        let scale = self.scale().max(0);
        let coefficient = rescale(
            self.coefficient() * factor.coefficient(),
            i64::from(scale) - i64::from(self.scale()) - i64::from(factor.scale()),
        )?;
        Ok(Self::new(coefficient, scale))
    }
    pub(crate) fn divide_floor(&self, divisor: &Self) -> Result<Self, DecimalArithmeticError> {
        if divisor.coefficient() == &BigInt::ZERO {
            return Err(DecimalArithmeticError::DivisionByZero);
        }
        let scale = self.scale().max(0);
        if self.coefficient() == &BigInt::ZERO {
            return Ok(Self::new(BigInt::ZERO, scale));
        }
        let shift = i64::from(scale) + i64::from(divisor.scale()) - i64::from(self.scale());
        let mut numerator = self.coefficient().clone();
        let mut denominator = divisor.coefficient().clone();
        if shift >= 0 {
            numerator *= power_ten(shift as u64)?;
        } else {
            if shift.unsigned_abs() > numerator.magnitude().to_str_radix(10).len() as u64 {
                return Ok(Self::new(
                    if numerator.sign() != denominator.sign() {
                        (-1).into()
                    } else {
                        BigInt::ZERO
                    },
                    scale,
                ));
            }
            denominator *= power_ten(shift.unsigned_abs())?;
        }
        Ok(Self::new(floor_divide(numerator, denominator), scale))
    }
    pub(crate) fn power_floor(&self, exponent: u32) -> Result<Self, DecimalArithmeticError> {
        let scale = self.scale().max(0);
        let coefficient = rescale(
            self.coefficient().pow(exponent),
            i64::from(scale) - i64::from(self.scale()) * i64::from(exponent),
        )?;
        Ok(Self::new(coefficient, scale))
    }
}
fn power_ten(exponent: u64) -> Result<BigInt, DecimalArithmeticError> {
    Ok(BigInt::from(10).pow(
        exponent
            .try_into()
            .map_err(|_| DecimalArithmeticError::ScaleOverflow)?,
    ))
}
fn floor_divide(a: BigInt, b: BigInt) -> BigInt {
    let mut q = &a / &b;
    if a.sign() != b.sign() && a % b != BigInt::ZERO {
        q -= 1;
    }
    q
}
fn rescale(coefficient: BigInt, shift: i64) -> Result<BigInt, DecimalArithmeticError> {
    if coefficient == BigInt::ZERO {
        return Ok(coefficient);
    }
    if shift >= 0 {
        Ok(coefficient * power_ten(shift as u64)?)
    } else if shift.unsigned_abs() > coefficient.magnitude().to_str_radix(10).len() as u64 {
        Ok(if coefficient.sign() == Sign::Minus {
            (-1).into()
        } else {
            BigInt::ZERO
        })
    } else {
        Ok(floor_divide(coefficient, power_ten(shift.unsigned_abs())?))
    }
}
