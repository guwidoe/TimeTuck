//! Validated coordinates and explicit canonical location identity.
use crate::RoutingError;
use std::{collections::HashMap, sync::Arc};
/// Finite latitude/longitude in degrees. Equality is exact after normalizing
/// signed zero; there is no approximate rounding or implicit coordinate wrapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct GeoLocation {
    latitude: u64,
    longitude: u64,
}
impl GeoLocation {
    pub fn new(latitude: f64, longitude: f64) -> Result<Self, RoutingError> {
        if !latitude.is_finite() || !(-90.0..=90.0).contains(&latitude) {
            return Err(RoutingError::InvalidLatitude(latitude));
        }
        if !longitude.is_finite() || !(-180.0..=180.0).contains(&longitude) {
            return Err(RoutingError::InvalidLongitude(longitude));
        }
        Ok(Self {
            latitude: if latitude == 0.0 { 0.0 } else { latitude }.to_bits(),
            longitude: if longitude == 0.0 { 0.0 } else { longitude }.to_bits(),
        })
    }
    pub fn latitude(self) -> f64 {
        f64::from_bits(self.latitude)
    }
    pub fn longitude(self) -> f64 {
        f64::from_bits(self.longitude)
    }
}
#[derive(Default)]
pub struct LocationInterner {
    locations: HashMap<GeoLocation, Arc<GeoLocation>>,
}
impl LocationInterner {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn intern(&mut self, location: GeoLocation) -> Arc<GeoLocation> {
        self.locations
            .entry(location)
            .or_insert_with(|| Arc::new(location))
            .clone()
    }
}
/// Accumulates unique coordinates in encounter order while detecting distinct
/// allocations with equal coordinates. Reusing the same canonical Arc is not
/// reported as a conflicting duplicate and does not append another location.
#[derive(Default)]
pub struct UniqueLocations {
    indexes: HashMap<GeoLocation, usize>,
    locations: Vec<Arc<GeoLocation>>,
}
impl UniqueLocations {
    pub fn new() -> Self {
        Self::default()
    }
    /// True means an equal coordinate was supplied through a different Arc.
    pub fn add(&mut self, location: Arc<GeoLocation>) -> bool {
        if let Some(&i) = self.indexes.get(location.as_ref()) {
            return !Arc::ptr_eq(&self.locations[i], &location);
        }
        self.indexes.insert(*location, self.locations.len());
        self.locations.push(location);
        false
    }
    pub fn locations(&self) -> &[Arc<GeoLocation>] {
        &self.locations
    }
}
