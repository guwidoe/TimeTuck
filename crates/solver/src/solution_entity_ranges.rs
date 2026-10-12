//! Typed solution-aware entity providers bound to actual native basic fields.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};
struct Record<V> {
    object: Arc<dyn Any + Send + Sync>,
    owner: PlanningType,
    fields: Vec<(String, Option<V>)>,
}
type EntityRead<'a, S, V> = dyn Fn(&S) -> Vec<Record<V>> + 'a;
type ProviderRead<'a, S, V> = dyn Fn(&S, &Arc<dyn Any + Send + Sync>) -> Vec<V> + 'a;
pub struct BoundEntityRangeOwner {
    object: Arc<dyn Any + Send + Sync>,
    owner: PlanningType,
    slots: Vec<(String, usize)>,
}
impl BoundEntityRangeOwner {
    pub fn object_ref<E: Any>(&self) -> Option<&E> {
        self.object.downcast_ref()
    }
    pub fn object(&self) -> &Arc<dyn Any + Send + Sync> {
        &self.object
    }
    pub fn entity_type(&self) -> PlanningType {
        self.owner
    }
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.slots.iter().find(|(n, _)| n == name).map(|(_, i)| *i)
    }
}
pub struct BoundEntityRangeSolution<V> {
    problem: FiniteProblem<V>,
    entities: Vec<BoundEntityRangeOwner>,
}
impl<V> BoundEntityRangeSolution<V> {
    pub fn problem(&self) -> &FiniteProblem<V> {
        &self.problem
    }
    pub fn entities(&self) -> &[BoundEntityRangeOwner] {
        &self.entities
    }
    pub fn into_problem(self) -> FiniteProblem<V> {
        self.problem
    }
}
/// Multiple genuine basic entity types/fields and providers are supported.
/// Providers receive the actual solution and entity; application composition
/// can explicitly project a base solution for inherited provider signatures.
/// Validate all registrations before callbacks. Aliased entity allocations bind
/// once, with conflicting field snapshots rejected. Each provider is read once
/// per unique entity per bind, even when several fields reference it. Callback effects/panics are
/// not rolled back. This binds domains/current values, not score/fact metadata.
pub struct SolutionEntityRangeAccessors<'a, S, V> {
    schema: Arc<PlanningSchema>,
    fields: Vec<SolutionFieldRange>,
    entities: HashMap<PlanningType, Box<EntityRead<'a, S, V>>>,
    providers: HashMap<(PlanningType, String), Box<ProviderRead<'a, S, V>>>,
}
impl<'a, S: 'static, V: Eq + Clone + 'static> SolutionEntityRangeAccessors<'a, S, V> {
    pub fn new(
        schema: Arc<PlanningSchema>,
        fields: Vec<SolutionFieldRange>,
    ) -> Result<Self, SolutionBindingError> {
        if schema.solution_type() != PlanningType::of::<S>() {
            return Err(SolutionBindingError::WrongValueType {
                member: "solution".into(),
                expected: schema.solution_type(),
                actual: PlanningType::of::<S>(),
            });
        }
        let mut seen = HashSet::new();
        for field in &fields {
            let variable = schema
                .entity_type(field.entity_type)
                .ok()
                .and_then(|e| e.genuine_variable(&field.variable).ok())
                .ok_or(SolutionBindingError::InvalidFieldRange {
                    entity: field.entity_type,
                    variable: field.variable.clone(),
                })?;
            if !matches!(variable.kind(), PlanningVariableKind::Basic { .. }) {
                return Err(SolutionBindingError::UnsupportedVariable {
                    entity: field.entity_type,
                    variable: field.variable.clone(),
                });
            }
            if variable.value_type() != PlanningType::of::<V>()
                || field.providers.is_empty()
                || !seen.insert((field.entity_type, field.variable.clone()))
            {
                return Err(SolutionBindingError::InvalidFieldRange {
                    entity: field.entity_type,
                    variable: field.variable.clone(),
                });
            }
            let mut ids = HashSet::new();
            if field
                .providers
                .iter()
                .any(|id| id.is_empty() || !ids.insert(id))
            {
                return Err(SolutionBindingError::InvalidFieldRange {
                    entity: field.entity_type,
                    variable: field.variable.clone(),
                });
            }
        }
        for entity in schema.genuine_entities() {
            for variable in entity.genuine_variables() {
                if !seen.contains(&(entity.entity_type(), variable.name().into())) {
                    return Err(SolutionBindingError::MissingFieldRange {
                        entity: entity.entity_type(),
                        variable: variable.name().into(),
                    });
                }
            }
        }
        Ok(Self {
            schema,
            fields,
            entities: HashMap::new(),
            providers: HashMap::new(),
        })
    }
    pub fn entities<E: Any + Send + Sync>(
        &mut self,
        read: impl Fn(&S) -> Vec<BasicEntityFields<E, V>> + 'a,
    ) -> Result<&mut Self, SolutionBindingError> {
        let owner = PlanningType::of::<E>();
        if self.schema.genuine_entity::<E>().is_err() {
            return Err(SolutionBindingError::WrongEntityType {
                member: "entities".into(),
                entity: owner,
            });
        }
        if self.entities.contains_key(&owner) {
            return Err(SolutionBindingError::DuplicateBinding(owner.name().into()));
        }
        self.entities.insert(
            owner,
            Box::new(move |s| {
                read(s)
                    .into_iter()
                    .map(|(object, fields)| Record {
                        object,
                        owner,
                        fields,
                    })
                    .collect()
            }),
        );
        Ok(self)
    }
    pub fn provider<E: Any + Send + Sync>(
        &mut self,
        id: impl Into<String>,
        read: impl Fn(&S, &E) -> Vec<V> + 'a,
    ) -> Result<&mut Self, SolutionBindingError> {
        let owner = PlanningType::of::<E>();
        let id = id.into();
        if !self
            .fields
            .iter()
            .any(|f| f.entity_type == owner && f.providers.contains(&id))
        {
            return Err(SolutionBindingError::UnknownProvider(id));
        }
        let key = (owner, id.clone());
        if self.providers.contains_key(&key) {
            return Err(SolutionBindingError::DuplicateBinding(id));
        }
        self.providers.insert(
            key,
            Box::new(move |s, e| {
                read(s, e.downcast_ref::<E>().expect("typed entity registration"))
            }),
        );
        Ok(self)
    }
    pub fn validate(&self) -> Result<(), SolutionBindingError> {
        for e in self.schema.genuine_entities() {
            if !self.entities.contains_key(&e.entity_type()) {
                return Err(SolutionBindingError::MissingBinding(
                    e.entity_type().name().into(),
                ));
            }
        }
        for f in &self.fields {
            for id in &f.providers {
                if !self.providers.contains_key(&(f.entity_type, id.clone())) {
                    return Err(SolutionBindingError::UnknownProvider(id.clone()));
                }
            }
        }
        Ok(())
    }
    pub fn bind(&self, solution: &S) -> Result<BoundEntityRangeSolution<V>, SolutionBindingError> {
        self.validate()?;
        let mut variables = vec![];
        let mut owners = vec![];
        let mut seen: HashMap<usize, Vec<Option<V>>> = HashMap::new();
        for entity in self.schema.genuine_entities() {
            for record in self.entities[&entity.entity_type()](solution) {
                let values: HashMap<_, _> =
                    record.fields.iter().map(|(n, v)| (n.as_str(), v)).collect();
                if values.len() != record.fields.len()
                    || values.len() != entity.genuine_variables().count()
                    || entity
                        .genuine_variables()
                        .any(|v| !values.contains_key(v.name()))
                {
                    return Err(SolutionBindingError::InvalidFields(record.owner));
                }
                let ordered: Vec<_> = entity
                    .genuine_variables()
                    .map(|v| values[v.name()].clone())
                    .collect();
                let key = Arc::as_ptr(&record.object) as *const () as usize;
                if let Some(old) = seen.get(&key) {
                    if old != &ordered {
                        return Err(SolutionBindingError::ConflictingEntityRecord(record.owner));
                    }
                    continue;
                }
                let mut slots = vec![];
                let mut provider_values: HashMap<&str, Vec<V>> = HashMap::new();
                for variable in entity.genuine_variables() {
                    let field = self
                        .fields
                        .iter()
                        .find(|f| f.entity_type == record.owner && f.variable == variable.name())
                        .expect("validated field");
                    let mut domain = vec![];
                    for id in &field.providers {
                        let provided = provider_values.entry(id.as_str()).or_insert_with(|| {
                            self.providers[&(record.owner, id.clone())](solution, &record.object)
                        });
                        for value in provided.iter() {
                            if !domain.contains(value) {
                                domain.push(value.clone());
                            }
                        }
                    }
                    let mut model = Variable::new(domain);
                    if matches!(
                        variable.kind(),
                        PlanningVariableKind::Basic {
                            allows_unassigned: true
                        }
                    ) {
                        model = model.allowing_unassigned();
                    }
                    if let Some(value) = values[variable.name()] {
                        model = model.with_initial(value.clone());
                    }
                    slots.push((variable.name().into(), variables.len()));
                    variables.push(model);
                }
                seen.insert(key, ordered);
                owners.push(BoundEntityRangeOwner {
                    object: record.object,
                    owner: record.owner,
                    slots,
                });
            }
        }
        let problem = FiniteProblem::new(variables).map_err(SolutionBindingError::Model)?;
        Ok(BoundEntityRangeSolution {
            problem,
            entities: owners,
        })
    }
}
