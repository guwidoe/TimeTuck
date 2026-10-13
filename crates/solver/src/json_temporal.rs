//! Typed ISO temporal values; explicit fixed offsets, no timezone database.
use crate::OffsetDeparture;
use std::{error::Error, fmt, str::FromStr, time::Duration};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JsonTemporalError(pub String);
impl fmt::Display for JsonTemporalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for JsonTemporalError {}
fn invalid(s: &str) -> JsonTemporalError {
    JsonTemporalError(format!("invalid ISO temporal value: {s}"))
}
fn leap(y: u16) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}
fn month_days(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}
fn digits(s: &str) -> Result<u32, JsonTemporalError> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(s));
    }
    s.parse().map_err(|_| invalid(s))
}
/// Proleptic Gregorian year0000..9999, nanosecond precision and explicit offset
/// seconds in ±18hours. Parsing requires uppercase T and Z, optional seconds and
/// 1..9 fractional digits, and ±HH:MM[:SS]. Calendar/offset construction validates
/// independently of parsing. No named zones, leap seconds or expanded years.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct JsonOffsetDateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanos: u32,
    offset: i32,
}
impl JsonOffsetDateTime {
    pub fn new(
        date: (u16, u8, u8),
        time: (u8, u8, u8, u32),
        offset: i32,
    ) -> Result<Self, JsonTemporalError> {
        let (year, month, day) = date;
        let (hour, minute, second, nanos) = time;
        if year > 9999
            || day == 0
            || day > month_days(year, month)
            || hour > 23
            || minute > 59
            || second > 59
            || nanos >= 1_000_000_000
        {
            return Err(invalid("date/time components"));
        }
        OffsetDeparture::new(0, offset).map_err(|e| JsonTemporalError(e.to_string()))?;
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            nanos,
            offset,
        })
    }
    pub fn date(self) -> (u16, u8, u8) {
        (self.year, self.month, self.day)
    }
    pub fn time(self) -> (u8, u8, u8, u32) {
        (self.hour, self.minute, self.second, self.nanos)
    }
    pub fn offset_seconds(self) -> i32 {
        self.offset
    }
    /// Reuse native departure/traffic arithmetic at whole-second precision.
    /// Fractional nanoseconds remain available in this typed value's `time()`.
    pub fn departure(self) -> OffsetDeparture {
        fn days(y: i64) -> i64 {
            365 * y + (y + 3) / 4 - (y + 99) / 100 + (y + 399) / 400
        }
        let mut day = days(i64::from(self.year)) - days(1970) + i64::from(self.day) - 1;
        for month in 1..self.month {
            day += i64::from(month_days(self.year, month));
        }
        let seconds = day * 86400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
            - i64::from(self.offset);
        OffsetDeparture::new(seconds, self.offset).expect("validated fixed offset")
    }
}
impl FromStr for JsonOffsetDateTime {
    type Err = JsonTemporalError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii()
            || s.len() < 17
            || s.as_bytes().get(4) != Some(&b'-')
            || s.as_bytes().get(7) != Some(&b'-')
            || s.as_bytes().get(10) != Some(&b'T')
            || s.as_bytes().get(13) != Some(&b':')
        {
            return Err(invalid(s));
        }
        let year = digits(&s[..4])? as u16;
        let month = digits(&s[5..7])? as u8;
        let day = digits(&s[8..10])? as u8;
        let hour = digits(&s[11..13])? as u8;
        let minute = digits(&s[14..16])? as u8;
        let tail = &s[16..];
        let zone = tail.find(['Z', '+', '-']).ok_or_else(|| invalid(s))?;
        let clock = &tail[..zone];
        let offset = &tail[zone..];
        let (second, nanos) = if clock.is_empty() {
            (0, 0)
        } else {
            let seconds = clock.strip_prefix(':').ok_or_else(|| invalid(s))?;
            let (whole, fraction) = seconds
                .split_once('.')
                .map_or((seconds, None), |(a, b)| (a, Some(b)));
            if whole.len() != 2 {
                return Err(invalid(s));
            }
            let second = digits(whole)? as u8;
            let nanos = match fraction {
                None => 0,
                Some(f) if !f.is_empty() && f.len() <= 9 => {
                    digits(f)? * 10u32.pow(9 - f.len() as u32)
                }
                _ => return Err(invalid(s)),
            };
            (second, nanos)
        };
        let offset = if offset == "Z" {
            0
        } else {
            if !matches!(offset.as_bytes()[0], b'+' | b'-') {
                return Err(invalid(s));
            }
            let fields: Vec<_> = offset[1..].split(':').collect();
            if !(2..=3).contains(&fields.len()) || fields.iter().any(|f| f.len() != 2) {
                return Err(invalid(s));
            }
            let h = digits(fields[0])?;
            let m = digits(fields[1])?;
            let sec = if fields.len() == 3 {
                digits(fields[2])?
            } else {
                0
            };
            if h > 18 || m > 59 || sec > 59 {
                return Err(invalid(s));
            }
            let sign = if offset.starts_with('-') { -1 } else { 1 };
            sign * (h * 3600 + m * 60 + sec) as i32
        };
        Self::new((year, month, day), (hour, minute, second, nanos), offset).map_err(|_| invalid(s))
    }
}
impl fmt::Display for JsonOffsetDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )?;
        if self.nanos != 0 {
            let frac = format!("{:09}", self.nanos);
            write!(f, ".{}", frac.trim_end_matches('0'))?;
        }
        if self.offset == 0 {
            return f.write_str("Z");
        }
        let value = self.offset.unsigned_abs();
        write!(
            f,
            "{}{:02}:{:02}",
            if self.offset < 0 { '-' } else { '+' },
            value / 3600,
            (value % 3600) / 60
        )?;
        if !value.is_multiple_of(60) {
            write!(f, ":{:02}", value % 60)?;
        }
        Ok(())
    }
}
/// Signed nanosecond duration. ISO days/hours/minutes/seconds (ordered) are
/// supported; calendar months/years/weeks are not durations. Fractions belong
/// only to seconds, up to9digits. Canonical negative output uses a leading sign.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct JsonDuration {
    nanos: i128,
}
impl JsonDuration {
    pub fn from_nanoseconds(nanos: i128) -> Result<Self, JsonTemporalError> {
        if nanos == i128::MIN {
            return Err(invalid("duration overflow"));
        }
        Ok(Self { nanos })
    }
    pub fn from_std(duration: Duration) -> Self {
        Self {
            nanos: i128::from(duration.as_secs()) * 1_000_000_000
                + i128::from(duration.subsec_nanos()),
        }
    }
    pub fn nanoseconds(self) -> i128 {
        self.nanos
    }
    pub fn to_std(self) -> Result<Duration, JsonTemporalError> {
        if self.nanos < 0 {
            return Err(invalid("negative std duration"));
        }
        let sec = u64::try_from(self.nanos / 1_000_000_000)
            .map_err(|_| invalid("std duration overflow"))?;
        Ok(Duration::new(sec, (self.nanos % 1_000_000_000) as u32))
    }
}
impl FromStr for JsonDuration {
    type Err = JsonTemporalError;
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        if !source.is_ascii() {
            return Err(invalid(source));
        }
        let (negative, s) = if let Some(s) = source.strip_prefix('-') {
            (true, s)
        } else {
            (false, source.strip_prefix('+').unwrap_or(source))
        };
        let mut rest = s.strip_prefix('P').ok_or_else(|| invalid(source))?;
        let mut in_time = false;
        let mut rank = 0;
        let mut components = 0;
        let mut time_components = 0;
        let mut total = 0i128;
        while !rest.is_empty() {
            if let Some(r) = rest.strip_prefix('T') {
                if in_time {
                    return Err(invalid(source));
                }
                in_time = true;
                rest = r;
                continue;
            }
            let mut end = 0;
            for (i, b) in rest.bytes().enumerate() {
                if b.is_ascii_digit() || b == b'.' || ((b == b'+' || b == b'-') && i == 0) {
                    end = i + 1;
                } else {
                    break;
                }
            }
            if end == 0 || end >= rest.len() {
                return Err(invalid(source));
            }
            let num = &rest[..end];
            let unit = rest.as_bytes()[end];
            rest = &rest[end + 1..];
            let (current, factor) = match (in_time, unit) {
                (false, b'D') => (1, 86400i128),
                (true, b'H') => (2, 3600),
                (true, b'M') => (3, 60),
                (true, b'S') => (4, 1),
                _ => return Err(invalid(source)),
            };
            if current <= rank {
                return Err(invalid(source));
            }
            rank = current;
            components += 1;
            if in_time {
                time_components += 1;
            }
            let (part_negative, n) = if let Some(n) = num.strip_prefix('-') {
                (true, n)
            } else {
                (false, num.strip_prefix('+').unwrap_or(num))
            };
            let (whole, fraction) = n.split_once('.').map_or((n, None), |(a, b)| (a, Some(b)));
            if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid(source));
            }
            let integer = whole.parse::<i128>().map_err(|_| invalid(source))?;
            let frac = match fraction {
                None => 0,
                Some(f) if current == 4 && !f.is_empty() && f.len() <= 9 => {
                    i128::from(digits(f)?) * 10i128.pow(9 - f.len() as u32)
                }
                _ => return Err(invalid(source)),
            };
            let mut part = integer
                .checked_mul(factor)
                .and_then(|v| v.checked_mul(1_000_000_000))
                .and_then(|v| v.checked_add(frac))
                .ok_or_else(|| invalid(source))?;
            if part_negative {
                part = -part;
            }
            total = total.checked_add(part).ok_or_else(|| invalid(source))?;
        }
        if components == 0 || (in_time && time_components == 0) {
            return Err(invalid(source));
        }
        if negative {
            total = total.checked_neg().ok_or_else(|| invalid(source))?;
        }
        Self::from_nanoseconds(total)
    }
}
impl fmt::Display for JsonDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.nanos.unsigned_abs();
        let seconds = n / 1_000_000_000;
        let nanos = n % 1_000_000_000;
        let hours = seconds / 3600;
        let minutes = (seconds % 3600) / 60;
        let seconds = seconds % 60;
        if self.nanos < 0 {
            f.write_str("-")?;
        }
        f.write_str("PT")?;
        if hours != 0 {
            write!(f, "{hours}H")?;
        }
        if minutes != 0 {
            write!(f, "{minutes}M")?;
        }
        if seconds != 0 || nanos != 0 || (hours == 0 && minutes == 0) {
            write!(f, "{seconds}")?;
            if nanos != 0 {
                let fraction = format!("{nanos:09}");
                write!(f, ".{}", fraction.trim_end_matches('0'))?;
            }
            f.write_str("S")?;
        }
        Ok(())
    }
}
