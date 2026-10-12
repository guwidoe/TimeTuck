//! Checked signed integer combinatorial counts and continuous inverse roots.
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriangularError {
    NegativeArgument,
    IntegerOverflow,
}
impl fmt::Display for TriangularError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NegativeArgument => "triangular argument must not be negative",
            Self::IntegerOverflow => "integer overflow",
        })
    }
}
impl Error for TriangularError {}
/// Largest n with n*(n+1) representable as a signed32-bit intermediate.
/// Derived mathematically from that bound, not from an upstream implementation.
pub const HIGHEST_SAFE_TRIANGULAR_N: i32 = 46_340;
pub fn nth_triangle(n: i32) -> Result<i32, TriangularError> {
    if n < 0 {
        return Err(TriangularError::NegativeArgument);
    }
    n.checked_add(1)
        .and_then(|next| n.checked_mul(next))
        .map(|v| v / 2)
        .ok_or(TriangularError::IntegerOverflow)
}
/// Continuous inverse of n*(n+1)/2. Nontriangular inputs lie strictly between
/// adjacent integer roots; i32 inputs retain exact intermediate representation.
pub fn triangular_root(value: i32) -> Result<f64, TriangularError> {
    if value < 0 {
        return Err(TriangularError::NegativeArgument);
    }
    Ok(((8.0 * f64::from(value) + 1.0).sqrt() - 1.0) / 2.0)
}
