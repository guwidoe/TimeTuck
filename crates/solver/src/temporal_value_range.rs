//! Local Gregorian calendar values and anchored, indexed temporal progressions.
use crate::ValueRange;
use std::{error::Error, fmt};

/// Comparator accepted by the explicit immutable-sort rejection API.
pub type TemporalRangeComparator<'a, T> = dyn Fn(&T, &T) -> std::cmp::Ordering + 'a;

const YEAR_MIN: i32 = -999_999_999;
const YEAR_MAX: i32 = 999_999_999;
const SECOND: i128 = 1_000_000_000;
const DAY: i128 = 86_400 * SECOND;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalRangeError {
    InvalidDate,
    InvalidTime,
    InvalidYear,
    NonPositiveStep,
    DescendingBounds,
    UnsupportedUnit,
    UnalignedEnd,
    IndexOutOfBounds { index: i128, size: u128 },
    UnsupportedSorting,
}
impl fmt::Display for TemporalRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::IndexOutOfBounds { index, size } => {
                return write!(f, "temporal index ({index}) is outside range size ({size})");
            }
            Self::UnsupportedSorting => "temporal progressions do not support in-place sorting",
            Self::InvalidDate => "invalid Gregorian date",
            Self::InvalidTime => "invalid local time",
            Self::InvalidYear => "year is outside -999999999..=999999999",
            Self::NonPositiveStep => "temporal step must be positive",
            Self::DescendingBounds => "range start must not exceed its end",
            Self::UnsupportedUnit => "unit is unsupported by this temporal value",
            Self::UnalignedEnd => "end must be exactly reachable from start by whole steps",
        })
    }
}
impl Error for TemporalRangeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum TemporalUnit {
    Nanoseconds,
    Microseconds,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
    HalfDays,
    Days,
    Weeks,
    Months,
    Years,
    Decades,
    Centuries,
    Millennia,
}
impl TemporalUnit {
    fn months(self) -> Option<i128> {
        Some(match self {
            Self::Months => 1,
            Self::Years => 12,
            Self::Decades => 120,
            Self::Centuries => 1200,
            Self::Millennia => 12000,
            _ => return None,
        })
    }
    fn nanos(self) -> Option<i128> {
        Some(match self {
            Self::Nanoseconds => 1,
            Self::Microseconds => 1000,
            Self::Milliseconds => 1_000_000,
            Self::Seconds => SECOND,
            Self::Minutes => 60 * SECOND,
            Self::Hours => 3600 * SECOND,
            Self::HalfDays => DAY / 2,
            Self::Days => DAY,
            Self::Weeks => DAY * 7,
            _ => return None,
        })
    }
}

/// Proleptic Gregorian date with astronomical year numbering (year zero exists).
/// Fields are validated; the supported year interval is ±999,999,999.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}
fn month_days(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => 28 + u8::from(year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)),
        _ => 31,
    }
}
// Count whole years and leap days preceding January 1, relative to year zero.
// Euclidean division keeps the leap cycle continuous through negative years.
fn year_start(year: i64) -> i64 {
    365 * year + (year + 3).div_euclid(4) - (year + 99).div_euclid(100)
        + (year + 399).div_euclid(400)
}
impl CivilDate {
    pub const MIN: Self = Self {
        year: YEAR_MIN,
        month: 1,
        day: 1,
    };
    pub const MAX: Self = Self {
        year: YEAR_MAX,
        month: 12,
        day: 31,
    };
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, TemporalRangeError> {
        if !(YEAR_MIN..=YEAR_MAX).contains(&year)
            || !(1..=12).contains(&month)
            || day == 0
            || day > month_days(year, month)
        {
            return Err(TemporalRangeError::InvalidDate);
        }
        Ok(Self { year, month, day })
    }
    pub fn year(self) -> i32 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }
    /// Days since 0000-01-01; useful for exact civil-date differences.
    pub fn day_number(self) -> i64 {
        year_start(i64::from(self.year))
            + (1..self.month)
                .map(|m| i64::from(month_days(self.year, m)))
                .sum::<i64>()
            + i64::from(self.day)
            - 1
    }
    pub fn from_day_number(number: i64) -> Option<Self> {
        if number < year_start(i64::from(YEAR_MIN)) || number >= year_start(i64::from(YEAR_MAX) + 1)
        {
            return None;
        }
        // Invert cumulative year lengths by binary search, independent of range length.
        let (mut low, mut high) = (i64::from(YEAR_MIN), i64::from(YEAR_MAX) + 1);
        while high - low > 1 {
            let mid = low + (high - low) / 2;
            if year_start(mid) <= number {
                low = mid;
            } else {
                high = mid;
            }
        }
        let year = low as i32;
        let mut remaining = number - year_start(low);
        let mut month = 1;
        while remaining >= i64::from(month_days(year, month)) {
            remaining -= i64::from(month_days(year, month));
            month += 1;
        }
        Some(Self {
            year,
            month,
            day: remaining as u8 + 1,
        })
    }
    fn month_number(self) -> i128 {
        i128::from(self.year) * 12 + i128::from(self.month) - 1
    }
    fn add_months(self, months: i128) -> Option<Self> {
        let number = self.month_number().checked_add(months)?;
        let year = i32::try_from(number.div_euclid(12)).ok()?;
        let month = number.rem_euclid(12) as u8 + 1;
        Self::new(year, month, self.day.min(month_days(year, month))).ok()
    }
}

/// Time of day at nanosecond precision. No leap seconds or midnight wrapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CivilTime {
    nanos: i128,
}
impl CivilTime {
    pub const MIN: Self = Self { nanos: 0 };
    pub const MAX: Self = Self { nanos: DAY - 1 };
    pub fn new(
        hour: u8,
        minute: u8,
        second: u8,
        nanosecond: u32,
    ) -> Result<Self, TemporalRangeError> {
        if hour >= 24 || minute >= 60 || second >= 60 || nanosecond >= 1_000_000_000 {
            return Err(TemporalRangeError::InvalidTime);
        }
        Ok(Self {
            nanos: (i128::from(hour) * 3600 + i128::from(minute) * 60 + i128::from(second))
                * SECOND
                + i128::from(nanosecond),
        })
    }
    pub fn hour(self) -> u8 {
        (self.nanos / (3600 * SECOND)) as u8
    }
    pub fn minute(self) -> u8 {
        (self.nanos / (60 * SECOND) % 60) as u8
    }
    pub fn second(self) -> u8 {
        (self.nanos / SECOND % 60) as u8
    }
    pub fn nanosecond(self) -> u32 {
        (self.nanos % SECOND) as u32
    }
}

/// A local date and time, without a timezone, offset, or daylight-saving rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CivilDateTime {
    date: CivilDate,
    time: CivilTime,
}
impl CivilDateTime {
    pub const MIN: Self = Self {
        date: CivilDate::MIN,
        time: CivilTime::MIN,
    };
    pub const MAX: Self = Self {
        date: CivilDate::MAX,
        time: CivilTime::MAX,
    };
    pub fn new(date: CivilDate, time: CivilTime) -> Self {
        Self { date, time }
    }
    pub fn date(self) -> CivilDate {
        self.date
    }
    pub fn time(self) -> CivilTime {
        self.time
    }
    fn ticks(self) -> i128 {
        i128::from(self.date.day_number()) * DAY + self.time.nanos
    }
    fn from_ticks(ticks: i128) -> Option<Self> {
        Some(Self {
            date: CivilDate::from_day_number(i64::try_from(ticks.div_euclid(DAY)).ok()?)?,
            time: CivilTime {
                nanos: ticks.rem_euclid(DAY),
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CalendarYear(i32);
impl CalendarYear {
    pub const MIN: Self = Self(YEAR_MIN);
    pub const MAX: Self = Self(YEAR_MAX);
    pub fn new(year: i32) -> Result<Self, TemporalRangeError> {
        if (YEAR_MIN..=YEAR_MAX).contains(&year) {
            Ok(Self(year))
        } else {
            Err(TemporalRangeError::InvalidYear)
        }
    }
    pub fn value(self) -> i32 {
        self.0
    }
}

/// Local civil or application-defined temporal values. Implementations must keep
/// offset/distance/order consistent: distance is in whole units and offset from
/// the original anchor reaches the endpoint. Supported offsets must preserve unit
/// support and produce stable values. Range construction validates both endpoints;
/// indexed access never enumerates or materializes the progression.
pub trait TemporalValue: Copy + Ord {
    fn supports_unit(self, unit: TemporalUnit) -> bool {
        self.distance(self, unit).is_some()
    }
    fn offset(self, amount: i128, unit: TemporalUnit) -> Option<Self>;
    fn distance(self, end: Self, unit: TemporalUnit) -> Option<i128>;
}
impl TemporalValue for CivilDate {
    fn offset(self, amount: i128, unit: TemporalUnit) -> Option<Self> {
        if let Some(months) = unit.months() {
            return self.add_months(amount.checked_mul(months)?);
        }
        let days = match unit {
            TemporalUnit::Days => 1,
            TemporalUnit::Weeks => 7,
            _ => return None,
        };
        Self::from_day_number(
            i64::try_from(i128::from(self.day_number()).checked_add(amount.checked_mul(days)?)?)
                .ok()?,
        )
    }
    fn distance(self, end: Self, unit: TemporalUnit) -> Option<i128> {
        if let Some(months) = unit.months() {
            return Some((end.month_number() - self.month_number()) / months);
        }
        let days = match unit {
            TemporalUnit::Days => 1,
            TemporalUnit::Weeks => 7,
            _ => return None,
        };
        Some(i128::from(end.day_number() - self.day_number()) / days)
    }
}
impl TemporalValue for CivilTime {
    fn offset(self, amount: i128, unit: TemporalUnit) -> Option<Self> {
        let scale = unit.nanos().filter(|&n| n < DAY)?;
        let nanos = self.nanos.checked_add(amount.checked_mul(scale)?)?;
        (0..DAY).contains(&nanos).then_some(Self { nanos })
    }
    fn distance(self, end: Self, unit: TemporalUnit) -> Option<i128> {
        Some((end.nanos - self.nanos) / unit.nanos().filter(|&n| n < DAY)?)
    }
}
impl TemporalValue for CivilDateTime {
    fn offset(self, amount: i128, unit: TemporalUnit) -> Option<Self> {
        if unit.months().is_some() {
            return Some(Self::new(self.date.offset(amount, unit)?, self.time));
        }
        Self::from_ticks(
            self.ticks()
                .checked_add(amount.checked_mul(unit.nanos()?)?)?,
        )
    }
    fn distance(self, end: Self, unit: TemporalUnit) -> Option<i128> {
        if unit.months().is_some() {
            return self.date.distance(end.date, unit);
        }
        Some((end.ticks() - self.ticks()) / unit.nanos()?)
    }
}
impl TemporalValue for CalendarYear {
    fn offset(self, amount: i128, unit: TemporalUnit) -> Option<Self> {
        let years = unit.months().filter(|&m| m >= 12)? / 12;
        Self::new(i32::try_from(i128::from(self.0).checked_add(amount.checked_mul(years)?)?).ok()?)
            .ok()
    }
    fn distance(self, end: Self, unit: TemporalUnit) -> Option<i128> {
        let years = unit.months().filter(|&m| m >= 12)? / 12;
        Some((i128::from(end.0) - i128::from(self.0)) / years)
    }
}

/// Half-open temporal progression whose end must be exactly reachable by whole
/// positive steps. Each value is calculated from the original start: a monthly
/// January 31 range visits February's last day, then March 31 (without drift).
/// Calendar increments clamp the day to the destination month's last day.
/// Construction, membership and indexing never enumerate the range.
///
/// ```
/// use timetuck::*;
/// let start = CivilDate::new(2024, 1, 31)?;
/// let end = CivilDate::new(2024, 4, 30)?;
/// let range = TemporalValueRange::new(start, end, 1, TemporalUnit::Months)?;
/// assert_eq!(range.len(), 3);
/// assert_eq!(range.get(1), Some(CivilDate::new(2024, 2, 29)?));
/// assert_eq!(range.get(2), Some(CivilDate::new(2024, 3, 31)?));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemporalValueRange<T> {
    start: T,
    end: T,
    step: i128,
    unit: TemporalUnit,
    count: u128,
}
impl<T: TemporalValue> TemporalValueRange<T> {
    pub fn new(
        start: T,
        end: T,
        step: i64,
        unit: TemporalUnit,
    ) -> Result<Self, TemporalRangeError> {
        if step <= 0 {
            return Err(TemporalRangeError::NonPositiveStep);
        }
        if start > end {
            return Err(TemporalRangeError::DescendingBounds);
        }
        if !start.supports_unit(unit) || !end.supports_unit(unit) {
            return Err(TemporalRangeError::UnsupportedUnit);
        }
        let distance = start
            .distance(end, unit)
            .ok_or(TemporalRangeError::UnsupportedUnit)?;
        let step = i128::from(step);
        if distance < 0 || distance % step != 0 || start.offset(distance, unit) != Some(end) {
            return Err(TemporalRangeError::UnalignedEnd);
        }
        Ok(Self {
            start,
            end,
            step,
            unit,
            count: (distance / step) as u128,
        })
    }
    /// Checked signed indexing for inputs that may contain negative indices.
    pub fn get_signed(&self, index: i128) -> Result<T, TemporalRangeError> {
        u128::try_from(index).ok().and_then(|i| self.get(i)).ok_or(
            TemporalRangeError::IndexOutOfBounds {
                index,
                size: self.count,
            },
        )
    }
    /// Immutable anchored progressions cannot be sorted in place. Comparator
    /// views can be built externally; this never invokes the supplied comparator.
    pub fn sort(
        &mut self,
        _compare: Option<&TemporalRangeComparator<'_, T>>,
    ) -> Result<(), TemporalRangeError> {
        Err(TemporalRangeError::UnsupportedSorting)
    }
}
impl<T: TemporalValue> ValueRange for TemporalValueRange<T> {
    type Value = T;
    fn len(&self) -> u128 {
        self.count
    }
    fn get(&self, index: u128) -> Option<T> {
        if index >= self.count {
            return None;
        }
        self.start.offset(
            i128::try_from(index).ok()?.checked_mul(self.step)?,
            self.unit,
        )
    }
    fn contains(&self, value: &T) -> bool {
        if *value < self.start || *value >= self.end {
            return false;
        }
        let Some(distance) = self.start.distance(*value, self.unit) else {
            return false;
        };
        distance % self.step == 0 && self.start.offset(distance, self.unit) == Some(*value)
    }
}
