//! Explicit entity rows for multicolumn pillar neighborhoods.
use crate::{
    Assignment, FiniteProblem, ModelError, Move, MoveError, Pillar, PillarChangeMode, PillarError,
    SubPillarSelection,
};
use std::sync::Arc;

/// Nonempty, disjoint rows of aligned selected variable slots. Only selected
/// columns participate in grouping or moves. A pin in any selected slot excludes
/// that entire row. Unselected variables retain their ordinary model semantics.
/// Rows are canonicalized; search encounter order follows the first selected slot
/// encountered in its configured variable order. Columns retain caller order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PillarSchema {
    rows: Arc<Vec<Vec<usize>>>,
}
impl PillarSchema {
    pub fn new(rows: Vec<Vec<usize>>) -> Result<Self, PillarError> {
        let pillar = Pillar::from_rows(rows)?;
        Ok(Self {
            rows: Arc::new(pillar.rows().to_vec()),
        })
    }
    pub fn rows(&self) -> &[Vec<usize>] {
        &self.rows
    }
    /// Group this schema's rows against a current assignment, independently of
    /// any schema attached to the problem. No planning values are cloned.
    pub fn groups<V: Eq>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
    ) -> Result<Vec<Pillar>, ModelError> {
        if let Some(&variable) = self
            .rows
            .iter()
            .flatten()
            .find(|&&v| v >= p.variables().len())
        {
            return Err(crate::MoveError {
                variable,
                variable_count: p.variables().len(),
            }
            .into());
        }
        p.validate(a)?;
        Ok(groups(p, a, &self.rows, 0..p.variables().len())
            .iter()
            .map(|g| {
                Pillar::from_rows(g.iter().map(|&i| self.rows[i].clone()).collect())
                    .expect("nonempty disjoint schema rows")
            })
            .collect())
    }
    pub(crate) fn validate(&self, variable_count: usize) -> Result<(), PillarError> {
        for &variable in self.rows.iter().flatten() {
            if variable >= variable_count {
                return Err(PillarError::Move(MoveError {
                    variable,
                    variable_count,
                }));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Settings {
    pub left: Option<SubPillarSelection>,
    pub right: Option<SubPillarSelection>,
    pub changes: PillarChangeMode,
    pub swaps: bool,
}
fn selections(
    n: usize,
    policy: Option<SubPillarSelection>,
) -> Box<dyn Iterator<Item = Vec<usize>>> {
    match policy {
        Some(p) => Box::new(p.indices(n)),
        None => Box::new(std::iter::once((0..n).collect())),
    }
}
pub(crate) fn accepts<V: Eq>(p: &FiniteProblem<V>, variable: usize, value: Option<&V>) -> bool {
    match value {
        Some(v) => p.variables()[variable].contains_value(v),
        None => p.variables()[variable].allows_unassigned(),
    }
}
fn groups<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    rows: &[Vec<usize>],
    order: impl Iterator<Item = usize>,
) -> Arc<Vec<Vec<usize>>> {
    let mut row_of = vec![None; p.variables().len()];
    for (row, slots) in rows.iter().enumerate() {
        if slots.iter().any(|&v| p.variables()[v].is_pinned()) {
            continue;
        }
        for &v in slots {
            row_of[v] = Some(row);
        }
    }
    let mut seen = vec![false; rows.len()];
    let mut groups: Vec<Vec<usize>> = vec![];
    for slot in order {
        let Some(row) = row_of[slot] else { continue };
        if std::mem::replace(&mut seen[row], true) {
            continue;
        }
        if let Some(group) = groups.iter_mut().find(|g| {
            rows[g[0]]
                .iter()
                .zip(&rows[row])
                .all(|(&x, &y)| a.values()[x] == a.values()[y])
        }) {
            group.push(row);
        } else {
            groups.push(vec![row]);
        }
    }
    Arc::new(groups)
}
/// Enumerate both orientations of every legal whole-pillar swap, with no change
/// candidates. An attached schema groups aligned rows by their entire selected
/// value tuple; without a schema each basic slot is a single-column row.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![
///     Variable::new(vec![0,1]).with_initial(0),
///     Variable::new(vec![0,1]).with_initial(1),
///     Variable::new(vec![0,1]).with_initial(1),
///     Variable::new(vec![0,1]).with_initial(0),
/// ])?.with_pillar_schema(PillarSchema::new(vec![vec![0,1],vec![2,3]])?)?;
/// let a = p.initial_assignment();
/// assert_eq!(pillar_swap_moves(&p,&a)?.count(),2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn pillar_swap_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    Ok(ordered(
        p,
        a,
        Settings {
            left: None,
            right: None,
            changes: PillarChangeMode::Disabled,
            swaps: true,
        },
        0..p.variables().len(),
    ))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: Settings,
    order: impl Iterator<Item = usize>,
) -> impl Iterator<Item = Move<V>> + 'a {
    ordered_with_schema(p, a, config, order, p.pillar_schema(), &[])
}
pub(crate) fn ordered_with_schema<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    config: Settings,
    order: impl Iterator<Item = usize>,
    schema: Option<&'a PillarSchema>,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    let rows = schema
        .map(|s| s.rows.clone())
        .unwrap_or_else(|| Arc::new((0..p.variables().len()).map(|i| vec![i]).collect()));
    let groups = groups(p, a, &rows, order);
    let changes = {
        let rows = rows.clone();
        let groups = groups.clone();
        (0..if config.changes == PillarChangeMode::Disabled {
            0
        } else {
            groups.len()
        })
            .flat_map(move |g| {
                let groups = groups.clone();
                let rows = rows.clone();
                selections(groups[g].len(), config.left).flat_map(move |indices| {
                    let members: Arc<Vec<_>> =
                        Arc::new(indices.into_iter().map(|i| groups[g][i]).collect());
                    let rows = rows.clone();
                    (0..rows[members[0]].len()).flat_map(move |column| {
                        let members = members.clone();
                        let rows = rows.clone();
                        let first = rows[members[0]][column];
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
                                if !members.iter().all(|&r| accepts(p, rows[r][column], v)) {
                                    return None;
                                }
                                Some(Move::Composite(
                                    members
                                        .iter()
                                        .map(|&r| Move::Change {
                                            variable: rows[r][column],
                                            value: v.cloned(),
                                        })
                                        .collect(),
                                ))
                            })
                    })
                })
            })
    };
    let swaps = (0..if config.swaps { groups.len() } else { 0 }).flat_map(move |l| {
        let groups = groups.clone();
        let rows = rows.clone();
        let sizes = groups.clone();
        (0..groups.len())
            .filter(move |&r| {
                r != l
                    && sizes[l].len() >= config.left.map_or(1, |s| s.minimum())
                    && sizes[r].len() >= config.right.map_or(1, |s| s.minimum())
            })
            .flat_map(move |r| {
                let groups = groups.clone();
                let rows = rows.clone();
                selections(groups[l].len(), config.left).flat_map(move |indices| {
                    let left: Arc<Vec<_>> =
                        Arc::new(indices.into_iter().map(|i| groups[l][i]).collect());
                    let groups = groups.clone();
                    let rows = rows.clone();
                    selections(groups[r].len(), config.right).filter_map(move |indices| {
                        let right: Vec<_> = indices.into_iter().map(|i| groups[r][i]).collect();
                        let valid = |members: &[usize], source: usize| {
                            members.iter().all(|&row| {
                                rows[row]
                                    .iter()
                                    .zip(&rows[source])
                                    .all(|(&to, &from)| accepts(p, to, a.values()[from].as_ref()))
                            })
                        };
                        if !valid(&left, right[0]) || !valid(&right, left[0]) {
                            return None;
                        }
                        let mut parts = vec![];
                        for (members, source) in [(&left[..], right[0]), (&right[..], left[0])] {
                            for &row in members {
                                for (&variable, &from) in rows[row].iter().zip(&rows[source]) {
                                    parts.push(Move::Change {
                                        variable,
                                        value: a.values()[from].clone(),
                                    });
                                }
                            }
                        }
                        Some(Move::Composite(parts))
                    })
                })
            })
    });
    changes.chain(swaps)
}

/// Group unpinned entity rows by equal selected value tuples in the current
/// assignment. Uses the attached schema or one row per basic variable. Groups
/// follow their first variable's encounter order; rows within a Pillar are canonical.
/// No planning values are cloned. Call again after changing the assignment.
pub fn pillar_groups<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
) -> Result<Vec<Pillar>, ModelError> {
    p.validate(a)?;
    let rows = p
        .pillar_schema()
        .map(|s| s.rows.clone())
        .unwrap_or_else(|| Arc::new((0..p.variables().len()).map(|i| vec![i]).collect()));
    Ok(groups(p, a, &rows, 0..p.variables().len())
        .iter()
        .map(|g| {
            Pillar::from_rows(g.iter().map(|&i| rows[i].clone()).collect())
                .expect("nonempty disjoint schema rows")
        })
        .collect())
}
