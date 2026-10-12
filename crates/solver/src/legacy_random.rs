//! Optional bounded 48-bit LCG for reproducible public Java Random input streams.
//! Specification: https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/util/Random.html
//! Native solver defaults continue to use their existing random generators.
use std::{error::Error, fmt, num::NonZeroU32};
#[derive(Clone, Debug)]
pub struct Lcg48Random {
    state: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidLcg48Bound(pub u32);
impl fmt::Display for InvalidLcg48Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LCG48 bound {} must be at most i32::MAX", self.0)
    }
}
impl Error for InvalidLcg48Bound {}
impl Lcg48Random {
    pub fn new(seed: u64) -> Self {
        Self {
            state: (seed ^ 0x5deece66d) & ((1 << 48) - 1),
        }
    }
    fn bits(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x5deece66d).wrapping_add(11) & ((1 << 48) - 1);
        (self.state >> 17) as u32
    }
    /// One-element bounds still consume a draw; invalid bounds consume none.
    pub fn below(&mut self, bound: NonZeroU32) -> Result<u32, InvalidLcg48Bound> {
        let n = bound.get();
        if n > i32::MAX as u32 {
            return Err(InvalidLcg48Bound(n));
        }
        if n.is_power_of_two() {
            return Ok(((u64::from(n) * u64::from(self.bits())) >> 31) as u32);
        }
        loop {
            let bits = self.bits();
            let result = bits % n;
            if u64::from(bits - result) + u64::from(n - 1) <= i32::MAX as u64 {
                return Ok(result);
            }
        }
    }
}
