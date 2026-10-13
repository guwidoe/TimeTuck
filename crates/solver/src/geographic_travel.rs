//! Native travel values and spherical matrix round trips; no road service is used.
use crate::{DirectedDistanceMatrix, GeoLocation, RoutingError, great_circle_distance};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    sync::{Arc, OnceLock},
};
#[derive(Debug, Eq, PartialEq)]
pub struct NativeTravelTime {
    seconds: Option<i64>,
}
#[derive(Debug)]
pub enum GeographicTravelError {
    MissingAdjuster,
    NegativeSeconds(i64),
    Matrix(RoutingError),
    Json(String),
}
impl fmt::Display for GeographicTravelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geographic travel: {self:?}")
    }
}
impl Error for GeographicTravelError {}
impl From<RoutingError> for GeographicTravelError {
    fn from(e: RoutingError) -> Self {
        Self::Matrix(e)
    }
}
impl NativeTravelTime {
    pub fn of(seconds: i64) -> Result<Arc<Self>, GeographicTravelError> {
        if seconds < 0 {
            return Err(GeographicTravelError::NegativeSeconds(seconds));
        }
        Ok(Arc::new(Self {
            seconds: Some(seconds),
        }))
    }
    pub fn unreachable() -> Arc<Self> {
        static VALUE: OnceLock<Arc<NativeTravelTime>> = OnceLock::new();
        VALUE
            .get_or_init(|| Arc::new(Self { seconds: None }))
            .clone()
    }
    pub fn seconds(&self) -> Option<i64> {
        self.seconds
    }
    pub fn adjust<F: FnOnce(i64) -> i64>(
        self: &Arc<Self>,
        adjuster: Option<F>,
    ) -> Result<Arc<Self>, GeographicTravelError> {
        let adjuster = adjuster.ok_or(GeographicTravelError::MissingAdjuster)?;
        let Some(seconds) = self.seconds else {
            return Ok(self.clone());
        };
        let next = adjuster(seconds);
        if next == seconds {
            Ok(self.clone())
        } else {
            Self::of(next)
        }
    }
}
/// Public Community quickstart preset:50km/hour, nearest integral second.
/// This is an explicit native preset, not a claim about an undocumented service default.
pub fn spherical_driving_seconds(meters: i64) -> Result<i64, GeographicTravelError> {
    if meters < 0 {
        return Err(GeographicTravelError::NegativeSeconds(meters));
    }
    i64::try_from((i128::from(meters) * 9 + 62) / 125)
        .map_err(|_| GeographicTravelError::Json("time overflow".into()))
}
pub fn spherical_travel_time(from: GeoLocation, to: GeoLocation) -> Arc<NativeTravelTime> {
    NativeTravelTime::of(
        spherical_driving_seconds(great_circle_distance(from, to)).expect("bounded Earth distance"),
    )
    .expect("nonnegative")
}
/// All validated Earth coordinates are in this spherical model's coverage.
pub fn spherical_locations_out_of_map(locations: &[Arc<GeoLocation>]) -> Vec<usize> {
    locations
        .iter()
        .enumerate()
        .filter_map(|(i, p)| (!p.latitude().is_finite() || !p.longitude().is_finite()).then_some(i))
        .collect()
}
/// Spherical routes have no intermediate road vertices; preserve supplied allocations/order.
pub fn spherical_waypoints(locations: &[Arc<GeoLocation>]) -> Vec<Arc<GeoLocation>> {
    locations.to_vec()
}
pub struct GeographicTravelMatrices {
    locations: Vec<GeoLocation>,
    distance: DirectedDistanceMatrix<GeoLocation>,
    time: DirectedDistanceMatrix<GeoLocation>,
}
impl GeographicTravelMatrices {
    /// Explicit entry ceiling is checked before dense allocation or geography calls.
    pub fn calculate(
        sources: &[GeoLocation],
        destinations: &[GeoLocation],
        max_entries: usize,
    ) -> Result<Self, GeographicTravelError> {
        let mut seen = HashSet::new();
        let locations = sources
            .iter()
            .chain(destinations)
            .copied()
            .filter(|p| seen.insert(*p))
            .collect::<Vec<_>>();
        if locations
            .len()
            .checked_mul(locations.len())
            .is_none_or(|n| n > max_entries)
        {
            return Err(GeographicTravelError::Json(
                "matrix entry limit exceeded".into(),
            ));
        }
        let mut result = Self::empty(locations)?;
        for &a in sources {
            for &b in destinations {
                let meters = great_circle_distance(a, b);
                result.distance.put(&a, &b, meters)?;
                result
                    .time
                    .put(&a, &b, spherical_driving_seconds(meters)?)?;
            }
        }
        Ok(result)
    }
    fn empty(locations: Vec<GeoLocation>) -> Result<Self, GeographicTravelError> {
        let mut distance = DirectedDistanceMatrix::with_capacity(locations.len() as i64)?;
        let mut time = DirectedDistanceMatrix::with_capacity(locations.len() as i64)?;
        for &p in &locations {
            distance.register(p)?;
            time.register(p)?;
        }
        Ok(Self {
            locations,
            distance,
            time,
        })
    }
    pub fn distance(&self, from: GeoLocation, to: GeoLocation) -> Option<i64> {
        self.distance.get(&from, &to)
    }
    pub fn travel_time(&self, from: GeoLocation, to: GeoLocation) -> Option<Arc<NativeTravelTime>> {
        self.time.get(&from, &to).map(|t| {
            if t == -1 {
                NativeTravelTime::unreachable()
            } else {
                NativeTravelTime::of(t).expect("validated time")
            }
        })
    }
    pub fn set_unreachable(
        &mut self,
        from: GeoLocation,
        to: GeoLocation,
    ) -> Result<(), GeographicTravelError> {
        if !self.locations.contains(&from) || !self.locations.contains(&to) {
            return Err(GeographicTravelError::Json(
                "unknown matrix location".into(),
            ));
        }
        self.distance.put(&from, &to, -1)?;
        self.time.put(&from, &to, -1)?;
        Ok(())
    }
    pub fn to_json(&self) -> Vec<u8> {
        let table = |m: &DirectedDistanceMatrix<GeoLocation>| {
            self.locations
                .iter()
                .map(|a| {
                    self.locations
                        .iter()
                        .map(|b| m.get(a, b))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };
        serde_json::to_vec(&serde_json::json!({"version":1,"locations":self.locations.iter().map(|p|[p.latitude(),p.longitude()]).collect::<Vec<_>>(),"distance_meters":table(&self.distance),"travel_seconds":table(&self.time)})).expect("finite validated native matrix")
    }
    /// Native versioned format; Java wire compatibility is not implied. Duplicate
    /// coordinates, invalid dimensions/costs and allocation limits reject before publication.
    pub fn from_json(bytes: &[u8], max_entries: usize) -> Result<Self, GeographicTravelError> {
        let fail = |s: &str| GeographicTravelError::Json(s.into());
        let data: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|e| fail(&e.to_string()))?;
        if data["version"].as_u64() != Some(1) {
            return Err(fail("unsupported matrix version"));
        }
        let raw = data["locations"]
            .as_array()
            .ok_or_else(|| fail("locations must be an array"))?;
        if raw
            .len()
            .checked_mul(raw.len())
            .is_none_or(|n| n > max_entries)
        {
            return Err(fail("matrix entry limit exceeded"));
        }
        let mut seen = HashMap::new();
        let mut locations = Vec::new();
        for point in raw {
            let pair = point
                .as_array()
                .filter(|p| p.len() == 2)
                .ok_or_else(|| fail("coordinate pair required"))?;
            let p = GeoLocation::new(
                pair[0].as_f64().ok_or_else(|| fail("latitude required"))?,
                pair[1].as_f64().ok_or_else(|| fail("longitude required"))?,
            )?;
            if seen.insert(p, ()).is_some() {
                return Err(fail("duplicate location"));
            }
            locations.push(p);
        }
        let mut result = Self::empty(locations)?;
        for (name, matrix) in [
            ("distance_meters", &mut result.distance),
            ("travel_seconds", &mut result.time),
        ] {
            let rows = data[name]
                .as_array()
                .filter(|r| r.len() == result.locations.len())
                .ok_or_else(|| fail("matrix dimensions"))?;
            for (i, row) in rows.iter().enumerate() {
                let cols = row
                    .as_array()
                    .filter(|r| r.len() == result.locations.len())
                    .ok_or_else(|| fail("matrix dimensions"))?;
                for (j, v) in cols.iter().enumerate() {
                    if v.is_null() {
                        continue;
                    }
                    let n = v
                        .as_i64()
                        .filter(|n| *n >= -1)
                        .ok_or_else(|| fail("invalid matrix cost"))?;
                    matrix.put(&result.locations[i], &result.locations[j], n)?;
                }
            }
        }
        Ok(result)
    }
}
