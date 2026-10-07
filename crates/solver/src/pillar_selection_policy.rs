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
