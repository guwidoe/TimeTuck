// Adapted pinned Community SampleModel/DummyWaypointsExtractor and service-test
// harness transformer fixtures; Apache-2.0. Typed requests replace HTTP chunks.
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
fn p(n: f64) -> GeoLocation {
    GeoLocation::new(n, n).unwrap()
}
fn model(name: Option<&str>, values: &[f64]) -> MapsLocationsModel {
    MapsLocationsModel {
        name: name.map(str::to_owned),
        locations: values.iter().map(|n| Arc::new(p(*n))).collect(),
    }
}
fn response(frames: Vec<Option<MapsMatrixFrame>>) -> MapsResponse {
    MapsResponse {
        frames,
        not_in_map: vec![],
        resolved_map: None,
        hash: "hash".into(),
        waypoints: vec![],
    }
}
#[allow(dead_code)] // Shared source helper used by separate focused targets.
fn frame(a: GeoLocation, b: GeoLocation, t: i64, d: i64) -> MapsMatrixFrame {
    let mut f = MapsMatrixFrame::new();
    f.put(a, b, t, d);
    f.put(b, a, t, d);
    f
}
#[allow(dead_code)] // Shared source helper used by separate focused targets.
#[derive(Default)]
struct Server {
    requests: Vec<MapsRequest>,
    sets: HashMap<String, Vec<GeoLocation>>,
}
impl MapsTransport for Server {
    fn execute(&mut self, r: &MapsRequest) -> Result<MapsResponse, MapsTransportError> {
        self.requests.push(r.clone());
        let points = r.model.locations.iter().map(|p| **p).collect::<Vec<_>>();
        if r.kind == MapsRequestKind::Waypoints {
            let mut out = response(vec![]);
            out.waypoints = points;
            return Ok(out);
        }
        let update = matches!(r.kind, MapsRequestKind::Update { .. });
        if update {
            let old = r
                .model
                .name
                .as_ref()
                .and_then(|n| self.sets.get(n))
                .ok_or(MapsTransportError::Gone)?;
            if old == &points {
                return Err(MapsTransportError::NotModified);
            }
        }
        if let Some(name) = &r.model.name {
            self.sets.insert(name.clone(), points.clone());
        }
        let mut f = MapsMatrixFrame::new();
        let mut excluded = Vec::new();
        let outside = GeoLocation::new(-90., -90.).unwrap();
        for (i, &a) in points.iter().enumerate() {
            if a == outside {
                excluded.push(i);
                continue;
            }
            for &b in &points {
                if b == outside {
                    continue;
                }
                let meters = great_circle_distance(a, b);
                f.put(a, b, spherical_driving_seconds(meters).unwrap(), meters);
            }
        }
        let mut out = response(vec![Some(f)]);
        out.not_in_map = excluded;
        out.resolved_map = if update {
            None
        } else {
            Some("us-georgia".into())
        };
        Ok(out)
    }
}
#[allow(dead_code)] // Shared source helper used by separate focused targets.
fn client() -> MapsClient<Server> {
    MapsClient::new(Server::default(), false, "sourceOption3:1000.0", 100)
}
#[allow(dead_code)] // Shared source helper used by separate focused targets.
fn routes(m: &MapsLocationsModel) -> Vec<MapsWaypointRoute> {
    if m.locations.len() < 2 {
        vec![MapsWaypointRoute {
            id: "id".into(),
            locations: m.locations.clone(),
        }]
    } else {
        m.locations
            .chunks_exact(2)
            .enumerate()
            .map(|(i, p)| MapsWaypointRoute {
                id: format!("id_{i}"),
                locations: p.to_vec(),
            })
            .collect()
    }
}
#[allow(dead_code)] // Shared source helper used by separate focused targets.
fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|s| s.to_string()).collect()
}
