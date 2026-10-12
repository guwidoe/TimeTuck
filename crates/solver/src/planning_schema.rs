//! Explicit typed planning declarations and stable borrowed metadata views.
use crate::{
    ManagedRangeKind, ManagedValueRanges, StatisticsEntityType, StatisticsVariable,
    StatisticsVariableKind,
};
use std::{
    any::{TypeId, type_name},
    collections::BTreeSet,
    error::Error,
    fmt,
};

/// Runtime identity of a native Rust type; names are for diagnostics, not identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PlanningType {
    id: TypeId,
    name: &'static str,
}
impl PlanningType {
    pub fn of<T: 'static>() -> Self {
        Self {
            id: TypeId::of::<T>(),
            name: type_name::<T>(),
        }
    }
    pub fn id(self) -> TypeId {
        self.id
    }
    pub fn name(self) -> &'static str {
        self.name
    }
    pub fn is<T: 'static>(self) -> bool {
        self.id == TypeId::of::<T>()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanningVariableKind {
    Basic { allows_unassigned: bool },
    List { allows_unassigned: bool },
    Shadow,
}
#[derive(Clone, Debug)]
pub struct PlanningVariableDefinition {
    name: String,
    value_type: PlanningType,
    kind: PlanningVariableKind,
}
impl PlanningVariableDefinition {
    pub fn basic<V: 'static>(name: impl Into<String>, allows_unassigned: bool) -> Self {
        Self {
            name: name.into(),
            value_type: PlanningType::of::<V>(),
            kind: PlanningVariableKind::Basic { allows_unassigned },
        }
    }
    /// V is the list element type, rather than Vec<V>.
    pub fn list<V: 'static>(name: impl Into<String>, allows_unassigned: bool) -> Self {
        Self {
            name: name.into(),
            value_type: PlanningType::of::<V>(),
            kind: PlanningVariableKind::List { allows_unassigned },
        }
    }
    pub fn shadow<V: 'static>(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value_type: PlanningType::of::<V>(),
            kind: PlanningVariableKind::Shadow,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value_type(&self) -> PlanningType {
        self.value_type
    }
    pub fn kind(&self) -> PlanningVariableKind {
        self.kind
    }
}
#[derive(Clone, Debug)]
pub struct PlanningEntityDefinition {
    entity_type: PlanningType,
    variables: Vec<PlanningVariableDefinition>,
}
impl PlanningEntityDefinition {
    pub(crate) fn from_parts(
        entity_type: PlanningType,
        variables: Vec<PlanningVariableDefinition>,
    ) -> Self {
        Self {
            entity_type,
            variables,
        }
    }
    pub fn of<E: 'static>(variables: Vec<PlanningVariableDefinition>) -> Self {
        Self {
            entity_type: PlanningType::of::<E>(),
            variables,
        }
    }
    pub fn entity_type(&self) -> PlanningType {
        self.entity_type
    }
    pub fn variables(&self) -> &[PlanningVariableDefinition] {
        &self.variables
    }
    pub fn is_genuine(&self) -> bool {
        self.variables
            .iter()
            .any(|v| v.kind != PlanningVariableKind::Shadow)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanningSchemaError {
    DuplicateEntity(PlanningType),
    EmptyVariableName(PlanningType),
    DuplicateVariable {
        entity: PlanningType,
        variable: String,
    },
    UnknownEntity(PlanningType),
    WrongEntityKind {
        entity: PlanningType,
        expected_genuine: bool,
    },
    UnknownVariable {
        entity: PlanningType,
        variable: String,
    },
    WrongVariableKind {
        variable: String,
    },
    WrongValueType {
        variable: String,
        expected: PlanningType,
        actual: PlanningType,
    },
    MissingBinding {
        entity: PlanningType,
        variable: String,
    },
    BindingLength {
        variable: String,
        expected: usize,
        actual: usize,
    },
}
impl fmt::Display for PlanningSchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "planning schema: {self:?}")
    }
}
impl Error for PlanningSchemaError {}
/// Validated immutable metadata; declarations retain their supplied order.
/// Explicit Rust types/fields replace annotation discovery. Field getters/setters,
/// object traversal and shadow evaluation remain separate concerns. Explicit
/// component ancestry can be compiled through PlanningComposition.
/// Borrowed handles cannot outlive their schema, and refer to canonical metadata.
///
/// ```
/// use timetuck::*;
/// struct Schedule; struct Task;
/// let schema=PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Task>(
///     vec![PlanningVariableDefinition::basic::<i32>("worker",false)])])?;
/// let worker=schema.genuine_entity::<Task>()?.genuine_variable("worker")?;
/// let ranges=worker.shared_ranges(vec![vec![1,2]],3)?;
/// let problem=FiniteProblem::new((0..3).map(|i|ranges.variable(i).unwrap()).collect())?;
/// assert_eq!(problem.variables().len(),3);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct PlanningSchema {
    solution_type: PlanningType,
    entities: Vec<PlanningEntityDefinition>,
}
impl PlanningSchema {
    pub fn new<S: 'static>(
        entities: Vec<PlanningEntityDefinition>,
    ) -> Result<Self, PlanningSchemaError> {
        for (index, entity) in entities.iter().enumerate() {
            if entities[..index]
                .iter()
                .any(|e| e.entity_type == entity.entity_type)
            {
                return Err(PlanningSchemaError::DuplicateEntity(entity.entity_type));
            }
            let mut names = BTreeSet::new();
            for variable in &entity.variables {
                if variable.name.is_empty() {
                    return Err(PlanningSchemaError::EmptyVariableName(entity.entity_type));
                }
                if !names.insert(&variable.name) {
                    return Err(PlanningSchemaError::DuplicateVariable {
                        entity: entity.entity_type,
                        variable: variable.name.clone(),
                    });
                }
            }
        }
        Ok(Self {
            solution_type: PlanningType::of::<S>(),
            entities,
        })
    }
    pub fn solution_type(&self) -> PlanningType {
        self.solution_type
    }
    pub fn entities(
        &self,
    ) -> impl ExactSizeIterator<Item = PlanningEntityRef<'_>> + DoubleEndedIterator {
        (0..self.entities.len()).map(|index| PlanningEntityRef {
            schema: self,
            index,
        })
    }
    pub fn genuine_entities(&self) -> impl Iterator<Item = PlanningEntityRef<'_>> {
        self.entities().filter(|e| e.is_genuine())
    }
    pub fn shadow_entities(&self) -> impl Iterator<Item = PlanningEntityRef<'_>> {
        self.entities().filter(|e| !e.is_genuine())
    }
    pub fn entity<E: 'static>(&self) -> Result<PlanningEntityRef<'_>, PlanningSchemaError> {
        self.entity_type(PlanningType::of::<E>())
    }
    pub fn entity_type(
        &self,
        entity_type: PlanningType,
    ) -> Result<PlanningEntityRef<'_>, PlanningSchemaError> {
        self.entities()
            .find(|e| e.entity_type() == entity_type)
            .ok_or(PlanningSchemaError::UnknownEntity(entity_type))
    }
    pub fn genuine_entity<E: 'static>(&self) -> Result<PlanningEntityRef<'_>, PlanningSchemaError> {
        self.entity::<E>()?.expect_kind(true)
    }
    pub fn shadow_entity<E: 'static>(&self) -> Result<PlanningEntityRef<'_>, PlanningSchemaError> {
        self.entity::<E>()?.expect_kind(false)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PlanningEntityRef<'a> {
    schema: &'a PlanningSchema,
    index: usize,
}
impl PartialEq for PlanningEntityRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.schema, other.schema) && self.index == other.index
    }
}
impl Eq for PlanningEntityRef<'_> {}
impl<'a> PlanningEntityRef<'a> {
    /// Dense descriptor index within this schema, independent of object rows.
    pub fn ordinal(self) -> usize {
        self.index
    }
    pub fn definition(self) -> &'a PlanningEntityDefinition {
        &self.schema.entities[self.index]
    }
    pub fn solution(self) -> &'a PlanningSchema {
        self.schema
    }
    pub fn entity_type(self) -> PlanningType {
        self.definition().entity_type
    }
    pub fn is_genuine(self) -> bool {
        self.definition().is_genuine()
    }
    fn expect_kind(self, genuine: bool) -> Result<Self, PlanningSchemaError> {
        if self.is_genuine() == genuine {
            Ok(self)
        } else {
            Err(PlanningSchemaError::WrongEntityKind {
                entity: self.entity_type(),
                expected_genuine: genuine,
            })
        }
    }
    pub fn variables(
        self,
    ) -> impl ExactSizeIterator<Item = PlanningVariableRef<'a>> + DoubleEndedIterator {
        (0..self.definition().variables.len()).map(move |index| PlanningVariableRef {
            entity: self,
            index,
        })
    }
    pub fn genuine_variables(self) -> impl Iterator<Item = PlanningVariableRef<'a>> {
        self.variables().filter(|v| v.is_genuine())
    }
    pub fn shadow_variables(self) -> impl Iterator<Item = PlanningVariableRef<'a>> {
        self.variables().filter(|v| !v.is_genuine())
    }
    pub fn variable(self, name: &str) -> Result<PlanningVariableRef<'a>, PlanningSchemaError> {
        self.variables().find(|v| v.name() == name).ok_or_else(|| {
            PlanningSchemaError::UnknownVariable {
                entity: self.entity_type(),
                variable: name.into(),
            }
        })
    }
    pub fn genuine_variable(
        self,
        name: &str,
    ) -> Result<PlanningVariableRef<'a>, PlanningSchemaError> {
        self.variable(name)?.expect_kind(true)
    }
    pub fn shadow_variable(
        self,
        name: &str,
    ) -> Result<PlanningVariableRef<'a>, PlanningSchemaError> {
        self.variable(name)?.expect_kind(false)
    }
    /// Bind every genuine field exactly once to flattened native slots. Shadow
    /// fields are omitted. NamedProblemSize validates global slot uniqueness and
    /// model bounds when the resulting schema is applied to an actual model.
    pub fn statistics_binding(
        self,
        entity_count: usize,
        fields: Vec<StatisticsVariable>,
    ) -> Result<StatisticsEntityType, PlanningSchemaError> {
        self.expect_kind(true)?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            let var = self.genuine_variable(&field.name)?;
            let expected = var.statistics_kind()?;
            if expected != field.kind {
                return Err(PlanningSchemaError::WrongVariableKind {
                    variable: field.name.clone(),
                });
            }
            if !seen.insert(&field.name) {
                return Err(PlanningSchemaError::DuplicateVariable {
                    entity: self.entity_type(),
                    variable: field.name.clone(),
                });
            }
            if field.slots.len() != entity_count {
                return Err(PlanningSchemaError::BindingLength {
                    variable: field.name.clone(),
                    expected: entity_count,
                    actual: field.slots.len(),
                });
            }
        }
        let mut ordered = vec![];
        for var in self.genuine_variables() {
            ordered.push(
                fields
                    .iter()
                    .find(|f| f.name == var.name())
                    .cloned()
                    .ok_or_else(|| PlanningSchemaError::MissingBinding {
                        entity: self.entity_type(),
                        variable: var.name().into(),
                    })?,
            );
        }
        Ok(StatisticsEntityType {
            name: self.entity_type().name().into(),
            entity_count,
            variables: ordered,
        })
    }
}
#[derive(Clone, Copy, Debug)]
pub struct PlanningVariableRef<'a> {
    entity: PlanningEntityRef<'a>,
    index: usize,
}
impl PartialEq for PlanningVariableRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity && self.index == other.index
    }
}
impl Eq for PlanningVariableRef<'_> {}
impl<'a> PlanningVariableRef<'a> {
    pub fn definition(self) -> &'a PlanningVariableDefinition {
        &self.entity.definition().variables[self.index]
    }
    pub fn entity(self) -> PlanningEntityRef<'a> {
        self.entity
    }
    pub fn name(self) -> &'a str {
        &self.definition().name
    }
    pub fn value_type(self) -> PlanningType {
        self.definition().value_type
    }
    pub fn kind(self) -> PlanningVariableKind {
        self.definition().kind
    }
    pub fn is_genuine(self) -> bool {
        self.kind() != PlanningVariableKind::Shadow
    }
    fn expect_kind(self, genuine: bool) -> Result<Self, PlanningSchemaError> {
        if self.is_genuine() == genuine {
            Ok(self)
        } else {
            Err(PlanningSchemaError::WrongVariableKind {
                variable: self.name().into(),
            })
        }
    }
    fn range_kind<V: 'static>(self) -> Result<ManagedRangeKind, PlanningSchemaError> {
        let kind = match self.kind() {
            PlanningVariableKind::Basic { allows_unassigned } => {
                ManagedRangeKind::Basic { allows_unassigned }
            }
            PlanningVariableKind::List { allows_unassigned } => {
                ManagedRangeKind::List { allows_unassigned }
            }
            PlanningVariableKind::Shadow => {
                return Err(PlanningSchemaError::WrongVariableKind {
                    variable: self.name().into(),
                });
            }
        };
        let actual = PlanningType::of::<V>();
        if actual != self.value_type() {
            return Err(PlanningSchemaError::WrongValueType {
                variable: self.name().into(),
                expected: self.value_type(),
                actual,
            });
        }
        Ok(kind)
    }
    pub fn shared_ranges<V: Clone + Eq + 'static>(
        self,
        providers: Vec<Vec<V>>,
        entity_count: usize,
    ) -> Result<ManagedValueRanges<V>, PlanningSchemaError> {
        Ok(ManagedValueRanges::shared(
            self.range_kind::<V>()?,
            providers,
            entity_count,
        ))
    }
    pub fn entity_ranges<V: Clone + Eq + 'static>(
        self,
        providers: Vec<Vec<Vec<V>>>,
    ) -> Result<ManagedValueRanges<V>, PlanningSchemaError> {
        Ok(ManagedValueRanges::entity_provided(
            self.range_kind::<V>()?,
            providers,
        ))
    }
    fn statistics_kind(self) -> Result<StatisticsVariableKind, PlanningSchemaError> {
        match self.kind() {
            PlanningVariableKind::Basic { .. } => Ok(StatisticsVariableKind::Basic),
            PlanningVariableKind::List { .. } => Ok(StatisticsVariableKind::List),
            PlanningVariableKind::Shadow => Err(PlanningSchemaError::WrongVariableKind {
                variable: self.name().into(),
            }),
        }
    }
    pub fn statistics_binding(
        self,
        slots: Vec<usize>,
        empty_value_count: Option<usize>,
    ) -> Result<StatisticsVariable, PlanningSchemaError> {
        Ok(StatisticsVariable {
            name: self.name().into(),
            kind: self.statistics_kind()?,
            slots,
            empty_value_count,
        })
    }
}
