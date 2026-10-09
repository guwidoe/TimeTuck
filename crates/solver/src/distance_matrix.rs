//! Directed integral routing costs with stable, matrix-scoped location indexes.
use std::{
    collections::HashMap,
    error::Error,
    fmt,
    hash::Hash,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_MATRIX: AtomicU64 = AtomicU64::new(1);
/// Native location limit. Capacity validation happens before allocation.
pub const MAX_MATRIX_LOCATIONS: usize = 32767;
#[derive(Clone, Debug, PartialEq)]
pub enum RoutingError {
    InvalidCapacity { expected: i64, maximum: usize },
    InvalidLatitude(f64),
    InvalidLongitude(f64),
    Allocation,
    IdentityExhausted,
    ForeignLocation,
    MissingDistance,
    DistanceOverflow,
    Unconfigured,
    MissingTimeframe { index: usize },
    TimeframeOutOfBounds { index: usize, count: usize },
}
impl fmt::Display for RoutingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity { expected, maximum } => write!(
                f,
                "expected location count {expected} must be between 0 and {maximum}"
            ),
            Self::InvalidLatitude(v) => write!(f, "Latitude must be between -90 and 90, found {v}"),
            Self::InvalidLongitude(v) => {
                write!(f, "Longitude must be between -180 and 180, found {v}")
            }
            Self::Allocation => f.write_str("routing matrix allocation failed"),
            Self::IdentityExhausted => f.write_str("matrix identity space exhausted"),
            Self::ForeignLocation => f.write_str("location index belongs to another matrix"),
            Self::MissingDistance => f.write_str("directed distance is not present"),
            Self::DistanceOverflow => f.write_str("route cost exceeds i64"),
            Self::Unconfigured => f.write_str("no routing matrix configured"),
            Self::MissingTimeframe { index } => {
                write!(f, "No matrix fetched for timeframe {index}")
            }
            Self::TimeframeOutOfBounds { index, count } => {
                write!(f, "timeframe {index} out of bounds for {count} matrices")
            }
        }
    }
}
impl Error for RoutingError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct LocationIndex {
    matrix: u64,
    index: usize,
}
impl LocationIndex {
    pub fn ordinal(self) -> usize {
        self.index
    }
}
/// Dense directed matrix of i64 costs (caller-defined units). Zero and missing
/// entries are distinct. Registration is idempotent by key equality and preserves
/// insertion order. Cached indexes avoid hashing and remain valid after growth.
/// Foreign-matrix indexes are rejected, including when their ordinals coincide.
///
/// Triangular pairs store both directions. Appending locations extends storage
/// without changing any existing offsets. Dense storage takes O(capacity²) space;
/// indexed lookup is O(1). No symmetry or default diagonal cost is assumed.
/// This type is intentionally not Clone: a new matrix has a new index namespace.
pub struct DirectedDistanceMatrix<K> {
    identity: u64,
    capacity: usize,
    locations: Vec<K>,
    indexes: HashMap<K, usize>,
    values: Vec<i64>,
    present: Vec<u64>,
}
fn checked_capacity(expected: i64) -> Result<usize, RoutingError> {
    if expected < 0 || expected as u64 > MAX_MATRIX_LOCATIONS as u64 {
        Err(RoutingError::InvalidCapacity {
            expected,
            maximum: MAX_MATRIX_LOCATIONS,
        })
    } else {
        Ok(expected as usize)
    }
}
// Derived from a diagonal ordering: each larger endpoint appends a band of pairs.
fn pair_index(from: usize, to: usize) -> usize {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    hi * (hi + 1) / 2 + hi - lo
}
impl<K: Eq + Hash + Clone> DirectedDistanceMatrix<K> {
    /// Signed capacity hints allow invalid external counts to fail before allocation.
    pub fn with_capacity(expected: i64) -> Result<Self, RoutingError> {
        let capacity = checked_capacity(expected)?;
        let identity = NEXT_MATRIX
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| RoutingError::IdentityExhausted)?;
        let mut m = Self {
            identity,
            capacity: 0,
            locations: vec![],
            indexes: HashMap::new(),
            values: vec![],
            present: vec![],
        };
        m.grow(capacity)?;
        Ok(m)
    }
    fn grow(&mut self, capacity: usize) -> Result<(), RoutingError> {
        if capacity <= self.capacity {
            return Ok(());
        }
        let cells = capacity * (capacity + 1);
        let words = cells.div_ceil(64);
        self.values
            .try_reserve_exact(cells - self.values.len())
            .map_err(|_| RoutingError::Allocation)?;
        self.present
            .try_reserve_exact(words - self.present.len())
            .map_err(|_| RoutingError::Allocation)?;
        self.values.resize(cells, 0);
        self.present.resize(words, 0);
        self.capacity = capacity;
        Ok(())
    }
    /// Reserve at least this many locations, retaining all costs and index handles.
    /// Smaller valid hints are no-ops; removing locations is not supported.
    pub fn reserve_locations(&mut self, expected: i64) -> Result<(), RoutingError> {
        self.grow(checked_capacity(expected)?)
    }
    pub fn len(&self) -> usize {
        self.locations.len()
    }
    pub fn is_empty(&self) -> bool {
        self.locations.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    pub fn index(&self, key: &K) -> Option<LocationIndex> {
        self.indexes.get(key).map(|&index| LocationIndex {
            matrix: self.identity,
            index,
        })
    }
    pub fn register(&mut self, key: K) -> Result<LocationIndex, RoutingError> {
        if let Some(index) = self.index(&key) {
            return Ok(index);
        }
        let next = self.len();
        if next == MAX_MATRIX_LOCATIONS {
            return Err(RoutingError::InvalidCapacity {
                expected: next as i64 + 1,
                maximum: MAX_MATRIX_LOCATIONS,
            });
        }
        self.indexes
            .try_reserve(1)
            .map_err(|_| RoutingError::Allocation)?;
        self.locations
            .try_reserve(1)
            .map_err(|_| RoutingError::Allocation)?;
        if next == self.capacity {
            self.grow((self.capacity.max(1) * 2).min(MAX_MATRIX_LOCATIONS))?;
        }
        let stored = key.clone();
        self.indexes.insert(key, next);
        self.locations.push(stored);
        Ok(LocationIndex {
            matrix: self.identity,
            index: next,
        })
    }
    fn validate(&self, index: LocationIndex) -> Result<usize, RoutingError> {
        if index.matrix != self.identity || index.index >= self.len() {
            Err(RoutingError::ForeignLocation)
        } else {
            Ok(index.index)
        }
    }
    pub fn location(&self, index: LocationIndex) -> Result<&K, RoutingError> {
        Ok(&self.locations[self.validate(index)?])
    }
    fn offset(&self, from: LocationIndex, to: LocationIndex) -> Result<usize, RoutingError> {
        let a = self.validate(from)?;
        let b = self.validate(to)?;
        Ok(pair_index(a, b) * 2 + usize::from(a > b))
    }
    pub fn put_indexed(
        &mut self,
        from: LocationIndex,
        to: LocationIndex,
        cost: i64,
    ) -> Result<(), RoutingError> {
        let i = self.offset(from, to)?;
        self.values[i] = cost;
        self.present[i / 64] |= 1u64 << (i % 64);
        Ok(())
    }
    pub fn get_indexed(
        &self,
        from: LocationIndex,
        to: LocationIndex,
    ) -> Result<Option<i64>, RoutingError> {
        let i = self.offset(from, to)?;
        Ok((self.present[i / 64] & (1u64 << (i % 64)) != 0).then_some(self.values[i]))
    }
    /// Register missing keys before storing the directed cost. Registration may
    /// remain after a later allocation failure; already stored costs are preserved.
    pub fn put(&mut self, from: &K, to: &K, cost: i64) -> Result<(), RoutingError> {
        let a = match self.index(from) {
            Some(i) => i,
            None => self.register(from.clone())?,
        };
        let b = match self.index(to) {
            Some(i) => i,
            None => self.register(to.clone())?,
        };
        self.put_indexed(a, b, cost)
    }
    pub fn get(&self, from: &K, to: &K) -> Option<i64> {
        self.get_indexed(self.index(from)?, self.index(to)?)
            .ok()
            .flatten()
    }
    /// Sum a path, optionally closing it at the origin. An empty route costs zero.
    /// Missing edges and overflow are errors, not implicit zero or saturated costs.
    pub fn route_cost(
        &self,
        origin: LocationIndex,
        stops: impl IntoIterator<Item = LocationIndex>,
        closed: bool,
    ) -> Result<i64, RoutingError> {
        self.validate(origin)?;
        let mut last = origin;
        let mut total = 0i64;
        let mut any = false;
        for stop in stops {
            let cost = self
                .get_indexed(last, stop)?
                .ok_or(RoutingError::MissingDistance)?;
            total = total
                .checked_add(cost)
                .ok_or(RoutingError::DistanceOverflow)?;
            last = stop;
            any = true;
        }
        if closed && any {
            total = total
                .checked_add(
                    self.get_indexed(last, origin)?
                        .ok_or(RoutingError::MissingDistance)?,
                )
                .ok_or(RoutingError::DistanceOverflow)?;
        }
        Ok(total)
    }
}
/// One cached handle for an immutable location key. Rebinding replaces the cache;
/// requesting it for a different matrix returns None rather than a stale ordinal.
pub struct CachedLocation<K> {
    key: K,
    index: Option<LocationIndex>,
}
impl<K: Eq + Hash + Clone> CachedLocation<K> {
    pub fn new(key: K) -> Self {
        Self { key, index: None }
    }
    pub fn key(&self) -> &K {
        &self.key
    }
    pub fn index_for(&self, m: &DirectedDistanceMatrix<K>) -> Option<LocationIndex> {
        self.index.filter(|i| i.matrix == m.identity)
    }
    pub fn bind(
        &mut self,
        m: &mut DirectedDistanceMatrix<K>,
    ) -> Result<LocationIndex, RoutingError> {
        let i = m.register(self.key.clone())?;
        self.index = Some(i);
        Ok(i)
    }
}
#[cfg(test)]
mod cases;
