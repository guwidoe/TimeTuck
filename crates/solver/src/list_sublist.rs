//! Configurable contiguous list neighborhoods over immutable snapshots.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange};

/// Inclusive nonempty span lengths. Relocation and unassignment require minimum
/// two; swaps permit singleton sides. Large bounds are clamped to actual lists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListSize {
    minimum: usize,
    maximum: usize,
}
impl SubListSize {
    pub fn new(minimum: usize, maximum: usize) -> Result<Self, ListError> {
        if minimum == 0 || minimum > maximum {
            return Err(ListError::InvalidSampleSize { minimum, maximum });
        }
        Ok(Self { minimum, maximum })
    }
    pub fn minimum(self) -> usize {
        self.minimum
    }
    pub fn maximum(self) -> usize {
        self.maximum
    }
    fn contains(self, n: usize) -> bool {
        (self.minimum..=self.maximum).contains(&n)
    }
    fn multiple(self) -> Result<(), ListError> {
        if self.minimum < 2 {
            Err(ListError::InvalidSampleSize {
                minimum: self.minimum,
                maximum: self.maximum,
            })
        } else {
            Ok(())
        }
    }
}
/// Relocate contiguous spans, optionally reversed or unassigned. Default bounds
/// are two through the entire movable suffix (no implicit sampling cap).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListChangeConfig {
    size: SubListSize,
    reversing: bool,
    unassignment: Option<bool>,
}
impl SubListChangeConfig {
    pub fn new(minimum: usize, maximum: usize) -> Result<Self, ListError> {
        let size = SubListSize::new(minimum, maximum)?;
        size.multiple()?;
        Ok(Self {
            size,
            reversing: true,
            unassignment: None,
        })
    }
    pub fn with_reversing(mut self, enabled: bool) -> Self {
        self.reversing = enabled;
        self
    }
    /// None (the default) follows the model. Explicit true rejects required lists.
    pub fn with_unassignment(mut self, enabled: bool) -> Self {
        self.unassignment = Some(enabled);
        self
    }
    pub fn size(self) -> SubListSize {
        self.size
    }
    pub fn reversing(self) -> bool {
        self.reversing
    }
    pub fn unassignment(self) -> Option<bool> {
        self.unassignment
    }
    pub(crate) fn validate<V: Eq + Clone>(self, p: &ListProblem<V>) -> Result<(), ListError> {
        if self.unassignment == Some(true) && !p.allows_unassigned() {
            Err(ListError::UnassignmentNotAllowed)
        } else {
            Ok(())
        }
    }
}
impl Default for SubListChangeConfig {
    fn default() -> Self {
        Self {
            size: SubListSize {
                minimum: 2,
                maximum: usize::MAX,
            },
            reversing: true,
            unassignment: None,
        }
    }
}
/// Exchange disjoint spans with independent length bounds. Reverse both spans
/// together when enabled; singleton/singleton exchanges never repeat as reversed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListSwapConfig {
    left: SubListSize,
    right: SubListSize,
    reversing: bool,
}
impl SubListSwapConfig {
    pub fn new(minimum: usize, maximum: usize) -> Result<Self, ListError> {
        Self::asymmetric(minimum, maximum, minimum, maximum)
    }
    pub fn asymmetric(
        left_min: usize,
        left_max: usize,
        right_min: usize,
        right_max: usize,
    ) -> Result<Self, ListError> {
        Ok(Self {
            left: SubListSize::new(left_min, left_max)?,
            right: SubListSize::new(right_min, right_max)?,
            reversing: true,
        })
    }
    pub fn with_reversing(mut self, enabled: bool) -> Self {
        self.reversing = enabled;
        self
    }
    pub fn left_size(self) -> SubListSize {
        self.left
    }
    pub fn right_size(self) -> SubListSize {
        self.right
    }
    pub fn reversing(self) -> bool {
        self.reversing
    }
}
impl Default for SubListSwapConfig {
    fn default() -> Self {
        Self {
            left: SubListSize {
                minimum: 1,
                maximum: usize::MAX,
            },
            right: SubListSize {
                minimum: 1,
                maximum: usize::MAX,
            },
            reversing: true,
        }
    }
}
fn ranges<'a, V: Eq + Clone, I: Iterator<Item = usize> + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: SubListSize,
    order: I,
) -> impl Iterator<Item = ListRange> + 'a {
    crate::list_neighborhood::positions(p, a, order).flat_map(move |pos| {
        let available = a.lists()[pos.entity].len() - pos.index;
        (size.minimum..=size.maximum.min(available)).map(move |length| ListRange {
            entity: pos.entity,
            start: pos.index,
            end: pos.index + length,
        })
    })
}
pub(crate) fn changes<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    c: SubListChangeConfig,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    ranges(p, a, c.size, order.clone()).flat_map(move |source| {
        let span = &a.lists()[source.entity][source.start..source.end];
        let destinations = order
            .clone()
            .filter(move |&e| !p.entities()[e].is_pinned() && span.iter().all(|v| p.accepts(e, v)))
            .flat_map(move |e| {
                let length = a.lists()[e].len() - if e == source.entity { span.len() } else { 0 };
                (p.entities()[e].pinned_prefix()..=length).flat_map(move |index| {
                    [false, true]
                        .into_iter()
                        .filter(move |&reverse| {
                            (!reverse || c.reversing)
                                && (reverse || e != source.entity || index != source.start)
                        })
                        .map(move |reverse| ListMove::SubListChange {
                            entity: source.entity,
                            start: source.start,
                            end: source.end,
                            destination: ListPosition { entity: e, index },
                            reverse,
                        })
                })
            });
        destinations.chain(c.unassignment.unwrap_or(p.allows_unassigned()).then_some(
            ListMove::Unassign {
                entity: source.entity,
                start: source.start,
                end: source.end,
            },
        ))
    })
}
pub(crate) fn swaps<'a, V: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    c: SubListSwapConfig,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    ranges(p, a, c.left, order.clone()).flat_map(move |left| {
        ranges(p, a, c.right, order.clone())
            .filter(move |right| {
                let disjoint = left.entity != right.entity
                    || left.end <= right.start
                    || right.end <= left.start;
                let canonical = (left.entity, left.start) < (right.entity, right.start);
                let reverse_possible = c.left.contains(right.end - right.start)
                    && c.right.contains(left.end - left.start);
                disjoint
                    && (canonical || !reverse_possible)
                    && a.lists()[left.entity][left.start..left.end]
                        .iter()
                        .all(|v| p.accepts(right.entity, v))
                    && a.lists()[right.entity][right.start..right.end]
                        .iter()
                        .all(|v| p.accepts(left.entity, v))
            })
            .flat_map(move |right| {
                [false, true]
                    .into_iter()
                    .filter(move |&reverse| {
                        !reverse
                            || (c.reversing
                                && (left.end - left.start > 1 || right.end - right.start > 1))
                    })
                    .map(move |reverse| ListMove::SubListSwap {
                        left,
                        right,
                        reverse,
                    })
            })
    })
}
pub(crate) fn unassigns<'a, V: Eq + Clone, I: Iterator<Item = usize> + 'a>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: SubListSize,
    order: I,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    ranges(p, a, size, order).map(|r| ListMove::Unassign {
        entity: r.entity,
        start: r.start,
        end: r.end,
    })
}
pub(crate) fn validate_unassign<V: Eq + Clone>(
    p: &ListProblem<V>,
    size: SubListSize,
) -> Result<(), ListError> {
    size.multiple()?;
    if p.allows_unassigned() {
        Ok(())
    } else {
        Err(ListError::UnassignmentNotAllowed)
    }
}
/// Lazy relocations in source/length/destination order; optional unassignment
/// follows each source span. Destinations use indices after source removal.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])?;
/// let a = p.initial_assignment();
/// let config = SubListChangeConfig::new(2, 3)?.with_reversing(false);
/// let moves: Vec<_> = list_sublist_change_moves_with_config(&p, &a, config)?.collect();
/// assert!(!moves.is_empty());
/// let search = LocalSearchConfig::<SimpleScore> {
///     neighborhood: Neighborhood::SubListChange(config), ..Default::default()
/// };
/// assert_eq!(search.neighborhood, Neighborhood::SubListChange(config));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn list_sublist_change_moves_with_config<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    c: SubListChangeConfig,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    c.validate(p)?;
    Ok(changes(p, a, c, 0..p.entities().len()))
}
/// Lazy swaps with no overlapping or repeated symmetric span pairs. Each side
/// obeys its configured bounds; asymmetric configurations retain both placements.
pub fn list_sublist_swap_moves_with_config<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    c: SubListSwapConfig,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(swaps(p, a, c, 0..p.entities().len()))
}
/// Unassign each contiguous span of allowed length exactly once. Requires an
/// optional-list model and minimum two; pins and pinned prefixes are preserved.
pub fn list_sublist_unassign_moves<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    size: SubListSize,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    validate_unassign(p, size)?;
    Ok(unassigns(p, a, size, 0..p.entities().len()))
}
