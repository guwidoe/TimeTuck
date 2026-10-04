//! Calendar periods with independent year/month/day fields and checked sums.
use crate::{ReversibleCollector, SnapshotMap};
use std::{error::Error, fmt};

/// A calendar field or collector occurrence count exceeded its supported range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeriodOverflow;
impl fmt::Display for PeriodOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("calendar period arithmetic overflow")
    }
}
impl Error for PeriodOverflow {}

/// A date-based amount with independent signed i32 years, months and days.
/// Construction and addition do not normalize: 12 months and 1 year are different
/// values. Days never convert to months or fixed-duration seconds. No chronological
/// ordering is defined without a calendar anchor; this type deliberately has no Ord.
/// It does not apply amounts to dates, resolve time zones or parse text.
///
/// ```
/// use timetuck::CalendarPeriod;
/// let p=CalendarPeriod::new(1,12,-3);
/// assert_eq!(p.checked_add(CalendarPeriod::from_months(1))?.components(),[1,13,-3]);
/// assert_ne!(CalendarPeriod::from_years(1),CalendarPeriod::from_months(12));
/// assert_eq!(p.to_string(),"P1Y12M-3D");
/// # Ok::<(),timetuck::PeriodOverflow>(())
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct CalendarPeriod([i32; 3]);
impl CalendarPeriod {
    pub const ZERO: Self = Self([0; 3]);
    pub const fn new(years: i32, months: i32, days: i32) -> Self {
        Self([years, months, days])
    }
    pub const fn from_years(years: i32) -> Self {
        Self::new(years, 0, 0)
    }
    pub const fn from_months(months: i32) -> Self {
        Self::new(0, months, 0)
    }
    pub const fn from_days(days: i32) -> Self {
        Self::new(0, 0, days)
    }
    pub const fn components(self) -> [i32; 3] {
        self.0
    }
    pub const fn years(self) -> i32 {
        self.0[0]
    }
    pub const fn months(self) -> i32 {
        self.0[1]
    }
    pub const fn days(self) -> i32 {
        self.0[2]
    }
    pub fn checked_add(self, other: Self) -> Result<Self, PeriodOverflow> {
        Ok(Self::new(
            self.years()
                .checked_add(other.years())
                .ok_or(PeriodOverflow)?,
            self.months()
                .checked_add(other.months())
                .ok_or(PeriodOverflow)?,
            self.days()
                .checked_add(other.days())
                .ok_or(PeriodOverflow)?,
        ))
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, PeriodOverflow> {
        Ok(Self::new(
            self.years()
                .checked_sub(other.years())
                .ok_or(PeriodOverflow)?,
            self.months()
                .checked_sub(other.months())
                .ok_or(PeriodOverflow)?,
            self.days()
                .checked_sub(other.days())
                .ok_or(PeriodOverflow)?,
        ))
    }
}
impl fmt::Display for CalendarPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self == Self::ZERO {
            return f.write_str("P0D");
        }
        f.write_str("P")?;
        for (value, unit) in self.0.into_iter().zip(['Y', 'M', 'D']) {
            if value != 0 {
                write!(f, "{value}{unit}")?;
            }
        }
        Ok(())
    }
}

/// Reversible calendar-period sum. Every field checks i32 overflow separately.
/// An empty collection finishes to CalendarPeriod::ZERO; zero-valued occurrences
/// still count as captured inputs. Missing removals/replacements return false.
///
/// Replacement subtracts the old period and then adds the new period, checking
/// both intermediates. Even equal-value replacement can overflow during subtraction
/// if other periods cancel in the current total. This follows public Community
/// observations; there is no identity or equality shortcut. Try methods preserve
/// state on failure; trait methods panic and participate in graph rollback.
/// Keep every intermediate sum representable when requiring rebuild diagnostics.
///
/// Clones share immutable persistent occurrence roots, keyed internally by the
/// three fields; that bookkeeping order is not chronological period comparison.
/// Outputs are owned values, with no live view or automatic normalization.
///
/// ```
/// use timetuck::{CalendarPeriod,PeriodSumCollector,ReversibleCollector};
/// let mut c=PeriodSumCollector::default();
/// c.try_insert(CalendarPeriod::new(1,12,3))?;
/// c.try_insert(CalendarPeriod::new(-1,1,-3))?;
/// assert_eq!(c.finish(),CalendarPeriod::from_months(13));
/// # Ok::<(),timetuck::PeriodOverflow>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PeriodSumCollector {
    values: SnapshotMap<[i32; 3], usize>,
    sum: CalendarPeriod,
}
impl PeriodSumCollector {
    pub fn try_insert(&mut self, value: CalendarPeriod) -> Result<(), PeriodOverflow> {
        let sum = self.sum.checked_add(value)?;
        let key = value.components();
        let n = self
            .values
            .get(&key)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or(PeriodOverflow)?;
        self.values.insert(key, n);
        self.sum = sum;
        Ok(())
    }
    pub fn try_remove(&mut self, value: CalendarPeriod) -> Result<bool, PeriodOverflow> {
        let key = value.components();
        let Some(&n) = self.values.get(&key) else {
            return Ok(false);
        };
        let sum = self.sum.checked_sub(value)?;
        if n == 1 {
            self.values.remove(&key);
        } else {
            self.values.insert(key, n - 1);
        }
        self.sum = sum;
        Ok(true)
    }
    pub fn try_replace(
        &mut self,
        old: CalendarPeriod,
        new: CalendarPeriod,
    ) -> Result<bool, PeriodOverflow> {
        let mut next = self.clone();
        if !next.try_remove(old)? {
            return Ok(false);
        }
        next.try_insert(new)?;
        *self = next;
        Ok(true)
    }
}
impl ReversibleCollector<CalendarPeriod> for PeriodSumCollector {
    type Output = CalendarPeriod;
    fn insert(&mut self, v: &CalendarPeriod) {
        self.try_insert(*v).expect("period insertion overflow");
    }
    fn remove(&mut self, v: &CalendarPeriod) -> bool {
        self.try_remove(*v).expect("period removal overflow")
    }
    fn replace(&mut self, a: &CalendarPeriod, b: &CalendarPeriod) -> bool {
        self.try_replace(*a, *b)
            .expect("period replacement overflow")
    }
    fn finish(&self) -> CalendarPeriod {
        self.sum
    }
}
