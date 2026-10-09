//! Spherical straight-line estimates for routing without a road-network service.
use crate::{DirectedDistanceMatrix, GeoLocation, MAX_MATRIX_LOCATIONS, RoutingError};
use std::{collections::HashSet, error::Error, fmt};
/// A conventional spherical Earth approximation, in meters.
pub const ROUTING_EARTH_RADIUS_METERS: f64 = 6_371_000.0;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TravelEstimate {
    pub distance_meters: i64,
    pub travel_seconds: i64,
}
#[derive(Clone, Debug, PartialEq)]
pub enum GreatCircleError {
    ZeroSpeed,
    NegativeDistance(i64),
    TimeOverflow,
    Matrix(RoutingError),
}
impl fmt::Display for GreatCircleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSpeed => f.write_str("travel speed must be positive"),
            Self::NegativeDistance(v) => write!(f, "distance {v} meters is negative"),
            Self::TimeOverflow => f.write_str("travel time exceeds i64 seconds"),
            Self::Matrix(e) => e.fmt(f),
        }
    }
}
impl Error for GreatCircleError {}
impl From<RoutingError> for GreatCircleError {
    fn from(e: RoutingError) -> Self {
        Self::Matrix(e)
    }
}
struct Radians {
    latitude: f64,
    longitude: f64,
    cos_latitude: f64,
}
impl From<GeoLocation> for Radians {
    fn from(p: GeoLocation) -> Self {
        let latitude = p.latitude().to_radians();
        Self {
            latitude,
            longitude: p.longitude().to_radians(),
            cos_latitude: latitude.cos(),
        }
    }
}
fn distance(a: &Radians, b: &Radians) -> i64 {
    let latitude_term = ((a.latitude - b.latitude) * 0.5).sin();
    let longitude_term = ((a.longitude - b.longitude) * 0.5).sin();
    let h = (latitude_term * latitude_term
        + a.cos_latitude * b.cos_latitude * longitude_term * longitude_term)
        .clamp(0.0, 1.0);
    (2.0 * ROUTING_EARTH_RADIUS_METERS * h.sqrt().atan2((1.0 - h).sqrt())).round() as i64
}
/// Rounded great-circle meters, symmetric and nonnegative. A spherical
/// approximation, not road distance; coincident points cost zero.
pub fn great_circle_distance(from: GeoLocation, to: GeoLocation) -> i64 {
    distance(&from.into(), &to.into())
}
/// Explicit constant speed in millimeters/second; there is no implicit driving
/// speed or traffic factor. Seconds are rounded up from rounded integral meters.
#[derive(Clone, Copy, Debug)]
pub struct GreatCircleRouting {
    millimeters_per_second: u64,
}
pub struct GreatCircleMatrices {
    pub distance_meters: DirectedDistanceMatrix<GeoLocation>,
    pub travel_seconds: DirectedDistanceMatrix<GeoLocation>,
}
impl GreatCircleRouting {
    pub fn new(millimeters_per_second: u64) -> Result<Self, GreatCircleError> {
        if millimeters_per_second == 0 {
            return Err(GreatCircleError::ZeroSpeed);
        }
        Ok(Self {
            millimeters_per_second,
        })
    }
    pub fn meters_to_seconds(self, meters: i64) -> Result<i64, GreatCircleError> {
        if meters < 0 {
            return Err(GreatCircleError::NegativeDistance(meters));
        }
        let seconds = (meters as u128 * 1000).div_ceil(u128::from(self.millimeters_per_second));
        i64::try_from(seconds).map_err(|_| GreatCircleError::TimeOverflow)
    }
    pub fn estimate(self, from: GeoLocation, to: GeoLocation) -> TravelEstimate {
        let meters = great_circle_distance(from, to);
        // Maximum spherical distance is <21 million meters, so every positive
        // integer speed produces a time well within i64.
        TravelEstimate {
            distance_meters: meters,
            travel_seconds: self
                .meters_to_seconds(meters)
                .expect("bounded spherical distance"),
        }
    }
    /// Build complete symmetric matrices, deduplicating equal coordinates in
    /// encounter order. Radians/cosines are computed once per unique location;
    /// each unordered pair is measured once. Matrices have independent handles.
    pub fn matrices(
        self,
        locations: &[GeoLocation],
    ) -> Result<GreatCircleMatrices, GreatCircleError> {
        let mut seen = HashSet::new();
        let mut points = Vec::new();
        for &p in locations {
            if seen.insert(p) {
                if points.len() == MAX_MATRIX_LOCATIONS {
                    return Err(RoutingError::InvalidCapacity {
                        expected: MAX_MATRIX_LOCATIONS as i64 + 1,
                        maximum: MAX_MATRIX_LOCATIONS,
                    }
                    .into());
                }
                points.push(p);
            }
        }
        // Allocate each dense table at its final size, avoiding geometric spare
        // capacity and repeated initialization while registering locations.
        let mut meters = DirectedDistanceMatrix::with_capacity(points.len() as i64)?;
        let mut seconds = DirectedDistanceMatrix::with_capacity(points.len() as i64)?;
        let mut unique = Vec::with_capacity(points.len());
        for p in points {
            let m = meters.register(p)?;
            let s = seconds.register(p)?;
            unique.push((Radians::from(p), m, s));
        }
        for (i, (a, ma, sa)) in unique.iter().enumerate() {
            meters.put_indexed(*ma, *ma, 0)?;
            seconds.put_indexed(*sa, *sa, 0)?;
            for (b, mb, sb) in &unique[..i] {
                let d = distance(a, b);
                let t = self.meters_to_seconds(d)?;
                meters.put_indexed(*ma, *mb, d)?;
                meters.put_indexed(*mb, *ma, d)?;
                seconds.put_indexed(*sa, *sb, t)?;
                seconds.put_indexed(*sb, *sa, t)?;
            }
        }
        Ok(GreatCircleMatrices {
            distance_meters: meters,
            travel_seconds: seconds,
        })
    }
}
