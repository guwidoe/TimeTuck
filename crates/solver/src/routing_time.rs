//! Fixed-offset departure times and deterministic daily traffic buckets.
use std::{error::Error, fmt};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoutingTimeError {
    InvalidOffset(i32),
    ReversedInterval,
}
impl fmt::Display for RoutingTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOffset(v) => write!(f, "UTC offset {v} seconds is outside -18h..=18h"),
            Self::ReversedInterval => f.write_str("departure interval ends before it starts"),
        }
    }
}
impl Error for RoutingTimeError {}
/// An instant in Unix seconds and an explicit fixed UTC offset. No timezone
/// database or daylight-saving conversion is implied. Local arithmetic uses i128
/// so every i64 instant is representable, including with an offset at either end.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OffsetDeparture {
    unix_seconds: i64,
    offset_seconds: i32,
}
impl OffsetDeparture {
    pub fn new(unix_seconds: i64, offset_seconds: i32) -> Result<Self, RoutingTimeError> {
        if !(-64800..=64800).contains(&offset_seconds) {
            return Err(RoutingTimeError::InvalidOffset(offset_seconds));
        }
        Ok(Self {
            unix_seconds,
            offset_seconds,
        })
    }
    pub fn unix_seconds(self) -> i64 {
        self.unix_seconds
    }
    pub fn offset_seconds(self) -> i32 {
        self.offset_seconds
    }
    pub fn with_offset(self, offset_seconds: i32) -> Result<Self, RoutingTimeError> {
        Self::new(self.unix_seconds, offset_seconds)
    }
    fn local_seconds(self) -> i128 {
        i128::from(self.unix_seconds) + i128::from(self.offset_seconds)
    }
    pub fn local_hour(self) -> u8 {
        (self.local_seconds().rem_euclid(86400) / 3600) as u8
    }
}
/// Morning [06:00,12:00), afternoon [12:00,18:00), night otherwise.
/// Stable matrix-slot order is Morning, Afternoon, Night.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Daypart {
    Morning,
    Afternoon,
    Night,
}
impl Daypart {
    pub const ALL: [Self; 3] = [Self::Morning, Self::Afternoon, Self::Night];
    pub fn index(self) -> usize {
        match self {
            Self::Morning => 0,
            Self::Afternoon => 1,
            Self::Night => 2,
        }
    }
    fn for_hour(hour: u8) -> Self {
        match hour {
            6..12 => Self::Morning,
            12..18 => Self::Afternoon,
            _ => Self::Night,
        }
    }
    pub fn at(departure: OffsetDeparture) -> Self {
        Self::for_hour(departure.local_hour())
    }
    /// Unique buckets in stable slot order over [from,to). A zero-length interval
    /// includes from's bucket. Reversed instants are errors. The interval uses
    /// from's fixed offset throughout, even if to represents its instant with
    /// another offset. Work is bounded by one day, regardless of interval length.
    pub fn covering(
        from: OffsetDeparture,
        to: OffsetDeparture,
    ) -> Result<Vec<Self>, RoutingTimeError> {
        let duration = i128::from(to.unix_seconds) - i128::from(from.unix_seconds);
        if duration < 0 {
            return Err(RoutingTimeError::ReversedInterval);
        }
        if duration >= 86400 {
            return Ok(Self::ALL.to_vec());
        }
        let mut seen = [false; 3];
        seen[Self::at(from).index()] = true;
        let start = from.local_seconds();
        let end = start + duration;
        let mut boundary = (start.div_euclid(21600) + 1) * 21600;
        while boundary < end {
            let hour = (boundary.rem_euclid(86400) / 3600) as u8;
            seen[Self::for_hour(hour).index()] = true;
            boundary += 21600;
        }
        Ok(Self::ALL.into_iter().filter(|p| seen[p.index()]).collect())
    }
}
