//! Independent openings over the native seeded stream.
use super::SplitMix64;
/// Each opening starts from its configured seed. Native SplitMix64 sequences are
/// reproducible; this is not Java RandomSource/SplittableRandom compatibility.
#[derive(Clone, Debug)]
pub struct FactoryRandomStream {
    seed: u64,
    random: SplitMix64,
}
impl FactoryRandomStream {
    pub fn seeded_factory_stream(seed: u64) -> Self {
        Self {
            seed,
            random: SplitMix64::new(seed),
        }
    }
    pub fn factory_stream_seed(&self) -> u64 {
        self.seed
    }
    pub fn next_factory_word(&mut self) -> u64 {
        self.random.next()
    }
}
