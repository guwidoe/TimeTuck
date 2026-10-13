//! Readable primitive diagnostics and retained post-execution inspection.
use crate::{ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListMoveDescriptionError {
    List(ListError),
    InvalidSpan(ListRange),
    Composite,
}
impl fmt::Display for ListMoveDescriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list move description: {self:?}")
    }
}
impl Error for ListMoveDescriptionError {}
impl From<ListError> for ListMoveDescriptionError {
    fn from(e: ListError) -> Self {
        Self::List(e)
    }
}
fn value_at<V>(a: &ListAssignment<V>, p: ListPosition) -> Result<&V, ListError> {
    a.lists()
        .get(p.entity)
        .ok_or(ListError::EntityIndex {
            entity: p.entity,
            count: a.lists().len(),
        })?
        .get(p.index)
        .ok_or(ListError::ElementIndex(p))
}
fn position_label(p: ListPosition, label: &mut impl FnMut(usize) -> String) -> String {
    format!("{}[{}]", label(p.entity), p.index)
}
fn ordered_spans(left: ListRange, right: ListRange) -> (ListRange, ListRange) {
    if left.entity == right.entity && left.start > right.start {
        (right, left)
    } else {
        (left, right)
    }
}
/// Format one primitive with application-supplied entity/value labels. No Eq,
/// Clone, scoring, pin/range checks or callbacks on the solver are required.
/// Assignment/change/swap/single-unassign descriptions read only needed values.
/// Destinations and sublist endpoints are metadata: they may intentionally refer
/// beyond current rows. Formatting is not a doability check. Composite coordinates
/// may refer to intermediate states; describe their prepared stages separately.
pub fn describe_list_move<V>(
    movement: &ListMove<V>,
    current: &ListAssignment<V>,
    mut entity_label: impl FnMut(usize) -> String,
    mut value_label: impl FnMut(&V) -> String,
) -> Result<String, ListMoveDescriptionError> {
    Ok(match movement {
        ListMove::Assign { value, destination } => format!(
            "{} {{null -> {}}}",
            value_label(value),
            position_label(*destination, &mut entity_label)
        ),
        ListMove::Change {
            source,
            destination,
        } => format!(
            "{} {{{} -> {}}}",
            value_label(value_at(current, *source)?),
            position_label(*source, &mut entity_label),
            position_label(*destination, &mut entity_label)
        ),
        ListMove::Swap { left, right } => format!(
            "{} {{{}}} <-> {} {{{}}}",
            value_label(value_at(current, *left)?),
            position_label(*left, &mut entity_label),
            value_label(value_at(current, *right)?),
            position_label(*right, &mut entity_label)
        ),
        ListMove::Unassign { entity, start, end } => {
            let r = ListRange {
                entity: *entity,
                start: *start,
                end: *end,
            };
            if end.checked_sub(*start) != Some(1) {
                return Err(ListMoveDescriptionError::InvalidSpan(r));
            }
            let p = ListPosition {
                entity: *entity,
                index: *start,
            };
            format!(
                "{} {{{} -> null}}",
                value_label(value_at(current, p)?),
                position_label(p, &mut entity_label)
            )
        }
        ListMove::SubListChange {
            entity,
            start,
            end,
            destination,
            reverse,
        } => {
            let length = end
                .checked_sub(*start)
                .ok_or(ListMoveDescriptionError::InvalidSpan(ListRange {
                    entity: *entity,
                    start: *start,
                    end: *end,
                }))?;
            let from = entity_label(*entity);
            let arrow = if *reverse { "-reversing->" } else { "->" };
            format!(
                "|{length}| {{{from}[{start}..{end}] {arrow} {}}}",
                position_label(*destination, &mut entity_label)
            )
        }
        ListMove::SubListSwap {
            left,
            right,
            reverse,
        } => {
            let (left, right) = ordered_spans(*left, *right);
            for r in [left, right] {
                if r.end < r.start {
                    return Err(ListMoveDescriptionError::InvalidSpan(r));
                }
            }
            let l = entity_label(left.entity);
            let r = entity_label(right.entity);
            let arrow = if *reverse { "<-reversing->" } else { "<->" };
            format!(
                "{{{l}[{}..{}]}} {arrow} {{{r}[{}..{}]}}",
                left.start, left.end, right.start, right.end
            )
        }
        ListMove::Composite(_) => return Err(ListMoveDescriptionError::Composite),
    })
}

/// Validated move record with stable moved-value metadata across execution.
/// Same-owner sublist swaps normalize lower spans first. Equality compares only
/// movement coordinates/payload, not captured snapshots. Single swaps intentionally
/// inspect the current values at their original positions; other moves retain the
/// prepared values. This post-execution inspection contract is distinct from the
/// solver's existing pre-move planning_values API and changes no tabu policy.
#[derive(Clone, Debug)]
pub struct RecordedListMove<V> {
    movement: ListMove<V>,
    entities: Vec<usize>,
    values: Vec<V>,
}
impl<V: PartialEq> PartialEq for RecordedListMove<V> {
    fn eq(&self, other: &Self) -> bool {
        self.movement == other.movement
    }
}
impl<V: Eq> Eq for RecordedListMove<V> {}
impl<V: Eq + Clone> RecordedListMove<V> {
    pub fn prepare(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        mut movement: ListMove<V>,
    ) -> Result<Self, ListError> {
        let mut pending = vec![&mut movement];
        while let Some(step) = pending.pop() {
            match step {
                ListMove::SubListSwap { left, right, .. } => {
                    (*left, *right) = ordered_spans(*left, *right)
                }
                ListMove::Composite(children) => pending.extend(children.iter_mut()),
                _ => {}
            }
        }
        let values = problem.planning_values(before, &movement)?;
        let entities = movement.touched_entities();
        Ok(Self {
            movement,
            entities,
            values,
        })
    }
}
impl<V> RecordedListMove<V> {
    /// Describe a prepared change using its retained value, including after its
    /// source row has become empty. Other primitives use their usual metadata.
    pub fn describe(
        &self,
        current: &ListAssignment<V>,
        mut entity_label: impl FnMut(usize) -> String,
        mut value_label: impl FnMut(&V) -> String,
    ) -> Result<String, ListMoveDescriptionError> {
        if let ListMove::Change {
            source,
            destination,
        } = self.movement
        {
            return Ok(format!(
                "{} {{{} -> {}}}",
                value_label(&self.values[0]),
                position_label(source, &mut entity_label),
                position_label(destination, &mut entity_label)
            ));
        }
        describe_list_move(&self.movement, current, entity_label, value_label)
    }
    pub fn movement(&self) -> &ListMove<V> {
        &self.movement
    }
    pub fn planning_entities(&self) -> &[usize] {
        &self.entities
    }
    pub fn planning_values<'a>(
        &'a self,
        current: &'a ListAssignment<V>,
    ) -> Result<Vec<&'a V>, ListError> {
        if let ListMove::Swap { left, right } = self.movement {
            Ok(vec![value_at(current, left)?, value_at(current, right)?])
        } else {
            Ok(self.values.iter().collect())
        }
    }
}
