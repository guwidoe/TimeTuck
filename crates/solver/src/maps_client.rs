//! Optional typed transport, matrix cache/enrichment and event-owned waypoint routes.
use crate::{Daypart, GeoLocation, OffsetDeparture};
use std::{
    any::Any,
    collections::{BTreeSet, HashMap, HashSet},
    error::Error,
    fmt,
    sync::Arc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapsLocationsModel {
    pub name: Option<String>,
    pub locations: Vec<Arc<GeoLocation>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapsRequestKind {
    Full,
    Update {
        hash: String,
        old_locations: Vec<GeoLocation>,
    },
    Timeframes,
    Waypoints,
}
#[derive(Clone, Debug)]
pub struct MapsRequest {
    pub kind: MapsRequestKind,
    pub model: MapsLocationsModel,
    pub options: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapsMatrixFrame {
    pub entries: HashMap<(GeoLocation, GeoLocation), (i64, i64)>,
}
impl MapsMatrixFrame {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
    pub fn put(&mut self, a: GeoLocation, b: GeoLocation, time: i64, distance: i64) {
        self.entries.insert((a, b), (time, distance));
    }
}
impl Default for MapsMatrixFrame {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Debug)]
pub struct MapsResponse {
    pub frames: Vec<Option<MapsMatrixFrame>>,
    pub not_in_map: Vec<usize>,
    pub resolved_map: Option<String>,
    pub hash: String,
    pub waypoints: Vec<GeoLocation>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapsTransportError {
    Gone,
    NotModified,
    Failure(String),
}
pub trait MapsTransport {
    fn execute(&mut self, request: &MapsRequest) -> Result<MapsResponse, MapsTransportError>;
}
impl<F: FnMut(&MapsRequest) -> Result<MapsResponse, MapsTransportError>> MapsTransport for F {
    fn execute(&mut self, request: &MapsRequest) -> Result<MapsResponse, MapsTransportError> {
        self(request)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapsClientError(pub String);
impl fmt::Display for MapsClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for MapsClientError {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnrichedMapsModel {
    pub model: MapsLocationsModel,
    pub not_in_map: Vec<Arc<GeoLocation>>,
    pub frames: Vec<Option<MapsMatrixFrame>>,
    pub resolved_map: Option<String>,
    pub hash: String,
}
impl EnrichedMapsModel {
    fn edge(
        &self,
        a: GeoLocation,
        b: GeoLocation,
        at: Option<OffsetDeparture>,
    ) -> Result<(i64, i64), MapsClientError> {
        if a == b {
            return Ok((0, 0));
        }
        if self.not_in_map.iter().any(|p| **p == a || **p == b) {
            return Ok((0, 0));
        }
        let frame = if self.frames.len() == 1 {
            self.frames[0].as_ref()
        } else if let Some(at) = at {
            self.frames
                .get(Daypart::at(at).index())
                .and_then(Option::as_ref)
        } else {
            self.frames.iter().find_map(Option::as_ref)
        }
        .ok_or_else(|| MapsClientError("No matrix fetched for timeframe".into()))?;
        frame
            .entries
            .get(&(a, b))
            .copied()
            .ok_or_else(|| MapsClientError("missing matrix edge".into()))
    }
    pub fn travel_time(
        &self,
        a: GeoLocation,
        b: GeoLocation,
        at: Option<OffsetDeparture>,
    ) -> Result<i64, MapsClientError> {
        Ok(self.edge(a, b, at)?.0)
    }
    pub fn distance(
        &self,
        a: GeoLocation,
        b: GeoLocation,
        at: Option<OffsetDeparture>,
    ) -> Result<i64, MapsClientError> {
        Ok(self.edge(a, b, at)?.1)
    }
}
#[derive(Clone, Debug)]
pub struct MapsWaypointRoute {
    pub id: String,
    pub locations: Vec<Arc<GeoLocation>>,
}
#[derive(Clone, Copy, Debug)]
pub enum MapsSolutionEvent {
    Initialized,
    Best,
    Final,
}
struct Routes {
    routes: Vec<MapsWaypointRoute>,
    cache: HashMap<String, Vec<Arc<GeoLocation>>>,
}
/// Requests and failures are real transport calls. Named matrix caches publish only
/// validated responses;410/Gone triggers a full fetch, NotModified retains cached
/// matrices, updates merge actual edge blocks and missing headers retain context.
/// No HTTP/Jackson wire or Java model-mutating ABI is claimed. Bounded dense entry
/// and route count limits are caller policy; failed waypoint calls remain retryable.
pub struct MapsClient<T> {
    transport: T,
    traffic: bool,
    options: String,
    max_entries: usize,
    cache: HashMap<(String, bool, String), Arc<EnrichedMapsModel>>,
    resolved_map: Option<String>,
    routes: HashMap<String, Routes>,
}
impl<T: MapsTransport> MapsClient<T> {
    pub fn new(
        transport: T,
        traffic: bool,
        options: impl Into<String>,
        max_entries: usize,
    ) -> Self {
        Self {
            transport,
            traffic,
            options: options.into(),
            max_entries,
            cache: HashMap::new(),
            resolved_map: None,
            routes: HashMap::new(),
        }
    }
    pub fn transport(&self) -> &T {
        &self.transport
    }
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }
    pub fn resolved_map(&self) -> Option<&str> {
        self.resolved_map.as_deref()
    }
    pub fn accepts(value: &dyn Any) -> bool {
        value.is::<MapsLocationsModel>()
    }
    fn request(
        &mut self,
        kind: MapsRequestKind,
        model: MapsLocationsModel,
    ) -> Result<MapsResponse, MapsTransportError> {
        self.transport.execute(&MapsRequest {
            kind,
            model,
            options: self.options.clone(),
        })
    }
    pub fn enrich(
        &mut self,
        model: &MapsLocationsModel,
    ) -> Result<Arc<EnrichedMapsModel>, MapsClientError> {
        if model
            .locations
            .len()
            .checked_mul(model.locations.len())
            .is_none_or(|n| n > self.max_entries)
        {
            return Err(MapsClientError("matrix entry limit exceeded".into()));
        }
        let key = model
            .name
            .as_ref()
            .map(|n| (n.clone(), self.traffic, self.options.clone()));
        let old = key.as_ref().and_then(|k| self.cache.get(k)).cloned();
        let kind = if self.traffic {
            MapsRequestKind::Timeframes
        } else if let Some(old) = &old {
            MapsRequestKind::Update {
                hash: old.hash.clone(),
                old_locations: old.model.locations.iter().map(|p| **p).collect(),
            }
        } else {
            MapsRequestKind::Full
        };
        let mut merge_previous = matches!(kind, MapsRequestKind::Update { .. });
        let reply = if model.locations.len() <= 1 && !self.traffic {
            Ok(MapsResponse {
                frames: vec![Some(MapsMatrixFrame::new())],
                not_in_map: vec![],
                resolved_map: None,
                hash: String::new(),
                waypoints: vec![],
            })
        } else {
            self.request(kind, model.clone())
        };
        let response = match reply {
            Err(MapsTransportError::NotModified) => {
                let old = old.ok_or_else(|| MapsClientError("NotModified without cache".into()))?;
                if old.model != *model {
                    return Err(MapsClientError("NotModified changed locations".into()));
                }
                self.resolved_map = old.resolved_map.clone();
                if old
                    .model
                    .locations
                    .iter()
                    .zip(&model.locations)
                    .all(|(a, b)| Arc::ptr_eq(a, b))
                {
                    return Ok(old);
                }
                let mut rebound = old.as_ref().clone();
                rebound.model = model.clone();
                let rebound = Arc::new(rebound);
                if let Some(key) = key {
                    self.cache.insert(key, rebound.clone());
                }
                return Ok(rebound);
            }
            Err(MapsTransportError::Gone) => {
                merge_previous = false;
                self.request(
                    if self.traffic {
                        MapsRequestKind::Timeframes
                    } else {
                        MapsRequestKind::Full
                    },
                    model.clone(),
                )
                .map_err(wrap)?
            }
            Err(e) => return Err(wrap(e)),
            Ok(r) => r,
        };
        if response.frames.is_empty() || !matches!(response.frames.len(), 1 | 3) {
            return Err(MapsClientError("invalid timeframe count".into()));
        }
        let mut frames = response.frames;
        let mut excluded = Vec::new();
        let mut indexes = BTreeSet::new();
        for i in response.not_in_map {
            if !indexes.insert(i) {
                return Err(MapsClientError("duplicate out-of-map index".into()));
            }
            excluded.push(
                model
                    .locations
                    .get(i)
                    .ok_or_else(|| MapsClientError("out-of-map index out of bounds".into()))?
                    .clone(),
            );
        }
        let all = model.locations.iter().map(|p| **p).collect::<HashSet<_>>();
        for frame in frames.iter().flatten() {
            if frame.entries.len() > self.max_entries
                || frame.entries.iter().any(|((a, b), (t, d))| {
                    !all.contains(a) || !all.contains(b) || *t < 0 || *d < 0
                })
            {
                return Err(MapsClientError("invalid matrix response".into()));
            }
        }
        if merge_previous && let Some(old) = &old {
            for (i, frame) in frames.iter_mut().enumerate() {
                if let Some(frame) = frame
                    && let Some(Some(previous)) = old.frames.get(i)
                {
                    for (edge, value) in &previous.entries {
                        if all.contains(&edge.0) && all.contains(&edge.1) {
                            frame.entries.entry(*edge).or_insert(*value);
                        }
                    }
                }
            }
        }
        let resolved = response
            .resolved_map
            .or_else(|| old.as_ref().and_then(|o| o.resolved_map.clone()));
        let enriched = Arc::new(EnrichedMapsModel {
            model: model.clone(),
            not_in_map: excluded,
            frames,
            resolved_map: resolved.clone(),
            hash: response.hash,
        });
        self.resolved_map = resolved;
        if let Some(key) = key {
            self.cache.insert(key, enriched.clone());
        }
        Ok(enriched)
    }
    pub fn publish_routes(
        &mut self,
        dataset: impl Into<String>,
        _: MapsSolutionEvent,
        routes: Vec<MapsWaypointRoute>,
    ) -> Result<(), MapsClientError> {
        if routes.len() > self.max_entries {
            return Err(MapsClientError("route limit exceeded".into()));
        }
        let mut ids = BTreeSet::new();
        if routes
            .iter()
            .any(|r| !ids.insert(r.id.clone()) || r.locations.len() > self.max_entries)
        {
            return Err(MapsClientError("invalid routes".into()));
        }
        self.routes.insert(
            dataset.into(),
            Routes {
                routes,
                cache: HashMap::new(),
            },
        );
        Ok(())
    }
    pub fn waypoints(
        &mut self,
        dataset: &str,
        filter: &BTreeSet<String>,
    ) -> Result<Vec<MapsWaypointRoute>, MapsClientError> {
        let routes = self
            .routes
            .get(dataset)
            .ok_or_else(|| MapsClientError("dataset routes missing".into()))?
            .routes
            .clone();
        let mut output = Vec::new();
        for route in routes
            .into_iter()
            .filter(|r| filter.is_empty() || filter.contains(&r.id))
        {
            let existing = self.routes[dataset].cache.get(&route.id).cloned();
            let points = if let Some(points) = existing {
                points
            } else if route.locations.len() <= 1 {
                route.locations.clone()
            } else {
                let response = self
                    .request(
                        MapsRequestKind::Waypoints,
                        MapsLocationsModel {
                            name: None,
                            locations: route.locations.clone(),
                        },
                    )
                    .map_err(wrap)?;
                if response.waypoints.len() > self.max_entries
                    || response.waypoints.first() != route.locations.first().map(|p| p.as_ref())
                    || response.waypoints.last() != route.locations.last().map(|p| p.as_ref())
                {
                    return Err(MapsClientError("invalid waypoint response".into()));
                }
                response
                    .waypoints
                    .into_iter()
                    .map(|p| {
                        route
                            .locations
                            .iter()
                            .find(|l| ***l == p)
                            .cloned()
                            .unwrap_or_else(|| Arc::new(p))
                    })
                    .collect()
            };
            self.routes
                .get_mut(dataset)
                .expect("known dataset")
                .cache
                .insert(route.id.clone(), points.clone());
            output.push(MapsWaypointRoute {
                id: route.id,
                locations: points,
            });
        }
        Ok(output)
    }
}
fn wrap(error: MapsTransportError) -> MapsClientError {
    MapsClientError(format!(
        "Error getting travel time and distances: {error:?}"
    ))
}
