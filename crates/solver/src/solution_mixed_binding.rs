//! Bind real typed solution objects to native basic and list components together.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct SolutionMixedEntityRecord<B, L> {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    basic: Vec<(String, Option<B>)>,
    lists: Vec<(String, Vec<L>)>,
    basic_pins: Vec<String>,
    list_pins: ListPinPolicy,
}
impl<B, L> SolutionMixedEntityRecord<B, L> {
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn new<E: Any + Send + Sync>(
        object: Arc<E>,
        basic: Vec<(String, Option<B>)>,
        lists: Vec<(String, Vec<L>)>,
    ) -> Self {
        Self {
            object,
            entity_type: PlanningType::of::<E>(),
            basic,
            lists,
            basic_pins: vec![],
            list_pins: ListPinPolicy::default(),
        }
    }
    pub fn with_basic_pin(mut self, field: impl Into<String>) -> Self {
        self.basic_pins.push(field.into());
        self
    }
    pub fn with_list_pin_policy(mut self, pins: ListPinPolicy) -> Self {
        self.list_pins = pins;
        self
    }
}
pub struct SolutionMixedCollection<B, L> {
    pub member: String,
    pub entities: Vec<SolutionMixedEntityRecord<B, L>>,
}
pub struct BoundMixedEntity {
    object: Arc<dyn Any + Send + Sync>,
    entity_type: PlanningType,
    basic_slots: Vec<(String, usize)>,
    list_field: Option<(String, usize)>,
}
impl BoundMixedEntity {
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn object_ref<E: Any>(&self) -> Option<&E> {
        self.object.downcast_ref()
    }
    pub fn entity_type(&self) -> PlanningType {
        self.entity_type
    }
    pub fn basic_slots(&self) -> &[(String, usize)] {
        &self.basic_slots
    }
    pub fn list_field(&self) -> Option<(&str, usize)> {
        self.list_field.as_ref().map(|(n, i)| (n.as_str(), *i))
    }
}
pub struct BoundMixedSolution<B, L> {
    problem: MixedProblem<B, L>,
    entities: Vec<BoundMixedEntity>,
    collections: HashMap<String, Vec<usize>>,
    facts: BoundSolutionFacts,
}
impl<B, L> BoundMixedSolution<B, L> {
    pub fn problem(&self) -> &MixedProblem<B, L> {
        &self.problem
    }
    pub fn entities(&self) -> &[BoundMixedEntity] {
        &self.entities
    }
    pub fn collection(&self, name: &str) -> Option<&[usize]> {
        self.collections.get(name).map(Vec::as_slice)
    }
    pub fn facts(&self) -> &BoundSolutionFacts {
        &self.facts
    }
}
#[derive(Debug)]
pub enum MixedSolutionBindingError {
    Binding(SolutionBindingError),
    List(ListSolutionBindingError),
}
impl std::fmt::Display for MixedSolutionBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::List(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for MixedSolutionBindingError {}
impl From<SolutionBindingError> for MixedSolutionBindingError {
    fn from(e: SolutionBindingError) -> Self {
        Self::Binding(e)
    }
}
fn check_reads<V: 'static>(
    layout: &ComposedPlanningSolution,
    reads: &[SolutionRange<V>],
    names: &mut HashSet<String>,
) -> Result<(), SolutionBindingError> {
    for read in reads {
        let member = layout
            .member(&read.member)
            .ok_or_else(|| SolutionBindingError::UnknownMember(read.member.clone()))?;
        let SolutionMemberKind::ValueRange { value_type, .. } = member.kind() else {
            return Err(SolutionBindingError::WrongMemberKind(read.member.clone()));
        };
        if *value_type != PlanningType::of::<V>() {
            return Err(SolutionBindingError::WrongValueType {
                member: read.member.clone(),
                expected: *value_type,
                actual: PlanningType::of::<V>(),
            });
        }
        if !names.insert(read.member.clone()) {
            return Err(SolutionBindingError::DuplicateBinding(read.member.clone()));
        }
    }
    Ok(())
}
impl ComposedPlanningSolution {
    /// Bind basic and one genuine list declaration using the actual shared schema.
    /// One object may own both kinds. Repeated allocation aliases contribute one
    /// canonical row, while distinct equal objects remain independent entities.
    pub fn bind_mixed_with_facts<B: Clone + Eq + 'static, L: Clone + Eq + 'static>(
        &self,
        collections: Vec<SolutionMixedCollection<B, L>>,
        basic_reads: Vec<SolutionRange<B>>,
        list_reads: Vec<SolutionRange<L>>,
        fields: Vec<SolutionFieldRange>,
        facts: BoundSolutionFacts,
    ) -> Result<BoundMixedSolution<B, L>, MixedSolutionBindingError> {
        if !std::ptr::eq(self, facts.descriptor().as_ref()) {
            return Err(SolutionBindingError::DifferentFactDescriptor.into());
        }
        let descriptors = self.compile_variable_ranges(fields)?;
        let list_fields: Vec<_> = descriptors
            .iter()
            .filter(|d| matches!(d.variable().kind(), PlanningVariableKind::List { .. }))
            .collect();
        if list_fields.len() > 1 {
            return Err(MixedSolutionBindingError::List(
                ListSolutionBindingError::MultipleListFields(
                    list_fields
                        .iter()
                        .map(|d| (d.definition().entity_type, d.definition().variable.clone()))
                        .collect(),
                ),
            ));
        }
        let mut names = HashSet::new();
        check_reads(self, &basic_reads, &mut names)?;
        check_reads(self, &list_reads, &mut names)?;
        let mut records: Vec<SolutionMixedEntityRecord<B, L>> = vec![];
        let mut by_object = HashMap::new();
        let mut members = HashMap::new();
        let mut counts = HashMap::new();
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
            let mut indices = vec![];
            for mut record in collection.entities {
                let ty = record.entity_type;
                if !allowed_types.contains(&ty) {
                    return Err(SolutionBindingError::WrongEntityType {
                        member: collection.member,
                        entity: ty,
                    }
                    .into());
                }
                let schema = self.schema().entity_type(ty).unwrap();
                let mut basic: HashMap<_, _> = record.basic.iter().cloned().collect();
                let mut lists: HashMap<_, _> = record.lists.iter().cloned().collect();
                let pins: HashSet<_> = record.basic_pins.iter().cloned().collect();
                let invalid = || SolutionBindingError::InvalidFields(ty);
                if basic.len() != record.basic.len()
                    || lists.len() != record.lists.len()
                    || pins.len() != record.basic_pins.len()
                    || !pins.iter().all(|p| basic.contains_key(p))
                {
                    return Err(invalid().into());
                }
                record.basic.clear();
                record.lists.clear();
                for variable in schema.genuine_variables() {
                    let name = variable.name();
                    match variable.kind() {
                        PlanningVariableKind::Basic { .. } => record
                            .basic
                            .push((name.into(), basic.remove(name).ok_or_else(invalid)?)),
                        PlanningVariableKind::List { .. } => record
                            .lists
                            .push((name.into(), lists.remove(name).ok_or_else(invalid)?)),
                        PlanningVariableKind::Shadow => unreachable!("genuine variable"),
                    }
                }
                if !basic.is_empty()
                    || !lists.is_empty()
                    || (record.lists.is_empty() && record.list_pins != ListPinPolicy::default())
                {
                    return Err(invalid().into());
                }
                record.basic_pins.sort();
                let key = Arc::as_ptr(&record.object) as *const () as usize;
                let i = if let Some(&i) = by_object.get(&key) {
                    let old: &SolutionMixedEntityRecord<B, L> = &records[i];
                    if old.entity_type != ty
                        || old.basic != record.basic
                        || old.lists != record.lists
                        || old.basic_pins != record.basic_pins
                        || old.list_pins != record.list_pins
                    {
                        return Err(SolutionBindingError::ConflictingEntityRecord(ty).into());
                    }
                    i
                } else {
                    let i = records.len();
                    by_object.insert(key, i);
                    *counts.entry(ty).or_insert(0usize) += 1;
                    records.push(record);
                    i
                };
                indices.push(i);
            }
            members.insert(collection.member, indices);
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
        let mut ranges = HashMap::new();
        for d in descriptors
            .iter()
            .filter(|d| matches!(d.variable().kind(), PlanningVariableKind::Basic { .. }))
        {
            let field = d.definition();
            ranges.insert(
                d.ordinal(),
                d.bind_shared(
                    &basic_reads,
                    counts.get(&field.entity_type).copied().unwrap_or(0),
                )?,
            );
        }
        let mut variables = vec![];
        let mut list_entities = vec![];
        let mut entities = vec![];
        let mut ordinals = HashMap::new();
        for record in records {
            let ordinal = ordinals.entry(record.entity_type).or_insert(0usize);
            let mut slots = vec![];
            for (name, initial) in record.basic {
                let d = descriptors.get_type(record.entity_type, &name).unwrap();
                let mut variable = ranges[&d.ordinal()]
                    .variable(*ordinal)
                    .map_err(|_| SolutionBindingError::InvalidFields(record.entity_type))?;
                if let Some(value) = initial {
                    variable = variable.with_initial(value);
                }
                variable = variable.with_pinned(record.basic_pins.contains(&name));
                slots.push((name, variables.len()));
                variables.push(variable);
            }
            *ordinal += 1;
            let list_field = record.lists.into_iter().next().map(|(name, initial)| {
                let i = list_entities.len();
                list_entities.push(
                    ListEntity::new(initial)
                        .with_pinned(record.list_pins.fully_pinned)
                        .with_pinned_prefix(record.list_pins.prefix),
                );
                (name, i)
            });
            entities.push(BoundMixedEntity {
                object: record.object,
                entity_type: record.entity_type,
                basic_slots: slots,
                list_field,
            });
        }
        let basic = FiniteProblem::new(variables).map_err(SolutionBindingError::Model)?;
        let lists = if let Some(d) = list_fields.first() {
            d.bind_shared(&list_reads, list_entities.len())?
                .list_problem(list_entities)
                .map_err(|e| MixedSolutionBindingError::List(ListSolutionBindingError::Range(e)))?
        } else {
            ListProblem::new(vec![], vec![])
                .map_err(|e| MixedSolutionBindingError::List(ListSolutionBindingError::Model(e)))?
        };
        Ok(BoundMixedSolution {
            problem: MixedProblem::new(basic, lists),
            entities,
            collections: members,
            facts,
        })
    }
}
