//! Bounded, lazy selection of members from equal-value groups.
use crate::{Assignment, FiniteProblem, ModelError, Move, Pillar, pillar};
use std::{error::Error, fmt, iter::FusedIterator, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubPillarSizeError {
    pub min: usize,
    pub max: usize,
}
impl fmt::Display for SubPillarSizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "subpillar sizes must satisfy 1 <= min <= max, got {}..={}",
            self.min, self.max
        )
    }
}
impl Error for SubPillarSizeError {}

/// Enumerate nonempty subsets by increasing size, then lexicographic member
/// position. Consecutive selection restricts each subset to a contiguous window
/// in the supplied member order. Bounds larger than a group are clipped to its
/// size; a group below the minimum yields nothing. No subset list is precomputed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SubPillarSelection {
    min: usize,
    max: usize,
    consecutive: bool,
}
impl SubPillarSelection {
    pub fn combinations(min: usize, max: usize) -> Result<Self, SubPillarSizeError> {
        if min == 0 || min > max {
            return Err(SubPillarSizeError { min, max });
        }
        Ok(Self {
            min,
            max,
            consecutive: false,
        })
    }
    pub fn consecutive(min: usize, max: usize) -> Result<Self, SubPillarSizeError> {
        Ok(Self {
            consecutive: true,
            ..Self::combinations(min, max)?
        })
    }
    pub fn minimum(self) -> usize {
        self.min
    }
    pub fn maximum(self) -> usize {
        self.max
    }
    pub fn is_consecutive(self) -> bool {
        self.consecutive
    }
    pub(crate) fn indices(self, n: usize) -> Subsets {
        Subsets {
            n,
            max: self.max.min(n),
            consecutive: self.consecutive,
            next: (self.min <= n).then(|| (0..self.min).collect()),
        }
    }
}
pub(crate) struct Subsets {
    n: usize,
    max: usize,
    consecutive: bool,
    next: Option<Vec<usize>>,
}
impl Iterator for Subsets {
    type Item = Vec<usize>;
    fn next(&mut self) -> Option<Self::Item> {
        let result = self.next.take()?;
        let k = result.len();
        let mut following = result.clone();
        if self.consecutive {
            if result[k - 1] < self.n - 1 {
                for i in &mut following {
                    *i += 1;
                }
                self.next = Some(following);
                return Some(result);
            }
        } else if let Some(i) = (0..k).rev().find(|&i| following[i] < self.n - k + i) {
            following[i] += 1;
            for j in i + 1..k {
                following[j] = following[j - 1] + 1;
            }
            self.next = Some(following);
            return Some(result);
        }
        if k < self.max {
            self.next = Some((0..k + 1).collect());
        }
        Some(result)
    }
}
impl FusedIterator for Subsets {}

impl Pillar {
    /// Select member rows in this pillar's canonical slot order. Column order
    /// is retained. This selects members only; assignment uniformity is checked
    /// when constructing a move with the resulting pillar.
    pub fn subpillars(&self, selection: SubPillarSelection) -> impl Iterator<Item = Self> + '_ {
        selection.indices(self.rows().len()).map(|indices| {
            Pillar::from_rows(
                indices
                    .into_iter()
                    .map(|i| self.rows()[i].clone())
                    .collect(),
            )
            .expect("a subset of valid pillar rows is valid")
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PillarChangeMode {
    Disabled,
    AssignedOnly,
    /// Include optional unassignment after assigned targets.
    All,
    UnassignOnly,
}
/// Settings for basic single-column subpillar neighborhoods. Swap selection
/// applies independently to the other group. None disables swaps. Changes are
/// emitted first; equal swap policies use unordered group pairs, while different
/// policies use both orientations. Overlapping successor states may occur.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SubPillarConfig {
    pub selection: SubPillarSelection,
    pub changes: PillarChangeMode,
    pub swap_selection: Option<SubPillarSelection>,
}
impl SubPillarConfig {
    pub fn new(selection: SubPillarSelection) -> Self {
        Self {
            selection,
            changes: PillarChangeMode::All,
            swap_selection: Some(selection),
        }
    }
}

/// Lazily enumerate changes and swaps of selected subsets of unpinned equal-value
/// basic slots. Every target must fit every selected member's domain; None requires
/// optional members. Pins and no-ops are excluded. Consecutive selection follows
/// variable index order here, or the configured variable order during search.
/// An attached PillarSchema selects whole entity rows: keys use all selected
/// columns, changes update one column at a time, and swaps emit both orientations.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 3])?;
/// let a = p.initial_assignment();
/// let config = SubPillarConfig::new(SubPillarSelection::combinations(2, 2)?);
/// assert_eq!(subpillar_moves(&p, &a, config)?.count(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn subpillar_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: SubPillarConfig,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    Ok(ordered(p, a, config, 0..p.variables().len()))
}
fn write<'a, V: Clone>(
    members: &'a [usize],
    value: Option<&'a V>,
) -> impl Iterator<Item = Move<V>> + 'a {
    members.iter().map(move |&variable| Move::Change {
        variable,
        value: value.cloned(),
    })
}
fn flat_ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: SubPillarConfig,
    order: impl Iterator<Item = usize>,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    let groups = pillar::groups(p, a, order);
    let changes = {
        let groups = groups.clone();
        (0..if config.changes == PillarChangeMode::Disabled {
            0
        } else {
            groups.len()
        })
            .flat_map(move |g| {
                let groups = groups.clone();
                config
                    .selection
                    .indices(groups[g].len())
                    .flat_map(move |indices| {
                        let members: Vec<_> = indices.into_iter().map(|i| groups[g][i]).collect();
                        let first = members[0];
                        let assigned = matches!(
                            config.changes,
                            PillarChangeMode::AssignedOnly | PillarChangeMode::All
                        );
                        let unassign = matches!(
                            config.changes,
                            PillarChangeMode::UnassignOnly | PillarChangeMode::All
                        );
                        crate::basic_value_order::targets(p, first, values, assigned, unassign)
                            .filter(move |v| *v != a.values()[first].as_ref())
                            .filter_map(move |v| {
                                if !pillar::accepts(p, &members, v) {
                                    return None;
                                }
                                Some(Move::Composite(write(&members, v).collect()))
                            })
                    })
            })
    };
    let swaps = config
        .swap_selection
        .into_iter()
        .flat_map(move |right_selection| {
            let groups = groups.clone();
            (0..groups.len()).flat_map(move |l| {
                let groups = groups.clone();
                let sizes = groups.clone();
                (0..groups.len())
                    .filter(move |&r| {
                        if sizes[l].len() < config.selection.minimum()
                            || sizes[r].len() < right_selection.minimum()
                        {
                            return false;
                        }
                        if config.selection == right_selection {
                            r > l
                        } else {
                            r != l
                        }
                    })
                    .flat_map(move |r| {
                        let groups = groups.clone();
                        config
                            .selection
                            .indices(groups[l].len())
                            .flat_map(move |indices| {
                                let left: Arc<Vec<_>> =
                                    Arc::new(indices.into_iter().map(|i| groups[l][i]).collect());
                                let groups = groups.clone();
                                right_selection.indices(groups[r].len()).filter_map(
                                    move |indices| {
                                        let right: Vec<_> =
                                            indices.into_iter().map(|i| groups[r][i]).collect();
                                        let lv = a.values()[left[0]].as_ref();
                                        let rv = a.values()[right[0]].as_ref();
                                        if !pillar::accepts(p, &left, rv)
                                            || !pillar::accepts(p, &right, lv)
                                        {
                                            return None;
                                        }
                                        Some(Move::Composite(
                                            write(&left, rv).chain(write(&right, lv)).collect(),
                                        ))
                                    },
                                )
                            })
                    })
            })
        });
    changes.chain(swaps)
}

pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: SubPillarConfig,
    order: impl Iterator<Item = usize>,
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    ordered_with_values(p, a, config, order, &[])
}
pub(crate) fn ordered_with_values<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: SubPillarConfig,
    order: impl Iterator<Item = usize>,
    values: &'a [crate::BasicValueOrder],
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    if p.pillar_schema().is_some() {
        Box::new(crate::pillar_schema::ordered_with_schema(
            p,
            a,
            crate::pillar_schema::Settings {
                left: Some(config.selection),
                right: config.swap_selection,
                changes: config.changes,
                swaps: config.swap_selection.is_some(),
            },
            order,
            p.pillar_schema(),
            values,
        ))
    } else {
        Box::new(flat_ordered(p, a, config, order, values))
    }
}
