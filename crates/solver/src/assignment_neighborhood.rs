//! Optional single-slot and whole-pillar assignment policies.
use crate::{Assignment, FiniteProblem, ModelError, Move, PillarChangeMode};

/// Whole-pillar changes of at least two members, with explicit null crossing.
/// None follows each slot's optionality. False excludes both null sources and
/// null destinations. Explicit true requires every model slot to be optional.
/// An attached PillarSchema groups rows by their selected value tuples and changes
/// one selected column at a time; singleton groups are excluded by member count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PillarChangeConfig {
    crossing_null: Option<bool>,
}
impl PillarChangeConfig {
    pub fn with_crossing_null(mut self, enabled: bool) -> Self {
        self.crossing_null = Some(enabled);
        self
    }
    pub fn crossing_null(self) -> Option<bool> {
        self.crossing_null
    }
    pub(crate) fn validate<V: Eq>(self, p: &FiniteProblem<V>) -> Result<(), ModelError> {
        if self.crossing_null == Some(true) {
            crate::mass_assignment::validate_unassignment(p)?;
        }
        Ok(())
    }
}
/// Assign each missing optional slot to every value in its own domain. Already
/// assigned and pinned slots are skipped. All model slots must be optional.
pub fn assign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    crate::mass_assignment::validate_unassignment(p)?;
    Ok(single(p, a, false, 0..p.variables().len()))
}
/// Clear each assigned unpinned optional slot once. Missing slots are skipped.
/// All model slots must be optional, including pinned slots.
pub fn unassign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    crate::mass_assignment::validate_unassignment(p)?;
    Ok(single(p, a, true, 0..p.variables().len()))
}
pub(crate) fn single<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    unassign: bool,
    order: impl Iterator<Item = usize> + 'a,
) -> impl Iterator<Item = Move<V>> + 'a {
    single_with_values(p, a, unassign, order, &[])
}
pub(crate) fn single_with_values<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    unassign: bool,
    order: impl Iterator<Item = usize> + 'a,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    order
        .filter(move |&i| !p.variables()[i].is_pinned() && a.values()[i].is_some() == unassign)
        .flat_map(move |i| {
            crate::basic_value_order::targets(p, i, values, !unassign, unassign).map(move |v| {
                Move::Change {
                    variable: i,
                    value: v.cloned(),
                }
            })
        })
}
/// Change complete equal-value groups, excluding singleton pillars. Destinations
/// must be legal for every member and different from the group's current value.
/// Required missing slots never participate; optional null pillars participate
/// unless null crossing is disabled. Pins never join a movable group.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0);2])?;
/// let a=p.initial_assignment();
/// let moves:Vec<_>=pillar_change_moves(&p,&a,PillarChangeConfig::default())?.collect();
/// assert_eq!(moves.len(),1);
/// assert_eq!(moves[0].touched_variables(),vec![0,1]);
/// # Ok::<(),ModelError>(())
/// ```
pub fn pillar_change_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    c: PillarChangeConfig,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    c.validate(p)?;
    Ok(pillars(p, a, c, false, 0..p.variables().len()))
}
/// Unassign whole assigned pillars, including singleton groups. Requires every
/// model slot to be optional. Missing and pinned members never participate.
pub fn pillar_unassign_moves<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
    p.validate(a)?;
    crate::mass_assignment::validate_unassignment(p)?;
    Ok(pillars(
        p,
        a,
        PillarChangeConfig::default(),
        true,
        0..p.variables().len(),
    ))
}
pub(crate) fn pillars<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    c: PillarChangeConfig,
    unassign: bool,
    order: impl Iterator<Item = usize>,
) -> impl Iterator<Item = Move<V>> + 'a {
    pillars_with_values(p, a, c, unassign, order, &[])
}
pub(crate) fn pillars_with_values<'a, V: Eq + Clone>(
    p: &'a FiniteProblem<V>,
    a: &'a Assignment<V>,
    c: PillarChangeConfig,
    unassign: bool,
    order: impl Iterator<Item = usize>,
    values: &'a [crate::BasicValueOrder],
) -> impl Iterator<Item = Move<V>> + 'a {
    // Reuse tuple grouping and destination intersection, including attached schemas.
    // Unassignment never scans non-null destinations.
    let changes = if unassign {
        PillarChangeMode::UnassignOnly
    } else if c.crossing_null == Some(false) {
        PillarChangeMode::AssignedOnly
    } else {
        PillarChangeMode::All
    };
    crate::pillar_schema::ordered_with_schema(
        p,
        a,
        crate::pillar_schema::Settings {
            left: None,
            right: None,
            changes,
            swaps: false,
        },
        order,
        p.pillar_schema(),
        values,
    )
    .filter(move |movement| {
        let Move::Composite(steps) = movement else {
            unreachable!("pillar generator emits flat composites")
        };
        (unassign || steps.len() >= 2)
            && steps.iter().all(|step| {
                let Move::Change { variable, .. } = step else {
                    unreachable!("pillar changes contain only writes")
                };
                a.values()[*variable].is_some()
                    || (!unassign
                        && c.crossing_null != Some(false)
                        && p.variables()[*variable].allows_unassigned())
            })
    })
}
