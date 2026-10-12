//! Executable typed getter registration for concrete native basic inputs.
use crate::*;
use std::{any::Any, collections::HashMap, sync::Arc};
pub type BasicEntityFields<E, V> = (Arc<E>, Vec<(String, Option<V>)>);
type EntitiesRead<'a, S, V> = dyn Fn(&S) -> Vec<SolutionEntityRecord<V>> + 'a;
type RangeRead<'a, S, V> = dyn Fn(&S) -> Vec<V> + 'a;
#[derive(Debug)]
pub enum BasicSolutionAccessError {
    Binding(SolutionBindingError),
    Facts(SolutionFactError),
}
impl std::fmt::Display for BasicSolutionAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Facts(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for BasicSolutionAccessError {}
impl From<SolutionBindingError> for BasicSolutionAccessError {
    fn from(e: SolutionBindingError) -> Self {
        Self::Binding(e)
    }
}
impl From<SolutionFactError> for BasicSolutionAccessError {
    fn from(e: SolutionFactError) -> Self {
        Self::Facts(e)
    }
}
/// Callbacks may dispatch through application traits. Concrete handles preserve
/// actual allocations; domain payloads need no Clone bound. All registrations
/// validate before getter execution. Each entity/range callback runs once per
/// explicit bind; facts use their separately registered accessors. Callback side
/// effects are not rolled back, and a failed bind publishes no partial model.
pub struct SolutionBasicAccessors<'a, S, V> {
    layout: Arc<ComposedPlanningSolution>,
    fields: Vec<SolutionFieldRange>,
    entities: HashMap<String, Box<EntitiesRead<'a, S, V>>>,
    ranges: HashMap<String, Box<RangeRead<'a, S, V>>>,
    facts: SolutionFactAccessors<'a, S>,
}
impl<'a, S: 'static, V: Clone + Eq + 'static> SolutionBasicAccessors<'a, S, V> {
    pub fn new(
        layout: Arc<ComposedPlanningSolution>,
        fields: Vec<SolutionFieldRange>,
    ) -> Result<Self, BasicSolutionAccessError> {
        let facts = SolutionFactAccessors::new(layout.clone())?;
        for d in layout.compile_variable_ranges(fields.clone())?.iter() {
            if !matches!(d.variable().kind(), PlanningVariableKind::Basic { .. }) {
                return Err(SolutionBindingError::UnsupportedVariable {
                    entity: d.definition().entity_type,
                    variable: d.definition().variable.clone(),
                }
                .into());
            }
            if d.variable().value_type() != PlanningType::of::<V>() {
                return Err(SolutionBindingError::InvalidFieldRange {
                    entity: d.definition().entity_type,
                    variable: d.definition().variable.clone(),
                }
                .into());
            }
        }
        Ok(Self {
            layout,
            fields,
            entities: HashMap::new(),
            ranges: HashMap::new(),
            facts,
        })
    }
    pub fn facts_mut(&mut self) -> &mut SolutionFactAccessors<'a, S> {
        &mut self.facts
    }
    pub fn layout(&self) -> &Arc<ComposedPlanningSolution> {
        &self.layout
    }
    pub fn entities<E: Any + Send + Sync>(
        &mut self,
        name: impl Into<String>,
        read: impl Fn(&S) -> Vec<BasicEntityFields<E, V>> + 'a,
    ) -> Result<&mut Self, BasicSolutionAccessError> {
        let name = name.into();
        let m = self
            .layout
            .member(&name)
            .ok_or_else(|| SolutionBindingError::UnknownMember(name.clone()))?;
        let SolutionMemberKind::Entities { allowed_types, .. } = m.kind() else {
            return Err(SolutionBindingError::WrongMemberKind(name).into());
        };
        if !allowed_types.contains(&PlanningType::of::<E>()) {
            return Err(SolutionBindingError::WrongEntityType {
                member: name,
                entity: PlanningType::of::<E>(),
            }
            .into());
        }
        if self.entities.contains_key(&name) {
            return Err(SolutionBindingError::DuplicateBinding(name).into());
        }
        self.entities.insert(
            name,
            Box::new(move |s| {
                read(s)
                    .into_iter()
                    .map(|(o, f)| SolutionEntityRecord::new(o, f))
                    .collect()
            }),
        );
        Ok(self)
    }
    pub fn range(
        &mut self,
        name: impl Into<String>,
        read: impl Fn(&S) -> Vec<V> + 'a,
    ) -> Result<&mut Self, BasicSolutionAccessError> {
        let name = name.into();
        let m = self
            .layout
            .member(&name)
            .ok_or_else(|| SolutionBindingError::UnknownMember(name.clone()))?;
        let SolutionMemberKind::ValueRange { value_type, .. } = m.kind() else {
            return Err(SolutionBindingError::WrongMemberKind(name).into());
        };
        if *value_type != PlanningType::of::<V>() {
            return Err(SolutionBindingError::WrongValueType {
                member: name,
                expected: *value_type,
                actual: PlanningType::of::<V>(),
            }
            .into());
        }
        if self.ranges.contains_key(&name) {
            return Err(SolutionBindingError::DuplicateBinding(name).into());
        }
        self.ranges.insert(name, Box::new(read));
        Ok(self)
    }
    pub fn bind(&self, solution: &S) -> Result<BoundBasicSolution<V>, BasicSolutionAccessError> {
        for m in self.layout.members() {
            let present = match m.kind() {
                SolutionMemberKind::Entities { .. } => self.entities.contains_key(m.name()),
                SolutionMemberKind::ValueRange { .. } => self.ranges.contains_key(m.name()),
                _ => true,
            };
            if !present {
                return Err(SolutionBindingError::MissingBinding(m.name().into()).into());
            }
        }
        let facts = self.facts.read(solution)?;
        let collections = self
            .layout
            .members()
            .iter()
            .filter_map(|m| {
                self.entities.get(m.name()).map(|read| SolutionCollection {
                    member: m.name().into(),
                    entities: read(solution),
                })
            })
            .collect();
        let ranges = self
            .layout
            .members()
            .iter()
            .filter_map(|m| {
                self.ranges.get(m.name()).map(|read| SolutionRange {
                    member: m.name().into(),
                    values: read(solution),
                })
            })
            .collect();
        Ok(self
            .layout
            .bind_basic_with_facts(collections, ranges, self.fields.clone(), facts)?)
    }
}
