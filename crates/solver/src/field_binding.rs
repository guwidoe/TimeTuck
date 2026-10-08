//! Typed schema fields bound to native model slots for custom move programs.
use crate::*;
use std::{collections::BTreeSet, error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldBindingError {
    WrongKind,
    WrongValueType {
        expected: PlanningType,
        actual: PlanningType,
    },
    UnassignedPolicy,
    DuplicateSlot(usize),
    SlotOutOfBounds {
        slot: usize,
        count: usize,
    },
    RowOutOfBounds {
        row: usize,
        count: usize,
    },
    AssignmentSize {
        expected: usize,
        actual: usize,
    },
    IndexOutOfBounds {
        row: usize,
        index: usize,
        len: usize,
    },
}
impl fmt::Display for FieldBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "planning field binding: {self:?}")
    }
}
impl Error for FieldBindingError {}
fn check_slots(slots: &[usize], count: usize) -> Result<(), FieldBindingError> {
    let mut seen = BTreeSet::new();
    for &slot in slots {
        if slot >= count {
            return Err(FieldBindingError::SlotOutOfBounds { slot, count });
        }
        if !seen.insert(slot) {
            return Err(FieldBindingError::DuplicateSlot(slot));
        }
    }
    Ok(())
}
fn check_type<V: 'static>(field: PlanningVariableRef<'_>) -> Result<(), FieldBindingError> {
    let actual = PlanningType::of::<V>();
    let expected = field.value_type();
    if actual != expected {
        return Err(FieldBindingError::WrongValueType { expected, actual });
    }
    Ok(())
}
fn slot(slots: &[usize], row: usize) -> Result<usize, FieldBindingError> {
    slots
        .get(row)
        .copied()
        .ok_or(FieldBindingError::RowOutOfBounds {
            row,
            count: slots.len(),
        })
}
fn check_size(expected: usize, actual: usize) -> Result<(), FieldBindingError> {
    if expected != actual {
        Err(FieldBindingError::AssignmentSize { expected, actual })
    } else {
        Ok(())
    }
}
impl SchemaVariableSlots<'_> {
    /// Bind one genuine basic field. Checks metadata type, assignment policy,
    /// bounds and duplicate slots once; the returned handle may outlive the schema.
    pub fn bind_basic<'p, V: Eq + Clone + 'static>(
        &self,
        problem: &'p FiniteProblem<V>,
    ) -> Result<BasicFieldBinding<'p, V>, FieldBindingError> {
        let PlanningVariableKind::Basic { allows_unassigned } = self.variable.kind() else {
            return Err(FieldBindingError::WrongKind);
        };
        check_type::<V>(self.variable)?;
        check_slots(&self.slots, problem.variables().len())?;
        if self
            .slots
            .iter()
            .any(|&s| problem.variables()[s].allows_unassigned() != allows_unassigned)
        {
            return Err(FieldBindingError::UnassignedPolicy);
        }
        Ok(BasicFieldBinding {
            problem,
            name: self.variable.name().into(),
            slots: self.slots.clone(),
        })
    }
    /// Bind one genuine list field, preserving entity order and per-entity ranges.
    pub fn bind_list<'p, V: Eq + Clone + 'static>(
        &self,
        problem: &'p ListProblem<V>,
    ) -> Result<ListFieldBinding<'p, V>, FieldBindingError> {
        let PlanningVariableKind::List { allows_unassigned } = self.variable.kind() else {
            return Err(FieldBindingError::WrongKind);
        };
        check_type::<V>(self.variable)?;
        check_slots(&self.slots, problem.entities().len())?;
        if problem.allows_unassigned() != allows_unassigned {
            return Err(FieldBindingError::UnassignedPolicy);
        }
        Ok(ListFieldBinding {
            problem,
            name: self.variable.name().into(),
            slots: self.slots.clone(),
        })
    }
}
/// A borrowed basic domain including `None` exactly when the variable permits it.
/// Values are borrowed, so iteration and membership never clone payloads.
#[derive(Debug)]
pub struct BasicFieldRange<'a, V> {
    values: &'a [V],
    optional: bool,
}
impl<'a, V: Eq> ValueRange for BasicFieldRange<'a, V> {
    type Value = Option<&'a V>;
    fn len(&self) -> u128 {
        self.values.len() as u128 + u128::from(self.optional)
    }
    fn get(&self, index: u128) -> Option<Self::Value> {
        if index < self.values.len() as u128 {
            Some(Some(&self.values[index as usize]))
        } else if self.optional && index == self.values.len() as u128 {
            Some(None)
        } else {
            None
        }
    }
    fn contains(&self, value: &Self::Value) -> bool {
        match value {
            Some(v) => self.values.contains(v),
            None => self.optional,
        }
    }
}
/// An entity-row mapping to one typed basic field in an immutable native problem.
/// Read from valid working assignments of this problem. Reads check shape, not
/// every domain/pin invariant. Constructed moves must be applied through the bound
/// problem's director; ordinary validation, scoring and undo happen there.
///
/// ```
/// use timetuck::*;
/// struct Solution; struct Task;
/// let schema=PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Task>(
///     vec![PlanningVariableDefinition::basic::<i32>("worker",false)])])?;
/// let p=FiniteProblem::new(vec![Variable::new(vec![1,2]).with_initial(1)])?;
/// let binding=SchemaVariableSlots {variable:schema.genuine_entity::<Task>()?.genuine_variable("worker")?,slots:vec![0]};
/// let field=binding.bind_basic(&p)?;
/// let mut a=p.initial_assignment();
/// p.apply(&mut a,&field.change(0,Some(2))?)?.commit();
/// assert_eq!(field.read(&a,0)?,Some(&2));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct BasicFieldBinding<'p, V> {
    problem: &'p FiniteProblem<V>,
    name: String,
    slots: Vec<usize>,
}
impl<V: Eq + Clone> BasicFieldBinding<'_, V> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn slots(&self) -> &[usize] {
        &self.slots
    }
    pub fn read<'a>(
        &self,
        a: &'a Assignment<V>,
        row: usize,
    ) -> Result<Option<&'a V>, FieldBindingError> {
        check_size(self.problem.variables().len(), a.values().len())?;
        Ok(a.values()[slot(&self.slots, row)?].as_ref())
    }
    pub fn range(&self, row: usize) -> Result<BasicFieldRange<'_, V>, FieldBindingError> {
        let v = &self.problem.variables()[slot(&self.slots, row)?];
        Ok(BasicFieldRange {
            values: v.domain(),
            optional: v.allows_unassigned(),
        })
    }
    pub fn is_value_in_range(
        &self,
        row: usize,
        value: Option<&V>,
    ) -> Result<bool, FieldBindingError> {
        Ok(self.range(row)?.contains(&value))
    }
    pub fn is_pinned(&self, row: usize) -> Result<bool, FieldBindingError> {
        Ok(self.problem.variables()[slot(&self.slots, row)?].is_pinned())
    }
    pub fn change(&self, row: usize, value: Option<V>) -> Result<Move<V>, FieldBindingError> {
        Ok(Move::Change {
            variable: slot(&self.slots, row)?,
            value,
        })
    }
    pub fn swap(&self, left: usize, right: usize) -> Result<Move<V>, FieldBindingError> {
        Ok(Move::Swap {
            left: slot(&self.slots, left)?,
            right: slot(&self.slots, right)?,
        })
    }
}
/// A position relative to the rows of one bound list field, not native slot IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldListPosition {
    pub row: usize,
    pub index: usize,
}
/// List field access in entity-row order. As with BasicFieldBinding, reads require
/// a valid assignment of the bound problem; move construction maps positions only.
/// Index/range/pin legality is validated atomically when the director applies it.
/// Reuse with each current assignment; this handle never caches mutable contents.
#[derive(Debug)]
pub struct ListFieldBinding<'p, V> {
    problem: &'p ListProblem<V>,
    name: String,
    slots: Vec<usize>,
}
impl<V: Eq + Clone> ListFieldBinding<'_, V> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn slots(&self) -> &[usize] {
        &self.slots
    }
    pub fn read<'a>(
        &self,
        a: &'a ListAssignment<V>,
        row: usize,
    ) -> Result<&'a [V], FieldBindingError> {
        check_size(self.problem.entities().len(), a.lists().len())?;
        Ok(&a.lists()[slot(&self.slots, row)?])
    }
    pub fn value_at<'a>(
        &self,
        a: &'a ListAssignment<V>,
        position: FieldListPosition,
    ) -> Result<&'a V, FieldBindingError> {
        let list = self.read(a, position.row)?;
        list.get(position.index)
            .ok_or(FieldBindingError::IndexOutOfBounds {
                row: position.row,
                index: position.index,
                len: list.len(),
            })
    }
    /// Return only locations belonging to this field. Unassigned and other-field
    /// values return None. The returned entity row respects the binding order.
    pub fn position_of(
        &self,
        a: &ListAssignment<V>,
        value: &V,
    ) -> Result<Option<FieldListPosition>, FieldBindingError> {
        check_size(self.problem.entities().len(), a.lists().len())?;
        Ok(a.location(value).and_then(|p| {
            self.slots
                .iter()
                .position(|&s| s == p.entity)
                .map(|row| FieldListPosition {
                    row,
                    index: p.index,
                })
        }))
    }
    pub fn range(&self, row: usize) -> Result<&[V], FieldBindingError> {
        Ok(self.problem.entities()[slot(&self.slots, row)?]
            .value_range()
            .unwrap_or(self.problem.values()))
    }
    pub fn is_value_in_range(&self, row: usize, value: &V) -> Result<bool, FieldBindingError> {
        Ok(self.range(row)?.contains(value))
    }
    pub fn is_pinned(&self, row: usize) -> Result<bool, FieldBindingError> {
        Ok(self.problem.entities()[slot(&self.slots, row)?].is_pinned())
    }
    pub fn first_unpinned_index(
        &self,
        a: &ListAssignment<V>,
        row: usize,
    ) -> Result<usize, FieldBindingError> {
        let list = self.read(a, row)?;
        let meta = &self.problem.entities()[slot(&self.slots, row)?];
        Ok(if meta.is_pinned() {
            list.len()
        } else {
            meta.pinned_prefix()
        })
    }
    fn position(&self, p: FieldListPosition) -> Result<ListPosition, FieldBindingError> {
        Ok(ListPosition {
            entity: slot(&self.slots, p.row)?,
            index: p.index,
        })
    }
    pub fn assign(
        &self,
        value: V,
        destination: FieldListPosition,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::Assign {
            value,
            destination: self.position(destination)?,
        })
    }
    pub fn relocate(
        &self,
        source: FieldListPosition,
        destination: FieldListPosition,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::Change {
            source: self.position(source)?,
            destination: self.position(destination)?,
        })
    }
    pub fn swap(
        &self,
        left: FieldListPosition,
        right: FieldListPosition,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::Swap {
            left: self.position(left)?,
            right: self.position(right)?,
        })
    }
    /// Destination index is measured after source removal, as for ListMove.
    pub fn relocate_span(
        &self,
        row: usize,
        span: std::ops::Range<usize>,
        destination: FieldListPosition,
        reverse: bool,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::SubListChange {
            entity: slot(&self.slots, row)?,
            start: span.start,
            end: span.end,
            destination: self.position(destination)?,
            reverse,
        })
    }
    pub fn swap_spans(
        &self,
        left_row: usize,
        left: std::ops::Range<usize>,
        right_row: usize,
        right: std::ops::Range<usize>,
        reverse: bool,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::SubListSwap {
            left: ListRange {
                entity: slot(&self.slots, left_row)?,
                start: left.start,
                end: left.end,
            },
            right: ListRange {
                entity: slot(&self.slots, right_row)?,
                start: right.start,
                end: right.end,
            },
            reverse,
        })
    }
    pub fn unassign(
        &self,
        row: usize,
        span: std::ops::Range<usize>,
    ) -> Result<ListMove<V>, FieldBindingError> {
        Ok(ListMove::Unassign {
            entity: slot(&self.slots, row)?,
            start: span.start,
            end: span.end,
        })
    }
}
