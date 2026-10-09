//! Borrowed single and departure-dependent routing matrices.
use crate::{DirectedDistanceMatrix, RoutingError};
use std::hash::Hash;
type Frames<'a, K, T> = (
    Vec<Option<&'a DirectedDistanceMatrix<K>>>,
    Box<dyn Fn(&T) -> usize + 'a>,
);
/// A matrix set for one measure (for example distance or travel time). Use
/// separate sets to keep measures independent. The caller owns timestamp/bucket
/// semantics; this type does not convert time zones or assume a traffic model.
///
/// Without a timestamp, a single matrix wins; otherwise the first configured
/// timeframe is used. With a timestamp, configured timeframes win and a missing
/// selected slot is an error. A single matrix is the fallback only when no
/// timeframe table is configured. Missing edges never fall back to another table.
pub struct TimeframeMatrices<'a, K, T> {
    single: Option<&'a DirectedDistanceMatrix<K>>,
    frames: Option<Frames<'a, K, T>>,
}
impl<K, T> Default for TimeframeMatrices<'_, K, T> {
    fn default() -> Self {
        Self {
            single: None,
            frames: None,
        }
    }
}
impl<'a, K: Eq + Hash + Clone, T> TimeframeMatrices<'a, K, T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_single(&mut self, m: &'a DirectedDistanceMatrix<K>) {
        self.single = Some(m);
    }
    pub fn set_timeframes(
        &mut self,
        frames: Vec<Option<&'a DirectedDistanceMatrix<K>>>,
        resolve: impl Fn(&T) -> usize + 'a,
    ) {
        self.frames = Some((frames, Box::new(resolve)));
    }
    pub fn matrix(&self, at: Option<&T>) -> Result<&'a DirectedDistanceMatrix<K>, RoutingError> {
        if let (Some(at), Some((frames, resolve))) = (at, &self.frames) {
            let index = resolve(at);
            return frames
                .get(index)
                .ok_or(RoutingError::TimeframeOutOfBounds {
                    index,
                    count: frames.len(),
                })?
                .ok_or(RoutingError::MissingTimeframe { index });
        }
        self.single
            .or_else(|| {
                self.frames
                    .as_ref()
                    .and_then(|(frames, _)| frames.iter().find_map(|m| *m))
            })
            .ok_or(RoutingError::Unconfigured)
    }
    pub fn get(&self, from: &K, to: &K, at: Option<&T>) -> Result<i64, RoutingError> {
        self.matrix(at)?
            .get(from, to)
            .ok_or(RoutingError::MissingDistance)
    }
}
