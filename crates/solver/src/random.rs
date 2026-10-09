//! Reproducible search randomness; not for cryptography or Java PRNG compatibility.
#[derive(Clone, Debug)]
pub(crate) struct SplitMix64 {
    state: u64,
}
impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    /// Uniform grid of 2^53 values in [0, 1).
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
    pub(crate) fn below(&mut self, bound: u64) -> u64 {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return value % bound;
            }
        }
    }
    pub(crate) fn below_wide(&mut self, bound: u128) -> u128 {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = (u128::from(self.next()) << 64) | u128::from(self.next());
            if value >= threshold {
                return value % bound;
            }
        }
    }
    pub(crate) fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.below(i as u64 + 1) as usize;
            values.swap(i, j);
        }
    }
}
