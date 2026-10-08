//! Bounded key-dependent value selection with reusable occurrence snapshots.
use crate::{
    PlacementSource, SelectionLifecycleEvent, SelectionProduct, SelectionProductError,
    SelectionSource,
};
use std::{error::Error, fmt, iter::FusedIterator, sync::Arc};

pub type KeyedSelectionPair<K, V> = (Arc<K>, Arc<V>);
type Row<K, V> = (Arc<K>, Arc<[Arc<V>]>);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyedSelectionLimits {
    pub maximum_keys: usize,
    pub maximum_values_per_key: usize,
    pub maximum_values: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyedSelectionError {
    Keys(SelectionProductError),
    UnboundedValues,
    ValuesLimit {
        key: usize,
        maximum: usize,
    },
    TotalLimit {
        maximum: usize,
    },
    SizeMismatch {
        key: usize,
        declared: usize,
        actual: usize,
    },
    /// None identifies the key draw; Some identifies the selected nonempty row.
    InvalidDraw {
        key: Option<usize>,
        bound: usize,
        actual: usize,
    },
}
impl fmt::Display for KeyedSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dependent selection: {self:?}")
    }
}
impl Error for KeyedSelectionError {}
impl From<SelectionProductError> for KeyedSelectionError {
    fn from(e: SelectionProductError) -> Self {
        Self::Keys(e)
    }
}

/// Choose an eligible key index, then a value index for that key. Bounds are
/// strictly positive. The value callback can use the actual selected key.
pub enum KeyedSelectionDraw<'a, K> {
    Key { bound: usize },
    Value { key: &'a K, bound: usize },
}
impl<K> KeyedSelectionDraw<'_, K> {
    pub fn bound(&self) -> usize {
        match self {
            Self::Key { bound } | Self::Value { bound, .. } => *bound,
        }
    }
}
/// Immutable key-major pools. Empty keys are omitted; duplicate occurrences remain
/// distinct, including equal values under different keys. No Clone payload bound.
pub struct KeyedSelectionSnapshot<K, V> {
    rows: Arc<[Row<K, V>]>,
    len: usize,
}
impl<K, V> Clone for KeyedSelectionSnapshot<K, V> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            len: self.len,
        }
    }
}
impl<K, V> KeyedSelectionSnapshot<K, V> {
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn key_count(&self) -> usize {
        self.rows.len()
    }
    pub fn iter(&self) -> KeyedSelectionIter<K, V> {
        KeyedSelectionIter {
            snapshot: self.clone(),
            key: 0,
            value: 0,
            remaining: self.len,
        }
    }
    pub fn samples_with<D: FnMut(KeyedSelectionDraw<'_, K>) -> usize>(
        &self,
        draw: D,
    ) -> KeyedSelectionSamples<K, V, D> {
        KeyedSelectionSamples {
            snapshot: self.clone(),
            draw,
            failed: false,
        }
    }
}
pub struct KeyedSelectionIter<K, V> {
    snapshot: KeyedSelectionSnapshot<K, V>,
    key: usize,
    value: usize,
    remaining: usize,
}
impl<K, V> Iterator for KeyedSelectionIter<K, V> {
    type Item = KeyedSelectionPair<K, V>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let (key, values) = &self.snapshot.rows[self.key];
        let pair = (key.clone(), values[self.value].clone());
        self.remaining -= 1;
        self.value += 1;
        if self.value == values.len() {
            self.key += 1;
            self.value = 0;
        }
        Some(pair)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> ExactSizeIterator for KeyedSelectionIter<K, V> {}
impl<K, V> FusedIterator for KeyedSelectionIter<K, V> {}
pub struct KeyedSelectionSamples<K, V, D> {
    snapshot: KeyedSelectionSnapshot<K, V>,
    draw: D,
    failed: bool,
}
impl<K, V, D: FnMut(KeyedSelectionDraw<'_, K>) -> usize> Iterator
    for KeyedSelectionSamples<K, V, D>
{
    type Item = Result<KeyedSelectionPair<K, V>, KeyedSelectionError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.snapshot.is_empty() {
            return None;
        }
        let bound = self.snapshot.rows.len();
        let index = (self.draw)(KeyedSelectionDraw::Key { bound });
        if index >= bound {
            self.failed = true;
            return Some(Err(KeyedSelectionError::InvalidDraw {
                key: None,
                bound,
                actual: index,
            }));
        }
        let (key, values) = &self.snapshot.rows[index];
        let bound = values.len();
        let value = (self.draw)(KeyedSelectionDraw::Value { key, bound });
        if value >= bound {
            self.failed = true;
            return Some(Err(KeyedSelectionError::InvalidDraw {
                key: Some(index),
                bound,
                actual: value,
            }));
        }
        Some(Ok((key.clone(), values[value].clone())))
    }
}
impl<K, V, D: FnMut(KeyedSelectionDraw<'_, K>) -> usize> FusedIterator
    for KeyedSelectionSamples<K, V, D>
{
}

/// Collect a finite key source and finite values separately for each key using
/// PlacementSource. Reuse pools until an event, source edit or invalidate. Context
/// membership facts must stay stable within that scope; use Step caching when
/// domains change with assignment. Each event invalidates first, then forwards
/// once to keys and values, including empty selections. Errors/panics publish no
/// partial snapshot; callback effects are not rolled back. Held snapshots survive.
///
/// Original traversal is key-major and retains every supplied candidate, including
/// no-ops and other currently non-doable pairs. The caller maps/filters legality
/// against the current assignment. size/try_snapshot/open may collect sources.
/// Separate key, per-key value and total-value limits bound stored occurrences.
/// Neither context, key nor value needs Clone. Returned pairs share Arc handles.
///
/// random(seed) selects eligible keys uniformly, then values uniformly within a
/// key, with replacement (not uniform over all pairs when pool sizes differ).
/// The stream continues across openings/events; reseed to restart. Native seeds
/// do not reproduce Java traces. Random mode is marked never-ending even if empty;
/// size reports pair cardinality, zero for empty. Wrap with LimitedSelectionSource
/// before a CachedMoveProvider. Original mode can feed that provider directly.
/// Trait methods panic on collection errors; try_snapshot returns typed errors.
pub struct KeyedSelectionSource<C, K, V, KS, VS> {
    keys: SelectionProduct<C, K, KS>,
    values: VS,
    limits: KeyedSelectionLimits,
    prepared: Option<KeyedSelectionSnapshot<K, V>>,
    random: Option<crate::random::SplitMix64>,
}
impl<C, K, V, KS: SelectionSource<C, K>, VS: PlacementSource<C, K, V>>
    KeyedSelectionSource<C, K, V, KS, VS>
{
    pub fn new(keys: KS, values: VS, limits: KeyedSelectionLimits) -> Self {
        Self {
            keys: SelectionProduct::new(vec![keys], false, limits.maximum_keys),
            values,
            limits,
            prepared: None,
            random: None,
        }
    }
    pub fn random(mut self, seed: u64) -> Self {
        self.random = Some(crate::random::SplitMix64::new(seed));
        self
    }
    pub fn reseed(&mut self, seed: u64) {
        if self.random.is_some() {
            self.random = Some(crate::random::SplitMix64::new(seed));
        }
    }
    pub fn key_source(&self) -> &KS {
        &self.keys.children()[0]
    }
    pub fn value_source(&self) -> &VS {
        &self.values
    }
    pub fn key_source_mut(&mut self) -> &mut KS {
        self.invalidate();
        &mut self.keys.children_mut()[0]
    }
    pub fn value_source_mut(&mut self) -> &mut VS {
        self.invalidate();
        &mut self.values
    }
    pub fn invalidate(&mut self) {
        self.prepared = None;
    }
    pub fn try_snapshot(
        &mut self,
        context: &C,
    ) -> Result<KeyedSelectionSnapshot<K, V>, KeyedSelectionError> {
        if let Some(snapshot) = &self.prepared {
            return Ok(snapshot.clone());
        }
        if self.values.is_never_ending() {
            return Err(KeyedSelectionError::UnboundedValues);
        }
        let keys = self.keys.snapshot(context)?;
        let mut rows = Vec::new();
        let mut len = 0usize;
        for (index, key) in keys.child_pool(0).unwrap_or(&[]).iter().enumerate() {
            let declared = self.values.size(context, key);
            let maximum = self.limits.maximum_values_per_key;
            if declared.is_some_and(|n| n > maximum) {
                return Err(KeyedSelectionError::ValuesLimit {
                    key: index,
                    maximum,
                });
            }
            if declared.is_some_and(|n| n > self.limits.maximum_values - len) {
                return Err(KeyedSelectionError::TotalLimit {
                    maximum: self.limits.maximum_values,
                });
            }
            let mut pool = Vec::new();
            for value in self.values.open(context, key) {
                if pool.len() == maximum {
                    return Err(KeyedSelectionError::ValuesLimit {
                        key: index,
                        maximum,
                    });
                }
                if len == self.limits.maximum_values {
                    return Err(KeyedSelectionError::TotalLimit {
                        maximum: self.limits.maximum_values,
                    });
                }
                pool.push(Arc::new(value));
                len += 1;
            }
            if let Some(declared) = declared
                && declared != pool.len()
            {
                return Err(KeyedSelectionError::SizeMismatch {
                    key: index,
                    declared,
                    actual: pool.len(),
                });
            }
            if !pool.is_empty() {
                rows.push((key.clone(), Arc::from(pool)));
            }
        }
        let snapshot = KeyedSelectionSnapshot {
            rows: rows.into(),
            len,
        };
        self.prepared = Some(snapshot.clone());
        Ok(snapshot)
    }
}
impl<C, K, V, KS: SelectionSource<C, K>, VS: PlacementSource<C, K, V>>
    SelectionSource<C, KeyedSelectionPair<K, V>> for KeyedSelectionSource<C, K, V, KS, VS>
{
    fn is_never_ending(&self) -> bool {
        self.random.is_some()
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.invalidate();
        self.keys.event(event, context);
        self.values.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        Some(
            self.try_snapshot(context)
                .unwrap_or_else(|e| panic!("{e}"))
                .len(),
        )
    }
    fn open<'a>(
        &'a mut self,
        context: &'a C,
    ) -> Box<dyn Iterator<Item = KeyedSelectionPair<K, V>> + 'a>
    where
        KeyedSelectionPair<K, V>: 'a,
    {
        let snapshot = self.try_snapshot(context).unwrap_or_else(|e| panic!("{e}"));
        if let Some(random) = &mut self.random {
            Box::new(
                snapshot
                    .samples_with(move |draw: KeyedSelectionDraw<'_, K>| {
                        random.below(draw.bound() as u64) as usize
                    })
                    .map(|r| r.unwrap_or_else(|e| panic!("{e}"))),
            )
        } else {
            Box::new(snapshot.iter())
        }
    }
}
