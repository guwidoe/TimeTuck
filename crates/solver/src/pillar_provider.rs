//! Comparator-aware pillar neighborhoods regenerated at each search step.
use crate::{
    Assignment, FiniteProblem, ListMove, MixedAssignment, MixedMove, MixedProblem, ModelError,
    Move, MoveProvider, PillarChangeMode, PillarSelectionPolicy, pillar_groups,
};
use std::sync::Arc;

/// Lazy changes and swaps of uniform entity groups. Groups are rebuilt from the
/// current assignment whenever candidates are opened; no grouping survives a
/// solver step. An attached PillarSchema selects aligned columns, otherwise every
/// basic variable is one row. A pin in any selected column excludes that row.
///
/// Changes use `left` and alter one column at a time, in domain order, followed
/// by optional null when requested. Swaps apply `left` and `right` independently
/// and exchange every selected column. Both orientations of distinct groups are
/// emitted, including equivalent successors. All destination ranges are checked.
/// Selection is finite and lazy after grouping; no collection of all subsets or
/// candidate moves is constructed. Comparators see row slot IDs, not value clones.
///
/// As a MoveProvider, invalid input assignments panic; solver entry points validate
/// assignments first. Use `select` directly for a checked standalone iterator.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0); 4])?;
/// let provider = PillarMoveProvider {
///     left: PillarSelectionPolicy::sequential(2, 2,
///         Some(PillarComparator::new(|a,b| a[0].cmp(&b[0]))))?,
///     changes: PillarChangeMode::AssignedOnly,
///     swaps: false,
///     ..Default::default()
/// };
/// let a = p.initial_assignment();
/// assert_eq!(provider.select(&p, &a)?.count(), 3);
/// // Pass this same provider to basic or mixed local search; each step regroups.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PillarMoveProvider {
    pub left: PillarSelectionPolicy,
    pub right: PillarSelectionPolicy,
    pub changes: PillarChangeMode,
    pub swaps: bool,
}
impl Default for PillarMoveProvider {
    fn default() -> Self {
        Self {
            left: PillarSelectionPolicy::whole(),
            right: PillarSelectionPolicy::whole(),
            changes: PillarChangeMode::All,
            swaps: true,
        }
    }
}
impl PillarMoveProvider {
    /// Open candidates against the current snapshot. Only metadata is copied at
    /// opening; values are cloned when a candidate is yielded.
    pub fn select<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
    ) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
        Ok(self.select_groups(p, a, pillar_groups(p, a)?))
    }

    pub(crate) fn select_groups<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        groups: Vec<crate::Pillar>,
    ) -> impl Iterator<Item = Move<V>> + 'a {
        let groups = Arc::new(groups);
        let changes = {
            let groups = groups.clone();
            (0..if self.changes == PillarChangeMode::Disabled {
                0
            } else {
                groups.len()
            })
                .flat_map(move |g| self.left.select(&groups[g]))
                .flat_map(move |pillar| {
                    let pillar = Arc::new(pillar);
                    (0..pillar.column_count()).flat_map(move |column| {
                        let pillar = pillar.clone();
                        let first = pillar.rows()[0][column];
                        let assigned = matches!(
                            self.changes,
                            PillarChangeMode::All | PillarChangeMode::AssignedOnly
                        );
                        let unassign = matches!(
                            self.changes,
                            PillarChangeMode::All | PillarChangeMode::UnassignOnly
                        );
                        crate::basic_value_order::targets(p, first, &[], assigned, unassign)
                            .filter(move |v| *v != a.values()[first].as_ref())
                            .filter_map(move |v| {
                                if !pillar
                                    .rows()
                                    .iter()
                                    .all(|row| crate::pillar_schema::accepts(p, row[column], v))
                                {
                                    return None;
                                }
                                Some(Move::Composite(
                                    pillar
                                        .rows()
                                        .iter()
                                        .map(|row| Move::Change {
                                            variable: row[column],
                                            value: v.cloned(),
                                        })
                                        .collect(),
                                ))
                            })
                    })
                })
        };
        let swaps = (0..if self.swaps { groups.len() } else { 0 }).flat_map(move |l| {
            let groups = groups.clone();
            let sizes = groups.clone();
            (0..groups.len())
                .filter(move |&r| {
                    r != l
                        && sizes[l].rows().len() >= self.left.sizes().map_or(1, |s| s.0)
                        && sizes[r].rows().len() >= self.right.sizes().map_or(1, |s| s.0)
                })
                .flat_map(move |r| {
                    let groups = groups.clone();
                    self.left.select(&groups[l]).flat_map(move |left| {
                        self.right.select(&groups[r]).filter_map(move |right| {
                            let valid = |to: &crate::Pillar, from: &crate::Pillar| {
                                to.rows().iter().all(|row| {
                                    row.iter().zip(&from.rows()[0]).all(|(&to, &from)| {
                                        crate::pillar_schema::accepts(
                                            p,
                                            to,
                                            a.values()[from].as_ref(),
                                        )
                                    })
                                })
                            };
                            if !valid(&left, &right) || !valid(&right, &left) {
                                return None;
                            }
                            Some(left.swap_move(&right, a).expect("disjoint uniform groups"))
                        })
                    })
                })
        });
        changes.chain(swaps)
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>> for PillarMoveProvider {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        Box::new(
            self.select(p, a)
                .expect("pillar provider requires a valid assignment"),
        )
    }
}
impl<B: Eq + Clone, L: Eq + Clone>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for PillarMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        p.validate(a)
            .expect("pillar provider requires a valid mixed assignment");
        Box::new(
            self.select(p.basic(), a.basic())
                .expect("validated basic assignment")
                .map(|basic| MixedMove {
                    basic,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
