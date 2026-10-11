//! Original heap-owned score with clone instrumentation for snapshot costs.
use std::{
    cell::Cell,
    cmp::Ordering,
    ops::{Add, Sub},
};
use timetuck::{AdditiveScore, Score};
thread_local! {static CLONES:Cell<usize>=const {Cell::new(0)};}
pub fn reset_clones() {
    CLONES.with(|c| c.set(0));
}
pub fn clones() -> usize {
    CLONES.with(Cell::get)
}
#[derive(Debug, Default)]
pub struct OwnedScore {
    levels: Vec<i64>,
    structural: i64,
}
impl Clone for OwnedScore {
    fn clone(&self) -> Self {
        CLONES.with(|c| c.set(c.get() + 1));
        Self {
            levels: self.levels.clone(),
            structural: self.structural,
        }
    }
}
impl OwnedScore {
    pub fn new(hard: i64, soft: i64) -> Self {
        Self {
            levels: vec![hard, soft],
            structural: 0,
        }
    }
    pub fn hard(&self) -> i64 {
        self.levels.first().copied().unwrap_or(0)
    }
    pub fn soft(&self) -> i64 {
        self.levels.get(1).copied().unwrap_or(0)
    }
}
impl PartialEq for OwnedScore {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for OwnedScore {}
impl PartialOrd for OwnedScore {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for OwnedScore {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.structural, self.hard(), self.soft()).cmp(&(o.structural, o.hard(), o.soft()))
    }
}
impl Score for OwnedScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 1;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        [self.hard(), self.soft()].into_iter()
    }
    fn structural(&self) -> i64 {
        self.structural
    }
    fn hard_cmp(&self, o: &Self) -> Ordering {
        self.hard().cmp(&o.hard())
    }
    fn is_feasible(&self) -> bool {
        self.structural >= 0 && self.hard() >= 0
    }
}
impl AdditiveScore for OwnedScore {
    const ZERO: Self = Self {
        levels: Vec::new(),
        structural: 0,
    };
    fn with_structural(mut self, s: i64) -> Self {
        self.structural = s;
        self
    }
}
impl Add for OwnedScore {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(
            self.hard().wrapping_add(o.hard()),
            self.soft().wrapping_add(o.soft()),
        )
    }
}
impl Sub for OwnedScore {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(
            self.hard().wrapping_sub(o.hard()),
            self.soft().wrapping_sub(o.soft()),
        )
    }
}
