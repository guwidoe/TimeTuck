//! Exchange value labels across selected basic-variable slots.
use crate::{Assignment, FiniteProblem, ModelError, Move, MoveError, SubPillarSelection};
use std::{collections::HashSet, error::Error, fmt, sync::Arc};

/// Select windows/subsets in the supplied variable order, then exchange each
/// unordered pair of distinct values. Bounds apply to selected slots, including
/// pinned and unchanged slots, rather than the number of writes in a move.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValueExchangeConfig {
    pub selection: SubPillarSelection,
    /// Treat unassignment as another value label. Assigning None is allowed only
    /// on optional variables; initializing a required None slot remains allowed.
    pub include_unassigned: bool,
}
impl ValueExchangeConfig {
    pub fn new(selection: SubPillarSelection) -> Self {
        Self {
            selection,
            include_unassigned: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueExchangeError {
    Model(ModelError),
    RepeatedVariable { variable: usize },
}
impl fmt::Display for ValueExchangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::RepeatedVariable { variable } => {
                write!(
                    f,
                    "variable {variable} appears more than once in exchange order"
                )
            }
        }
    }
}
impl Error for ValueExchangeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(e) => Some(e),
            Self::RepeatedVariable { .. } => None,
        }
    }
}
impl From<ModelError> for ValueExchangeError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

/// Exchange labels in model order. Domain values are deduplicated in encounter
/// order, including values not currently assigned anywhere. Empty/no-op exchanges
/// are omitted; one write is a plain Change and multiple writes are Composite.
/// Each candidate is atomic: an affected pin or disallowed destination rejects
/// the entire exchange. Unrelated pinned slots stay in the selection order.
/// Overlapping selections can yield duplicate moves; no global deduplication or
/// candidate list is allocated. Values are cloned only when a move is yielded.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![
///     Variable::new(vec![0,1,2]).with_initial(0),
///     Variable::new(vec![0,1,2]).with_initial(1),
/// ])?;
/// let a = p.initial_assignment();
/// let config = ValueExchangeConfig::new(SubPillarSelection::consecutive(2,2)?);
/// let moves: Vec<_> = value_exchange_moves(&p, &a, config)?.collect();
/// assert!(moves.contains(&Move::Change { variable: 0, value: Some(2) }));
/// assert_eq!(moves.len(), 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn value_exchange_moves<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    config: ValueExchangeConfig,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ValueExchangeError> {
    value_exchange_moves_in_order(problem, assignment, 0..problem.variables().len(), config)
}

/// Supply an ordered subset of variable IDs, for example entities sorted by code.
/// IDs must be in range and unique. Empty orders are valid and produce no moves.
/// Consecutive selection uses exactly this order, without dropping pinned slots.
/// Values come from all model domains; individual receiving domains still apply.
/// The returned cursor borrows the immutable assignment. Reopen it after a commit
/// using the existing MoveProvider extension API to see the new assignment.
pub fn value_exchange_moves_in_order<'a, V: Eq + Clone>(
    problem: &'a FiniteProblem<V>,
    assignment: &'a Assignment<V>,
    order: impl IntoIterator<Item = usize>,
    config: ValueExchangeConfig,
) -> Result<impl Iterator<Item = Move<V>> + 'a, ValueExchangeError> {
    problem.validate(assignment)?;
    let order: Vec<_> = order.into_iter().collect();
    let mut seen = HashSet::new();
    for &variable in &order {
        if variable >= problem.variables().len() {
            return Err(ModelError::Move(MoveError {
                variable,
                variable_count: problem.variables().len(),
            })
            .into());
        }
        if !seen.insert(variable) {
            return Err(ValueExchangeError::RepeatedVariable { variable });
        }
    }
    let mut values = Vec::new();
    for variable in problem.variables() {
        for value in variable.domain() {
            if !values.contains(&Some(value)) {
                values.push(Some(value));
            }
        }
    }
    if config.include_unassigned {
        values.push(None);
    }
    let values = Arc::new(values);
    Ok(config
        .selection
        .indices(order.len())
        .flat_map(move |indices| {
            let members: Arc<Vec<_>> = Arc::new(indices.into_iter().map(|i| order[i]).collect());
            let values = values.clone();
            (0..values.len()).flat_map(move |left| {
                let members = members.clone();
                let values = values.clone();
                (left + 1..values.len()).filter_map(move |right| {
                    let (a, b) = (values[left], values[right]);
                    let target = |slot: usize| {
                        let current = assignment.values()[slot].as_ref();
                        if current == a {
                            Some(b)
                        } else if current == b {
                            Some(a)
                        } else {
                            None
                        }
                    };
                    // Validate before cloning any values, including late rejection.
                    if members.iter().any(|&slot| {
                        target(slot).is_some_and(|destination| {
                            let variable = &problem.variables()[slot];
                            variable.is_pinned()
                                || match destination {
                                    Some(v) => !variable.domain().contains(v),
                                    None => !variable.allows_unassigned(),
                                }
                        })
                    }) {
                        return None;
                    }
                    let mut writes: Vec<_> = members
                        .iter()
                        .filter_map(|&variable| {
                            target(variable).map(|value| Move::Change {
                                variable,
                                value: value.cloned(),
                            })
                        })
                        .collect();
                    match writes.len() {
                        0 => None,
                        1 => writes.pop(),
                        _ => Some(Move::Composite(writes)),
                    }
                })
            })
        }))
}
