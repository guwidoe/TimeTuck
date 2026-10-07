//! Identity rebasing for snapshot-bound basic, list and mixed moves.
use crate::{ListMove, ListPosition, ListRange, MixedMove, Move};
use std::{collections::BTreeMap, error::Error, fmt};

/// Independent identity spaces within a mixed model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveRebaseNamespace {
    BasicVariable,
    BasicValue,
    ListEntity,
    ListValue,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoveRebaseError<E> {
    Lookup(E),
    /// Two different source identities mapped to one destination identity.
    Collision(MoveRebaseNamespace),
}
impl<E: fmt::Debug> fmt::Display for MoveRebaseError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "move rebase: {self:?}")
    }
}
impl<E: fmt::Debug> Error for MoveRebaseError<E> {}

struct IndexMap<F> {
    lookup: F,
    mapped: BTreeMap<usize, usize>,
    inverse: BTreeMap<usize, usize>,
    namespace: MoveRebaseNamespace,
}
impl<F> IndexMap<F> {
    fn new(lookup: F, namespace: MoveRebaseNamespace) -> Self {
        Self {
            lookup,
            mapped: BTreeMap::new(),
            inverse: BTreeMap::new(),
            namespace,
        }
    }
    fn get<E>(&mut self, source: usize) -> Result<usize, MoveRebaseError<E>>
    where
        F: FnMut(usize) -> Result<usize, E>,
    {
        if let Some(&value) = self.mapped.get(&source) {
            return Ok(value);
        }
        let value = (self.lookup)(source).map_err(MoveRebaseError::Lookup)?;
        if self.inverse.contains_key(&value) {
            return Err(MoveRebaseError::Collision(self.namespace));
        }
        self.mapped.insert(source, value);
        self.inverse.insert(value, source);
        Ok(value)
    }
    fn position<E>(&mut self, p: ListPosition) -> Result<ListPosition, MoveRebaseError<E>>
    where
        F: FnMut(usize) -> Result<usize, E>,
    {
        Ok(ListPosition {
            entity: self.get(p.entity)?,
            index: p.index,
        })
    }
    fn range<E>(&mut self, r: ListRange) -> Result<ListRange, MoveRebaseError<E>>
    where
        F: FnMut(usize) -> Result<usize, E>,
    {
        Ok(ListRange {
            entity: self.get(r.entity)?,
            start: r.start,
            end: r.end,
        })
    }
}
struct ValueMap<'a, V, U, F> {
    lookup: F,
    mapped: Vec<(&'a V, U)>,
    namespace: MoveRebaseNamespace,
}
impl<'a, V: Eq, U: Eq + Clone, F> ValueMap<'a, V, U, F> {
    fn new(lookup: F, namespace: MoveRebaseNamespace) -> Self {
        Self {
            lookup,
            mapped: vec![],
            namespace,
        }
    }
    fn get<E>(&mut self, source: &'a V) -> Result<U, MoveRebaseError<E>>
    where
        F: FnMut(&V) -> Result<U, E>,
    {
        if let Some((_, value)) = self.mapped.iter().find(|(key, _)| *key == source) {
            return Ok(value.clone());
        }
        let value = (self.lookup)(source).map_err(MoveRebaseError::Lookup)?;
        if self.mapped.iter().any(|(_, mapped)| *mapped == value) {
            return Err(MoveRebaseError::Collision(self.namespace));
        }
        self.mapped.push((source, value.clone()));
        Ok(value)
    }
}
// Preserve composition shape/order, including empty nodes, without recursive
// traversal. Ordinary Move/ListMove destruction retains their existing behavior.
fn map_tree<'a, S: 'a, T, E>(
    root: &'a S,
    children: impl Fn(&'a S) -> Option<&'a [S]>,
    mut leaf: impl FnMut(&'a S) -> Result<T, E>,
    compose: impl Fn(Vec<T>) -> T,
) -> Result<T, E> {
    let mut pending = vec![(root, false)];
    let mut output = vec![];
    while let Some((node, closing)) = pending.pop() {
        if let Some(steps) = children(node) {
            if closing {
                let parts = output.split_off(output.len() - steps.len());
                output.push(compose(parts));
            } else {
                pending.push((node, true));
                pending.extend(steps.iter().rev().map(|step| (step, false)));
            }
        } else {
            output.push(leaf(node)?);
        }
    }
    Ok(output.pop().expect("one mapped root"))
}

/// Map basic variable indices and non-null values into a destination model.
/// Each distinct referenced variable/value is looked up once in execution order;
/// source value identity is Eq, so use stable IDs or identity-aware handles.
/// Distinct source identities must remain distinct within each namespace. None
/// bypasses value lookup. Nested composition shape and child order are preserved.
///
/// The input and any assignments are untouched. This maps metadata, not model
/// validity: apply the result through the destination problem to validate domains
/// and pins. Source and destination must represent corresponding assignment states.
/// Callback side effects are not rolled back after errors or panics.
///
/// ```
/// use timetuck::*;
/// let movement=Move::Change {variable:0,value:Some("a")};
/// let rebased=rebase_basic_move(&movement, |i| Ok::<_,()>(i+2), |v| Ok(v.to_uppercase()))?;
/// assert_eq!(rebased,Move::Change {variable:2,value:Some("A".to_string())});
/// # Ok::<(),MoveRebaseError<()>>(())
/// ```
pub fn rebase_basic_move<V: Eq, U: Eq + Clone, E>(
    movement: &Move<V>,
    variable_lookup: impl FnMut(usize) -> Result<usize, E>,
    value_lookup: impl FnMut(&V) -> Result<U, E>,
) -> Result<Move<U>, MoveRebaseError<E>> {
    let mut variables = IndexMap::new(variable_lookup, MoveRebaseNamespace::BasicVariable);
    let mut values = ValueMap::new(value_lookup, MoveRebaseNamespace::BasicValue);
    map_tree(
        movement,
        |m| {
            if let Move::Composite(steps) = m {
                Some(steps)
            } else {
                None
            }
        },
        |m| {
            Ok(match m {
                Move::Change { variable, value } => Move::Change {
                    variable: variables.get(*variable)?,
                    value: value.as_ref().map(|v| values.get(v)).transpose()?,
                },
                Move::Swap { left, right } => Move::Swap {
                    left: variables.get(*left)?,
                    right: variables.get(*right)?,
                },
                Move::Composite(_) => unreachable!("composition handled by traversal"),
            })
        },
        Move::Composite,
    )
}

/// Rebase list owners and assigned values with the same identity/collision rules
/// as rebase_basic_move. All indices, span boundaries and reversal flags are kept:
/// each destination list must correspond position-for-position to the source list
/// at this move's opening snapshot. This cannot rebase onto a differently ordered
/// list. Child-relative indices remain relative to earlier composite edits.
/// Only Assign leaves look up values; other leaves identify values by position.
pub fn rebase_list_move<V: Eq, U: Eq + Clone, E>(
    movement: &ListMove<V>,
    entity_lookup: impl FnMut(usize) -> Result<usize, E>,
    value_lookup: impl FnMut(&V) -> Result<U, E>,
) -> Result<ListMove<U>, MoveRebaseError<E>> {
    let mut entities = IndexMap::new(entity_lookup, MoveRebaseNamespace::ListEntity);
    let mut values = ValueMap::new(value_lookup, MoveRebaseNamespace::ListValue);
    map_tree(
        movement,
        |m| {
            if let ListMove::Composite(steps) = m {
                Some(steps)
            } else {
                None
            }
        },
        |m| {
            Ok(match m {
                ListMove::Assign { value, destination } => ListMove::Assign {
                    value: values.get(value)?,
                    destination: entities.position(*destination)?,
                },
                ListMove::Change {
                    source,
                    destination,
                } => ListMove::Change {
                    source: entities.position(*source)?,
                    destination: entities.position(*destination)?,
                },
                ListMove::SubListChange {
                    entity,
                    start,
                    end,
                    destination,
                    reverse,
                } => ListMove::SubListChange {
                    entity: entities.get(*entity)?,
                    start: *start,
                    end: *end,
                    destination: entities.position(*destination)?,
                    reverse: *reverse,
                },
                ListMove::SubListSwap {
                    left,
                    right,
                    reverse,
                } => ListMove::SubListSwap {
                    left: entities.range(*left)?,
                    right: entities.range(*right)?,
                    reverse: *reverse,
                },
                ListMove::Swap { left, right } => ListMove::Swap {
                    left: entities.position(*left)?,
                    right: entities.position(*right)?,
                },
                ListMove::Unassign { entity, start, end } => ListMove::Unassign {
                    entity: entities.get(*entity)?,
                    start: *start,
                    end: *end,
                },
                ListMove::Composite(_) => unreachable!("composition handled by traversal"),
            })
        },
        ListMove::Composite,
    )
}

/// Rebase both components, visiting basic before list. The four namespaces have
/// independent lookup caches and collision checks, even if their value types or
/// numeric IDs coincide. Returns no partially rebased move on failure.
pub fn rebase_mixed_move<B: Eq, L: Eq, U: Eq + Clone, W: Eq + Clone, E>(
    movement: &MixedMove<B, L>,
    variable_lookup: impl FnMut(usize) -> Result<usize, E>,
    basic_value_lookup: impl FnMut(&B) -> Result<U, E>,
    entity_lookup: impl FnMut(usize) -> Result<usize, E>,
    list_value_lookup: impl FnMut(&L) -> Result<W, E>,
) -> Result<MixedMove<U, W>, MoveRebaseError<E>> {
    Ok(MixedMove {
        basic: rebase_basic_move(&movement.basic, variable_lookup, basic_value_lookup)?,
        lists: rebase_list_move(&movement.lists, entity_lookup, list_value_lookup)?,
    })
}
