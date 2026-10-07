//! Reusable whole-pillar, subset and comparator-ordered selection policies.
use crate::{Pillar, SubPillarSelection, SubPillarSizeError};
use std::{cmp::Ordering, error::Error, fmt, sync::Arc};
type Compare = dyn Fn(&[usize], &[usize]) -> Ordering + Send + Sync;
/// Compare entity rows by their selected variable slots. A cloned handle keeps
/// comparator identity; independently constructed handles are unequal, even if
/// their functions happen to return the same ordering. Comparators must define a
/// consistent order and must not mutate the model used by a search.
#[derive(Clone)]
pub struct PillarComparator(Arc<Compare>);
impl PillarComparator {
    pub fn new(compare: impl Fn(&[usize], &[usize]) -> Ordering + Send + Sync + 'static) -> Self {
        Self(Arc::new(compare))
    }
}
impl PartialEq for PillarComparator {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for PillarComparator {}
impl fmt::Debug for PillarComparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PillarComparator(..)")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PillarSelectionPolicyError {
    Size(SubPillarSizeError),
    MissingComparator,
}
impl fmt::Display for PillarSelectionPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size(e) => e.fmt(f),
            Self::MissingComparator => f.write_str("sequential subpillars require a comparator"),
        }
    }
}
impl Error for PillarSelectionPolicyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Size(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Selection {
    Whole,
    Subsets(SubPillarSelection),
    Sequential(SubPillarSelection, PillarComparator),
}
/// Validated reusable policy. Sequential selection takes contiguous windows after
/// a stable comparator sort of entity rows; each resulting Pillar retains its
/// canonical row representation. Selection is lazy after copying/sorting metadata,
/// with no planning-value clones. Sizes beyond the pillar length are clipped;
/// a pillar smaller than the minimum produces no subsets. Whole selects it once.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PillarSelectionPolicy(Selection);
impl PillarSelectionPolicy {
    pub fn whole() -> Self {
        Self(Selection::Whole)
    }
    pub fn subsets(min: usize, max: usize) -> Result<Self, PillarSelectionPolicyError> {
        SubPillarSelection::combinations(min, max)
            .map(|s| Self(Selection::Subsets(s)))
            .map_err(PillarSelectionPolicyError::Size)
    }
    /// Validate optional configuration input; None cannot create a sequential policy.
    pub fn sequential(
        min: usize,
        max: usize,
        comparator: Option<PillarComparator>,
    ) -> Result<Self, PillarSelectionPolicyError> {
        let sizes =
            SubPillarSelection::consecutive(min, max).map_err(PillarSelectionPolicyError::Size)?;
        Ok(Self(Selection::Sequential(
            sizes,
            comparator.ok_or(PillarSelectionPolicyError::MissingComparator)?,
        )))
    }
    /// Community default bounds, 1..=2,147,483,647. Explicit sequential policies
    /// may use larger native usize bounds.
    pub fn sequential_unlimited(comparator: PillarComparator) -> Self {
        Self::sequential(1, i32::MAX as usize, Some(comparator))
            .expect("valid built-in sizes and comparator")
    }
    pub fn is_enabled(&self) -> bool {
        !matches!(self.0, Selection::Whole)
    }
    pub fn sizes(&self) -> Option<(usize, usize)> {
        match self.0 {
            Selection::Whole => None,
            Selection::Subsets(s) | Selection::Sequential(s, _) => Some((s.minimum(), s.maximum())),
        }
    }
    pub fn comparator(&self) -> Option<&PillarComparator> {
        match &self.0 {
            Selection::Sequential(_, c) => Some(c),
            _ => None,
        }
    }
    /// Return independently owned pillar metadata suitable for change/swap move
    /// construction. Uniformity, domains and pins remain the ordinary move/model
    /// checks. Comparator panics do not modify the supplied pillar.
    pub fn select(&self, pillar: &Pillar) -> Box<dyn Iterator<Item = Pillar>> {
        let selection = match self.0 {
            Selection::Whole => return Box::new(std::iter::once(pillar.clone())),
            Selection::Subsets(s) | Selection::Sequential(s, _) => s,
        };
        let mut rows = pillar.rows().to_vec();
        if let Some(comparator) = self.comparator() {
            rows.sort_by(|a, b| (comparator.0)(a, b));
        }
        Box::new(selection.indices(rows.len()).map(move |indices| {
            Pillar::from_rows(indices.into_iter().map(|i| rows[i].clone()).collect())
                .expect("subset of valid pillar rows")
        }))
    }
}

/// Owned row metadata prepared once for repeated direct random draws.
pub(crate) struct PreparedPillarSamples {
    rows: Vec<Vec<usize>>,
    min: usize,
    max: usize,
    consecutive: bool,
}
impl PillarSelectionPolicy {
    pub(crate) fn prepare_samples(&self, pillar: &Pillar) -> Option<PreparedPillarSamples> {
        let n = pillar.rows().len();
        let (min, max) = self.sizes().unwrap_or((n, n));
        if min > n {
            return None;
        }
        let mut rows = pillar.rows().to_vec();
        if let Some(c) = self.comparator() {
            rows.sort_by(|a, b| (c.0)(a, b));
        }
        Some(PreparedPillarSamples {
            rows,
            min,
            max: max.min(n),
            consecutive: matches!(self.0, Selection::Sequential(..)),
        })
    }
    /// Draw at most `count` independent subpillars with replacement using a native
    /// seed. A pillar smaller than the minimum produces no samples. Sizes are
    /// uniform over the clipped bounds; conditional on size, subsets (or sorted
    /// contiguous windows) are uniform. This is not uniform over all subsets when
    /// their sizes differ. Whole-pillar selection repeats the whole pillar.
    ///
    /// Copies/sorts row metadata once. Each draw selects directly without
    /// enumerating combinations; arbitrary subsets use O(k) scratch entries for
    /// k selected rows. Results own only slot metadata, not planning values.
    /// Native seeds do not reproduce Java random traces.
    ///
    /// ```
    /// use timetuck::*;
    /// let pillar = Pillar::new(0..80)?;
    /// let policy = PillarSelectionPolicy::subsets(40, 40)?;
    /// let sample = policy.sample(&pillar, 42, 3).collect::<Vec<_>>();
    /// assert_eq!(sample.len(), 3);
    /// assert!(sample.iter().all(|p| p.rows().len() == 40));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn sample(
        &self,
        pillar: &Pillar,
        seed: u64,
        count: u64,
    ) -> Box<dyn Iterator<Item = Pillar>> {
        if count == 0 {
            return Box::new(std::iter::empty());
        }
        let Some(prepared) = self.prepare_samples(pillar) else {
            return Box::new(std::iter::empty());
        };
        let mut random = crate::random::SplitMix64::new(seed);
        Box::new((0..count).map(move |_| prepared.draw(&mut random)))
    }
}
impl PreparedPillarSamples {
    pub(crate) fn draw(&self, random: &mut crate::random::SplitMix64) -> Pillar {
        let size = self.min + random.below((self.max - self.min + 1) as u64) as usize;
        let n = self.rows.len();
        let indices: Vec<usize> = if size == n {
            (0..n).collect()
        } else if self.consecutive {
            let start = random.below((n - size + 1) as u64) as usize;
            (start..start + size).collect()
        } else {
            // Sparse partial Fisher-Yates: represent only displaced indices.
            let mut displaced = std::collections::BTreeMap::new();
            (0..size)
                .map(|i| {
                    let j = i + random.below((n - i) as u64) as usize;
                    let picked = displaced.remove(&j).unwrap_or(j);
                    if j != i {
                        let replacement = displaced.remove(&i).unwrap_or(i);
                        displaced.insert(j, replacement);
                    }
                    picked
                })
                .collect()
        };
        Pillar::from_rows(indices.into_iter().map(|i| self.rows[i].clone()).collect())
            .expect("sampled distinct rows from a valid pillar")
    }
}
