//! Bind one declared repair field to native basic slots or a list value registry.
use crate::*;
use std::{collections::BTreeSet, error::Error, fmt};

/// A ruin selector resolves exactly one genuine entity type and one field.
/// Shadow-only types are ignored during deduction. Explicit fields remove
/// ambiguity; runtime pins and eligibility remain the native repair engine's job.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchemaRuinConfig {
    pub entity: Option<PlanningType>,
    pub variable: Option<String>,
}
#[derive(Clone, Debug)]
pub struct SchemaRuinBinding {
    pub entity: PlanningType,
    pub variable: String,
    /// Basic variable slots or list entity slots, in entity order.
    pub slots: Vec<usize>,
}
#[derive(Clone, Debug)]
pub struct SchemaRuinPhaseConfiguration {
    pub phase: usize,
    pub selector: SchemaRuinConfig,
    pub bindings: Vec<SchemaRuinBinding>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaRuinError {
    Schema(PlanningSchemaError),
    NoGenuineEntities,
    AmbiguousEntities {
        count: usize,
    },
    NoVariables {
        entity: PlanningType,
    },
    AmbiguousVariables {
        entity: PlanningType,
        count: usize,
    },
    UnknownVariable {
        entity: PlanningType,
        name: String,
    },
    ShadowVariable {
        entity: PlanningType,
        name: String,
    },
    MissingBinding {
        entity: PlanningType,
        name: String,
    },
    DuplicateBinding {
        entity: PlanningType,
        name: String,
    },
    DuplicateSlot {
        slot: usize,
    },
    WrongModel,
    WrongValueType {
        expected: PlanningType,
        actual: PlanningType,
    },
    SlotOutOfBounds {
        slot: usize,
        count: usize,
    },
    UnassignedPolicy {
        name: String,
    },
    PartialListScope,
    PhaseIndex {
        phase: usize,
        count: usize,
    },
    NotRuinPhase {
        phase: usize,
    },
    DuplicatePhase {
        phase: usize,
    },
}
impl fmt::Display for SchemaRuinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(e) => e.fmt(f),
            Self::AmbiguousEntities { .. } => f.write_str("ruin selector has no entityClass configured and because there are multiple in the entityClassSet, it cannot be deduced automatically; select an explicit native entity type"),
            Self::AmbiguousVariables { entity, count } => write!(f,"The entity class ({}) contains several variables ({count}), so it cannot be deduced automatically. Maybe set the property variableName (the native selector variable field)",entity.name()),
            Self::UnknownVariable { entity, name } => write!(f,"The entity class ({}) has no variable named {name}",entity.name()),
            Self::ShadowVariable { entity, name } => write!(f,"The entity class ({}) variable {name} is a shadow field and cannot be ruined",entity.name()),
            Self::PartialListScope => f.write_str("a native list repair binding must cover the complete list field; partial list entity scopes are not supported"),
            _ => write!(f,"schema ruin selection: {self:?}"),
        }
    }
}
impl Error for SchemaRuinError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Schema(e) => Some(e),
            _ => None,
        }
    }
}
impl SchemaRuinConfig {
    pub fn resolve_schema_ruin<'s>(
        &self,
        schema: &'s PlanningSchema,
    ) -> Result<PlanningVariableRef<'s>, SchemaRuinError> {
        let entity = if let Some(kind) = self.entity {
            let entity = schema.entity_type(kind).map_err(SchemaRuinError::Schema)?;
            if !entity.is_genuine() {
                return Err(SchemaRuinError::NoVariables { entity: kind });
            }
            entity
        } else {
            let mut entities = schema.genuine_entities();
            let first = entities.next().ok_or(SchemaRuinError::NoGenuineEntities)?;
            let rest = entities.count();
            if rest != 0 {
                return Err(SchemaRuinError::AmbiguousEntities { count: rest + 1 });
            }
            first
        };
        if let Some(name) = &self.variable {
            let field = entity
                .variable(name)
                .map_err(|_| SchemaRuinError::UnknownVariable {
                    entity: entity.entity_type(),
                    name: name.clone(),
                })?;
            if !field.is_genuine() {
                return Err(SchemaRuinError::ShadowVariable {
                    entity: entity.entity_type(),
                    name: name.clone(),
                });
            }
            return Ok(field);
        }
        let mut fields = entity.genuine_variables();
        let first = fields.next().ok_or(SchemaRuinError::NoVariables {
            entity: entity.entity_type(),
        })?;
        let rest = fields.count();
        if rest != 0 {
            return Err(SchemaRuinError::AmbiguousVariables {
                entity: entity.entity_type(),
                count: rest + 1,
            });
        }
        Ok(first)
    }
    /// Resolve before binding, so an ambiguous selection cannot be hidden by a
    /// guessed runtime slot mapping. The resulting selector owns only metadata.
    pub fn bind_schema_ruin(
        &self,
        schema: &PlanningSchema,
        bindings: &[SchemaRuinBinding],
    ) -> Result<SchemaRuinSelector, SchemaRuinError> {
        let field = self.resolve_schema_ruin(schema)?;
        let entity = field.entity().entity_type();
        let mut matching = bindings
            .iter()
            .filter(|b| b.entity == entity && b.variable == field.name());
        let binding = matching
            .next()
            .ok_or_else(|| SchemaRuinError::MissingBinding {
                entity,
                name: field.name().into(),
            })?;
        if matching.next().is_some() {
            return Err(SchemaRuinError::DuplicateBinding {
                entity,
                name: field.name().into(),
            });
        }
        let mut unique = BTreeSet::new();
        for &slot in &binding.slots {
            if !unique.insert(slot) {
                return Err(SchemaRuinError::DuplicateSlot { slot });
            }
        }
        Ok(SchemaRuinSelector {
            entity,
            name: field.name().into(),
            kind: field.kind(),
            value_type: field.value_type(),
            slots: binding.slots.clone(),
        })
    }
}
/// Immutable schema/slot binding. Candidate helpers validate the actual model
/// once before installing the pool in RuinRecreateConfig::ruin_candidates. Basic
/// pools select one column; list pools select the complete bound list registry.
/// Mixed list indices are offset by the actual basic-slot count. Fine families
/// and any additional repair pools remain separately configured by the caller.
#[derive(Clone, Debug)]
pub struct SchemaRuinSelector {
    entity: PlanningType,
    name: String,
    kind: PlanningVariableKind,
    value_type: PlanningType,
    slots: Vec<usize>,
}
impl SchemaRuinSelector {
    pub fn ruin_entity_type(&self) -> PlanningType {
        self.entity
    }
    pub fn ruin_variable_name(&self) -> &str {
        &self.name
    }
    pub fn ruin_variable_kind(&self) -> PlanningVariableKind {
        self.kind
    }
    fn check_value_type<V: 'static>(&self) -> Result<(), SchemaRuinError> {
        let actual = PlanningType::of::<V>();
        if actual == self.value_type {
            Ok(())
        } else {
            Err(SchemaRuinError::WrongValueType {
                expected: self.value_type,
                actual,
            })
        }
    }
    pub fn basic_ruin_candidates<V: Eq + 'static>(
        &self,
        problem: &FiniteProblem<V>,
    ) -> Result<Vec<usize>, SchemaRuinError> {
        let PlanningVariableKind::Basic { allows_unassigned } = self.kind else {
            return Err(SchemaRuinError::WrongModel);
        };
        self.check_value_type::<V>()?;
        for &slot in &self.slots {
            let variable =
                problem
                    .variables()
                    .get(slot)
                    .ok_or(SchemaRuinError::SlotOutOfBounds {
                        slot,
                        count: problem.variables().len(),
                    })?;
            if variable.allows_unassigned() != allows_unassigned {
                return Err(SchemaRuinError::UnassignedPolicy {
                    name: self.name.clone(),
                });
            }
        }
        Ok(self.slots.clone())
    }
    pub fn list_ruin_candidates<V: Eq + Clone + 'static>(
        &self,
        problem: &ListProblem<V>,
    ) -> Result<Vec<usize>, SchemaRuinError> {
        let PlanningVariableKind::List { allows_unassigned } = self.kind else {
            return Err(SchemaRuinError::WrongModel);
        };
        self.check_value_type::<V>()?;
        for &slot in &self.slots {
            if slot >= problem.entities().len() {
                return Err(SchemaRuinError::SlotOutOfBounds {
                    slot,
                    count: problem.entities().len(),
                });
            }
        }
        if self.slots.len() != problem.entities().len() {
            return Err(SchemaRuinError::PartialListScope);
        }
        if problem.allows_unassigned() != allows_unassigned {
            return Err(SchemaRuinError::UnassignedPolicy {
                name: self.name.clone(),
            });
        }
        Ok((0..problem.values().len()).collect())
    }
    pub fn mixed_ruin_candidates<B: Eq + Clone + 'static, L: Eq + Clone + 'static>(
        &self,
        problem: &MixedProblem<B, L>,
    ) -> Result<Vec<usize>, SchemaRuinError> {
        match self.kind {
            PlanningVariableKind::Basic { .. } => self.basic_ruin_candidates(problem.basic()),
            PlanningVariableKind::List { .. } => Ok(self
                .list_ruin_candidates(problem.lists())?
                .into_iter()
                .map(|i| problem.basic().variables().len() + i)
                .collect()),
            PlanningVariableKind::Shadow => Err(SchemaRuinError::WrongModel),
        }
    }
}
