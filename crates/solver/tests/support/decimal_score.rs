//! Original numeric decimal score fixture, not an upstream score-family adaptation.
use std::{
    cmp::Ordering,
    ops::{Add, Sub},
};
use timetuck::{AdditiveScore, Decimal, Score};
#[derive(Clone, Debug)]
pub struct DecimalScore {
    pub hard: Decimal,
    pub soft: Decimal,
    structural: i64,
}
impl DecimalScore {
    pub fn new(hard: Decimal, soft: Decimal) -> Self {
        Self {
            hard,
            soft,
            structural: 0,
        }
    }
    pub fn scaled(hard: i64, soft: i64, scale: i32) -> Self {
        Self::new(
            Decimal::new(hard.into(), scale),
            Decimal::new(soft.into(), scale),
        )
    }
}
impl PartialEq for DecimalScore {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for DecimalScore {}
impl PartialOrd for DecimalScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for DecimalScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.structural
            .cmp(&other.structural)
            .then_with(|| self.hard.numeric_cmp(&other.hard))
            .then_with(|| self.soft.numeric_cmp(&other.soft))
    }
}
impl Score for DecimalScore {
    type Level = Decimal;
    const HARD_LEVEL_COUNT: usize = 1;
    fn ordinary_levels(&self) -> impl Iterator<Item = Decimal> {
        [self.hard.clone(), self.soft.clone()].into_iter()
    }
    fn structural(&self) -> i64 {
        self.structural
    }
    fn hard_cmp(&self, other: &Self) -> Ordering {
        self.hard.numeric_cmp(&other.hard)
    }
    fn is_feasible(&self) -> bool {
        self.structural >= 0 && !self.hard.numeric_cmp(&Decimal::ZERO).is_lt()
    }
}
impl AdditiveScore for DecimalScore {
    const ZERO: Self = Self {
        hard: Decimal::ZERO,
        soft: Decimal::ZERO,
        structural: 0,
    };
    fn with_structural(mut self, value: i64) -> Self {
        self.structural = value;
        self
    }
}
impl Add for DecimalScore {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.hard.add(&other.hard), self.soft.add(&other.soft))
    }
}
impl Sub for DecimalScore {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(
            self.hard.subtract(&other.hard),
            self.soft.subtract(&other.soft),
        )
    }
}
