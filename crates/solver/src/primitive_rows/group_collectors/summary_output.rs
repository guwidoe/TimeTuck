//! Complete owned result descriptions captured at the original summary phase.
use crate::{IntCollector, IntSummary};
pub(super) trait Output {
    type Seed;
    type Row: Copy;
    const NEEDS_SLOT: bool;
    fn prepare(value: &IntCollector) -> Self::Seed;
    fn count(seed: &Self::Seed) -> usize;
    fn finish(seed: Self::Seed, slot: usize) -> Self::Row;
    fn row_count(row: &Self::Row) -> usize;
}
pub(super) struct Full;
impl Output for Full {
    type Seed = IntSummary;
    type Row = IntSummary;
    const NEEDS_SLOT: bool = false;
    #[inline(always)]
    fn prepare(value: &IntCollector) -> IntSummary {
        value.summary()
    }
    #[inline(always)]
    fn count(seed: &IntSummary) -> usize {
        seed.count
    }
    #[inline(always)]
    fn finish(seed: IntSummary, _: usize) -> IntSummary {
        seed
    }
    #[inline(always)]
    fn row_count(row: &IntSummary) -> usize {
        row.count
    }
}
pub(super) struct Deferred;
impl Output for Deferred {
    type Seed = crate::int_collector_counts::SummarySeed;
    type Row = Self::Seed;
    const NEEDS_SLOT: bool = false;
    #[inline(always)]
    fn prepare(value: &IntCollector) -> Self::Seed {
        value.summary_seed()
    }
    #[inline(always)]
    fn count(seed: &Self::Seed) -> usize {
        seed.count()
    }
    #[inline(always)]
    fn finish(seed: Self::Seed, _: usize) -> Self::Row {
        seed
    }
    #[inline(always)]
    fn row_count(row: &Self::Row) -> usize {
        row.count()
    }
}
