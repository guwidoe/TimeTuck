//! Atomic group moves over one or several aligned basic-variable columns.
use crate::{Assignment, FiniteProblem, ModelError, Move, MoveError};
use std::{collections::BTreeSet, error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PillarError {
    Empty,
    EmptyRow,
    Arity { expected: usize, actual: usize },
    DuplicateVariable { variable: usize },
    Overlap { variable: usize },
    NotUniform { variable: usize },
    Move(MoveError),
}
impl fmt::Display for PillarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a pillar needs at least one member"),
            Self::EmptyRow => {
                f.write_str("a pillar needs at least one selected variable per member")
            }
            Self::Arity { expected, actual } => {
                write!(f, "expected {expected} pillar columns, found {actual}")
            }
            Self::DuplicateVariable { variable } => {
                write!(f, "variable {variable} occurs twice in a pillar")
            }
            Self::Overlap { variable } => write!(f, "pillars overlap at variable {variable}"),
            Self::NotUniform { variable } => {
                write!(f, "variable {variable} differs from its pillar column")
            }
            Self::Move(e) => e.fmt(f),
        }
    }
}
impl Error for PillarError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Move(e) => Some(e),
            _ => None,
        }
    }
}
/// A nonempty group of members, each selecting the same number of aligned basic
/// variable slots. Members must have equal values in every selected column when
/// constructing a move. Other variables are untouched. Member order is canonical;
/// column order identifies the variables to exchange and remains significant.
///
/// Move construction snapshots all old values before any assignment mutation.
/// Returned ordinary Composite moves use existing transaction/score rollback.
/// They must be regenerated after the starting assignment changes. Raw construction
/// does not check domains or pins; use FiniteProblem::apply or a score director.
///
/// ```
/// use timetuck::*;
/// let mut a = Assignment::new(vec![Some(1), Some(1), Some(2)]);
/// let left = Pillar::new([0, 1])?;
/// let right = Pillar::new([2])?;
/// let movement = left.swap_move(&right, &a)?;
/// { let trial = a.apply(&movement)?;
///   assert_eq!(trial.values(), &[Some(2), Some(2), Some(1)]); }
/// assert_eq!(a.values(), &[Some(1), Some(1), Some(2)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Pillar {
    rows: Vec<Vec<usize>>,
}
impl Pillar {
    pub fn new(members: impl IntoIterator<Item = usize>) -> Result<Self, PillarError> {
        Self::from_rows(members.into_iter().map(|i| vec![i]).collect())
    }
    pub fn from_rows(mut rows: Vec<Vec<usize>>) -> Result<Self, PillarError> {
        let width = rows.first().ok_or(PillarError::Empty)?.len();
        if width == 0 {
            return Err(PillarError::EmptyRow);
        }
        let mut seen = BTreeSet::new();
        for row in &rows {
            if row.len() != width {
                return Err(PillarError::Arity {
                    expected: width,
                    actual: row.len(),
                });
            }
            for &variable in row {
                if !seen.insert(variable) {
                    return Err(PillarError::DuplicateVariable { variable });
                }
            }
        }
        rows.sort_unstable();
        Ok(Self { rows })
    }
    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
    pub fn column_count(&self) -> usize {
        self.rows[0].len()
    }
    pub fn variables(&self) -> Vec<usize> {
        let mut vars: Vec<_> = self.rows.iter().flatten().copied().collect();
        vars.sort_unstable();
        vars
    }
    fn signature<V: Eq + Clone>(&self, a: &Assignment<V>) -> Result<Vec<Option<V>>, PillarError> {
        let read = |variable| {
            a.get(variable).ok_or(PillarError::Move(MoveError {
                variable,
                variable_count: a.values().len(),
            }))
        };
        let first: Vec<_> = self.rows[0]
            .iter()
            .map(|&i| read(i))
            .collect::<Result<_, _>>()?;
        for row in &self.rows[1..] {
            for (&variable, expected) in row.iter().zip(&first) {
                if read(variable)? != *expected {
                    return Err(PillarError::NotUniform { variable });
                }
            }
        }
        Ok(first.into_iter().cloned().collect())
    }
    fn writes<'a, V: Clone + 'a>(
        &'a self,
        values: &'a [Option<V>],
    ) -> impl Iterator<Item = Move<V>> + 'a {
        self.rows.iter().flat_map(move |row| {
            row.iter()
                .copied()
                .zip(values.iter().cloned())
                .map(|(variable, value)| Move::Change { variable, value })
        })
    }
    pub fn change_move<V: Eq + Clone>(
        &self,
        a: &Assignment<V>,
        values: &[Option<V>],
    ) -> Result<Move<V>, PillarError> {
        if values.len() != self.column_count() {
            return Err(PillarError::Arity {
                expected: self.column_count(),
                actual: values.len(),
            });
        }
        self.signature(a)?;
        Ok(Move::Composite(self.writes(values).collect()))
    }
    pub fn swap_move<V: Eq + Clone>(
        &self,
        other: &Self,
        a: &Assignment<V>,
    ) -> Result<Move<V>, PillarError> {
        if self.column_count() != other.column_count() {
            return Err(PillarError::Arity {
                expected: self.column_count(),
                actual: other.column_count(),
            });
        }
        let left: BTreeSet<_> = self.rows.iter().flatten().copied().collect();
        if let Some(&variable) = other.rows.iter().flatten().find(|v| left.contains(v)) {
            return Err(PillarError::Overlap { variable });
        }
        let before_left = self.signature(a)?;
        let before_right = other.signature(a)?;
        Ok(Move::Composite(
            self.writes(&before_right)
                .chain(other.writes(&before_left))
                .collect(),
        ))
    }
}
pub(crate) fn accepts<V: Eq>(p: &FiniteProblem<V>, members: &[usize], value: Option<&V>) -> bool {
    members.iter().all(|&i| match value {
        Some(v) => p.variables()[i].contains_value(v),
        None => p.variables()[i].allows_unassigned(),
    })
}
/// Group all unpinned basic slots by their current value (including None), then
/// lazily enumerate whole-group changes and optional unordered group swaps.
/// Targets must fit every member's domain; unassignment requires all members to
/// allow it. Groups follow first-member encounter order; target values follow the
/// first member's domain, with None last. Pins never join a movable group.
/// With an attached PillarSchema, grouping uses selected value tuples, changes
/// update one selected column at a time, and swaps emit both group orientations.
/// Without a schema, each basic slot is its own single-column member.
pub fn pillar_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    swaps: bool,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    Ok(ordered(p, a, swaps, 0..p.variables().len()))
}
fn flat_ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    swaps: bool,
    order: impl Iterator<Item = usize>,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    let groups = groups(p, a, order);
    let changes = {
        let groups = groups.clone();
        (0..groups.len()).flat_map(move |g| {
            let groups = groups.clone();
            let first = groups[g][0];
            crate::basic_value_order::targets(p, first, values, true, true)
                .filter(move |v| *v != a.values()[first].as_ref())
                .filter_map(move |v| {
                    if !accepts(p, &groups[g], v) {
                        return None;
                    }
                    Some(Move::Composite(
                        groups[g]
                            .iter()
                            .map(|&variable| Move::Change {
                                variable,
                                value: v.cloned(),
                            })
                            .collect(),
                    ))
                })
        })
    };
    let exchanges = (0..if swaps { groups.len() } else { 0 }).flat_map(move |left| {
        let groups = groups.clone();
        ((left + 1)..groups.len()).filter_map(move |right| {
            let l = &groups[left];
            let r = &groups[right];
            let lv = a.values()[l[0]].as_ref();
            let rv = a.values()[r[0]].as_ref();
            (accepts(p, l, rv) && accepts(p, r, lv)).then(|| {
                Move::Composite(
                    l.iter()
                        .map(|&variable| Move::Change {
                            variable,
                            value: rv.cloned(),
                        })
                        .chain(r.iter().map(|&variable| Move::Change {
                            variable,
                            value: lv.cloned(),
                        }))
                        .collect(),
                )
            })
        })
    });
    changes.chain(exchanges)
}

pub(crate) fn groups<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    order: impl Iterator<Item = usize>,
) -> Arc<Vec<Vec<usize>>> {
    let mut groups: Vec<Vec<usize>> = vec![];
    for i in order.filter(|&i| !p.variables()[i].is_pinned()) {
        if let Some(g) = groups
            .iter_mut()
            .find(|g| a.values()[g[0]] == a.values()[i])
        {
            g.push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    Arc::new(groups)
}

pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    swaps: bool,
    order: impl Iterator<Item = usize>,
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    ordered_with_values(p, a, swaps, order, &[])
}
pub(crate) fn ordered_with_values<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    swaps: bool,
    order: impl Iterator<Item = usize>,
    values: &'a [crate::BasicValueOrder],
) -> Box<dyn Iterator<Item = Move<V>> + 'a> {
    if p.pillar_schema().is_some() {
        Box::new(crate::pillar_schema::ordered_with_schema(
            p,
            a,
            crate::pillar_schema::Settings {
                left: None,
                right: None,
                changes: crate::PillarChangeMode::All,
                swaps,
            },
            order,
            p.pillar_schema(),
            values,
        ))
    } else {
        Box::new(flat_ordered(p, a, swaps, order, values))
    }
}
