//! Optional identity-bearing selections bound to existing move/grouping paths.
use crate::{
    Assignment, FiniteProblem, ListMove, ListPosition, ListRange, ModelError, Pillar, PillarError,
    PillarSchema, PillarSelectionPolicy, PlanningVariableKind, PlanningVariableRef,
};
use std::{collections::BTreeSet, error::Error, fmt, rc::Rc};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionIdentityError {
    SpanOverflow,
    UnknownEntity,
    EmptyVariables,
    InvalidVariables,
    MissingSlot,
    Pillar(PillarError),
    Model(ModelError),
}
impl fmt::Display for SelectionIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selection identity: {self:?}")
    }
}
impl Error for SelectionIdentityError {}

/// Entity identity plus start and length. Construction checks endpoint overflow,
/// without asserting list bounds or doability; geometry can outlive a snapshot.
/// Binding resolves the application entity key to the actual list index before
/// producing an ordinary executable sublist move. No planning values are copied.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SubListSpan<E> {
    entity: E,
    start: usize,
    length: usize,
}
impl<E> SubListSpan<E> {
    pub fn new(entity: E, start: usize, length: usize) -> Result<Self, SelectionIdentityError> {
        start
            .checked_add(length)
            .ok_or(SelectionIdentityError::SpanOverflow)?;
        Ok(Self {
            entity,
            start,
            length,
        })
    }
    pub fn entity(&self) -> &E {
        &self.entity
    }
    pub fn start(&self) -> usize {
        self.start
    }
    pub fn length(&self) -> usize {
        self.length
    }
    pub fn end(&self) -> usize {
        self.start + self.length
    }
    pub fn resolve(
        &self,
        entity_index: impl FnOnce(&E) -> Option<usize>,
    ) -> Result<ListRange, SelectionIdentityError> {
        Ok(ListRange {
            entity: entity_index(&self.entity).ok_or(SelectionIdentityError::UnknownEntity)?,
            start: self.start,
            end: self.end(),
        })
    }
    pub fn change_move<V>(
        &self,
        entity_index: impl FnOnce(&E) -> Option<usize>,
        destination: ListPosition,
        reverse: bool,
    ) -> Result<ListMove<V>, SelectionIdentityError> {
        let range = self.resolve(entity_index)?;
        Ok(ListMove::SubListChange {
            entity: range.entity,
            start: range.start,
            end: range.end,
            destination,
            reverse,
        })
    }
}
impl<E: fmt::Display> fmt::Display for SubListSpan<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}..{}]", self.entity, self.start, self.end())
    }
}

/// Shared live entity source. Clones preserve source identity; a new reader has
/// distinct identity even if it happens to produce equal entity rows today.
#[derive(Clone)]
pub struct PillarEntitySource<E>(Rc<dyn Fn() -> Vec<E>>);
impl<E> PillarEntitySource<E> {
    pub fn new(reader: impl Fn() -> Vec<E> + 'static) -> Self {
        Self(Rc::new(reader))
    }
}
impl<E> PartialEq for PillarEntitySource<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<E> Eq for PillarEntitySource<E> {}
impl<E> fmt::Debug for PillarEntitySource<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PillarEntitySource(..)")
    }
}
/// Shared live filter. Equality records callback identity, never cached decisions.
/// External captured facts may change; every preparation invokes it afresh.
#[derive(Clone)]
pub struct PillarEntityFilter<E>(Rc<dyn Fn(&E) -> bool>);
impl<E> PillarEntityFilter<E> {
    pub fn new(filter: impl Fn(&E) -> bool + 'static) -> Self {
        Self(Rc::new(filter))
    }
}
impl<E> PartialEq for PillarEntityFilter<E> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<E> Eq for PillarEntityFilter<E> {}
impl<E> fmt::Debug for PillarEntityFilter<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PillarEntityFilter(..)")
    }
}

/// Reusable demand key over actual shared source/filter handles, canonical basic
/// field identities in column order, and the existing subpillar policy. Repeated
/// wrappers using cloned source/filter handles and copied metadata compare equal.
/// Preparation reads current sources and filters, binds each registered field to
/// the caller's native entity slot, then delegates existing PillarSchema grouping
/// and PillarSelectionPolicy selection. No predicate result/group cache is retained.
/// Optional metadata work leaves ordinary search/grouping paths unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PillarSelectionDemand<'s, E> {
    source: PillarEntitySource<E>,
    filter: PillarEntityFilter<E>,
    variables: Vec<PlanningVariableRef<'s>>,
    policy: PillarSelectionPolicy,
}
impl<'s, E: Ord + Clone> PillarSelectionDemand<'s, E> {
    pub fn new(
        source: PillarEntitySource<E>,
        filter: PillarEntityFilter<E>,
        variables: Vec<PlanningVariableRef<'s>>,
        policy: PillarSelectionPolicy,
    ) -> Result<Self, SelectionIdentityError> {
        let first = variables
            .first()
            .ok_or(SelectionIdentityError::EmptyVariables)?;
        if variables.iter().enumerate().any(|(i, v)| {
            !matches!(v.kind(), PlanningVariableKind::Basic { .. })
                || v.entity() != first.entity()
                || variables[..i].contains(v)
        }) {
            return Err(SelectionIdentityError::InvalidVariables);
        }
        Ok(Self {
            source,
            filter,
            variables,
            policy,
        })
    }
    pub fn variables(&self) -> &[PlanningVariableRef<'s>] {
        &self.variables
    }
    pub fn policy(&self) -> &PillarSelectionPolicy {
        &self.policy
    }
    pub fn prepare<V: Eq>(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
        mut slot: impl FnMut(&E, PlanningVariableRef<'s>) -> Option<usize>,
    ) -> Result<Vec<Pillar>, SelectionIdentityError> {
        problem
            .validate(assignment)
            .map_err(SelectionIdentityError::Model)?;
        let mut seen = BTreeSet::new();
        let mut rows = vec![];
        for entity in (self.source.0)() {
            if !seen.insert(entity.clone()) || !(self.filter.0)(&entity) {
                continue;
            }
            rows.push(
                self.variables
                    .iter()
                    .map(|&variable| {
                        slot(&entity, variable).ok_or(SelectionIdentityError::MissingSlot)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        if rows.is_empty() {
            return Ok(vec![]);
        }
        let schema = PillarSchema::new(rows).map_err(SelectionIdentityError::Pillar)?;
        let groups = schema
            .groups(problem, assignment)
            .map_err(SelectionIdentityError::Model)?;
        Ok(groups
            .iter()
            .flat_map(|group| self.policy.select(group))
            .collect())
    }
}
