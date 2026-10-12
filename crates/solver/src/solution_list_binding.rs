//! Bind concrete list owner/value entities to a native model and real objects.
use crate::{
    BoundSolutionFacts, ComposedPlanningSolution, ListEntity, ListError, ListPinPolicy,
    ListProblem, ManagedRangeError, PlanningType, PlanningVariableKind, SolutionBindingError,
    SolutionFieldRange, SolutionMemberKind, SolutionRange,
};
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
#[derive(Debug)]
pub enum ListSolutionBindingError {
    Binding(SolutionBindingError),
    Range(ManagedRangeError),
    Model(ListError),
    ListFieldCount,
    ConflictingRecord(PlanningType),
}
impl std::fmt::Display for ListSolutionBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "list solution binding: {self:?}")
    }
}
impl std::error::Error for ListSolutionBindingError {}
impl From<SolutionBindingError> for ListSolutionBindingError {
    fn from(e: SolutionBindingError) -> Self {
        Self::Binding(e)
    }
}
pub struct SolutionListEntityRecord<V> {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    fields: Vec<(String, Vec<V>)>,
    pins: ListPinPolicy,
}
impl<V> SolutionListEntityRecord<V> {
    pub fn new<E: Any + Send + Sync>(object: Arc<E>, fields: Vec<(String, Vec<V>)>) -> Self {
        Self {
            object,
            entity_type: PlanningType::of::<E>(),
            fields,
            pins: ListPinPolicy::default(),
        }
    }
    pub fn with_pin_policy(mut self, pins: ListPinPolicy) -> Self {
        self.pins = pins;
        self
    }
}
pub struct SolutionListCollection<V> {
    pub member: String,
    pub entities: Vec<SolutionListEntityRecord<V>>,
}
pub struct BoundListEntity {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    list_index: Option<usize>,
}
impl BoundListEntity {
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn object_ref<E: Any>(&self) -> Option<&E> {
        self.object.downcast_ref()
    }
    pub fn entity_type(&self) -> PlanningType {
        self.entity_type
    }
    pub fn list_index(&self) -> Option<usize> {
        self.list_index
    }
}
/// Objects retain immutable fact/initial-state payloads; the native model and
/// its explicit assignment own current planning lists. Shadow-only value objects
/// have no list index. No reflective getters or inverse fields are required.
pub struct BoundListSolution<V> {
    problem: ListProblem<V>,
    entities: Vec<BoundListEntity>,
    collections: HashMap<String, Vec<usize>>,
    facts: BoundSolutionFacts,
    list_owner: PlanningType,
    list_variable: String,
}
impl<V> BoundListSolution<V> {
    pub fn problem(&self) -> &ListProblem<V> {
        &self.problem
    }
    pub fn entities(&self) -> &[BoundListEntity] {
        &self.entities
    }
    pub fn facts(&self) -> &BoundSolutionFacts {
        &self.facts
    }
    pub fn collection(&self, name: &str) -> Option<&[usize]> {
        self.collections.get(name).map(Vec::as_slice)
    }
    pub fn list_owner(&self) -> PlanningType {
        self.list_owner
    }
    pub fn list_variable(&self) -> &str {
        &self.list_variable
    }
}
impl<V: Clone + Eq + std::hash::Hash> BoundListSolution<V> {
    /// Opt into the native immutable registry/range indexes without changing
    /// concrete objects, provider order, optionality or pin baselines.
    pub fn indexed(mut self) -> Result<Self, ListSolutionBindingError> {
        let mut problem = ListProblem::new_indexed(
            self.problem.values().to_vec(),
            self.problem.entities().to_vec(),
        )
        .map_err(ListSolutionBindingError::Model)?;
        if self.problem.allows_unassigned() {
            problem = problem.allowing_unassigned();
        }
        self.problem = problem;
        Ok(self)
    }
}
impl ComposedPlanningSolution {
    /// Bind one genuine list field and any shadow-only entities. Mixed/basic or
    /// multiple-list-field layouts need their separate native model bindings.
    /// All concrete reads, pins, aliases and declarations validate before return.
    pub fn bind_list_with_facts<V: Clone + Eq + 'static>(
        &self,
        collections: Vec<SolutionListCollection<V>>,
        ranges: Vec<SolutionRange<V>>,
        field_ranges: Vec<SolutionFieldRange>,
        facts: BoundSolutionFacts,
    ) -> Result<BoundListSolution<V>, ListSolutionBindingError> {
        if !std::ptr::eq(self, facts.descriptor().as_ref()) {
            return Err(SolutionBindingError::DifferentFactDescriptor.into());
        }
        let descriptors = self.compile_variable_ranges(field_ranges)?;
        let mut genuine = descriptors.iter();
        let descriptor = genuine
            .next()
            .ok_or(ListSolutionBindingError::ListFieldCount)?;
        if genuine.next().is_some()
            || !matches!(
                descriptor.variable().kind(),
                PlanningVariableKind::List { .. }
            )
        {
            return Err(ListSolutionBindingError::ListFieldCount);
        }
        let field = descriptor.definition();
        let list_owner = field.entity_type;
        let list_variable = field.variable.clone();
        // Validate all provider reads even with an empty owner collection.
        let mut names = HashSet::new();
        for read in &ranges {
            let member = self
                .member(&read.member)
                .ok_or_else(|| SolutionBindingError::UnknownMember(read.member.clone()))?;
            if !matches!(member.kind(), SolutionMemberKind::ValueRange { .. }) {
                return Err(SolutionBindingError::WrongMemberKind(read.member.clone()).into());
            }
            if !names.insert(read.member.clone()) {
                return Err(SolutionBindingError::DuplicateBinding(read.member.clone()).into());
            }
        }
        let mut records: Vec<SolutionListEntityRecord<V>> = vec![];
        let mut by_object = HashMap::new();
        let mut member_rows = HashMap::new();
        for collection in collections {
            let member = self
                .member(&collection.member)
                .ok_or_else(|| SolutionBindingError::UnknownMember(collection.member.clone()))?;
            let SolutionMemberKind::Entities {
                allowed_types,
                singleton,
            } = member.kind()
            else {
                return Err(SolutionBindingError::WrongMemberKind(collection.member).into());
            };
            if !names.insert(collection.member.clone()) {
                return Err(SolutionBindingError::DuplicateBinding(collection.member).into());
            }
            if *singleton && collection.entities.len() > 1 {
                return Err(SolutionBindingError::SingletonCardinality(collection.member).into());
            }
            let mut rows = vec![];
            for record in collection.entities {
                if !allowed_types.contains(&record.entity_type) {
                    return Err(SolutionBindingError::WrongEntityType {
                        member: collection.member,
                        entity: record.entity_type,
                    }
                    .into());
                }
                if record.entity_type == list_owner {
                    if record.fields.len() != 1 || record.fields[0].0 != list_variable {
                        return Err(SolutionBindingError::InvalidFields(record.entity_type).into());
                    }
                } else if !record.fields.is_empty() || record.pins != ListPinPolicy::default() {
                    return Err(SolutionBindingError::InvalidFields(record.entity_type).into());
                }
                let key = Arc::as_ptr(&record.object) as *const () as usize;
                let row = if let Some(&i) = by_object.get(&key) {
                    let previous: &SolutionListEntityRecord<V> = &records[i];
                    if previous.entity_type != record.entity_type
                        || previous.fields != record.fields
                        || previous.pins != record.pins
                    {
                        return Err(ListSolutionBindingError::ConflictingRecord(
                            record.entity_type,
                        ));
                    }
                    i
                } else {
                    let i = records.len();
                    by_object.insert(key, i);
                    records.push(record);
                    i
                };
                rows.push(row);
            }
            member_rows.insert(collection.member, rows);
        }
        for member in self.members() {
            if matches!(
                member.kind(),
                SolutionMemberKind::Entities { .. } | SolutionMemberKind::ValueRange { .. }
            ) && !names.contains(member.name())
            {
                return Err(SolutionBindingError::MissingBinding(member.name().into()).into());
            }
        }
        let mut list_entities = vec![];
        let mut entities = vec![];
        for record in records {
            let list_index = if record.entity_type == list_owner {
                let initial = record.fields.into_iter().next().unwrap().1;
                let index = list_entities.len();
                list_entities.push(
                    ListEntity::new(initial)
                        .with_pinned(record.pins.fully_pinned)
                        .with_pinned_prefix(record.pins.prefix),
                );
                Some(index)
            } else {
                None
            };
            entities.push(BoundListEntity {
                object: record.object,
                entity_type: record.entity_type,
                list_index,
            });
        }
        let managed = descriptor
            .bind_shared(&ranges, list_entities.len())
            .map_err(ListSolutionBindingError::Binding)?;
        let problem = managed
            .list_problem(list_entities)
            .map_err(ListSolutionBindingError::Range)?;
        Ok(BoundListSolution {
            problem,
            entities,
            collections: member_rows,
            facts,
            list_owner,
            list_variable,
        })
    }
}
