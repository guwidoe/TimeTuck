//! Canonical variable-specific range descriptors backed by real provider members.
use crate::{
    ComposedPlanningSolution, ManagedValueRanges, PlanningType, PlanningVariableRef,
    SolutionBindingError, SolutionFieldRange, SolutionMemberKind, SolutionRange,
};
use std::collections::HashMap;

/// A descriptor is per genuine variable, even when several fields share one
/// provider. Ordinals follow schema entity/field order, not input binding order.
/// It borrows the actual variable and validates all provider types at compilation.
pub struct SolutionVariableRange<'s> {
    layout: &'s ComposedPlanningSolution,
    variable: PlanningVariableRef<'s>,
    definition: SolutionFieldRange,
    ordinal: usize,
}
impl<'s> SolutionVariableRange<'s> {
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn definition(&self) -> &SolutionFieldRange {
        &self.definition
    }
    /// Bind typed reads of the actual provider members into managed native
    /// basic/list ranges. Provider order follows this variable's declaration.
    pub fn bind_shared<V: Clone + Eq + 'static>(
        &self,
        reads: &[SolutionRange<V>],
        entity_count: usize,
    ) -> Result<ManagedValueRanges<V>, SolutionBindingError> {
        let mut providers = HashMap::new();
        for read in reads {
            let member = self
                .layout
                .member(&read.member)
                .ok_or_else(|| SolutionBindingError::UnknownMember(read.member.clone()))?;
            let SolutionMemberKind::ValueRange {
                provider,
                value_type,
            } = member.kind()
            else {
                return Err(SolutionBindingError::WrongMemberKind(read.member.clone()));
            };
            if *value_type != PlanningType::of::<V>() {
                return Err(SolutionBindingError::WrongValueType {
                    member: read.member.clone(),
                    expected: *value_type,
                    actual: PlanningType::of::<V>(),
                });
            }
            if providers.insert(provider.as_str(), &read.values).is_some() {
                return Err(SolutionBindingError::DuplicateBinding(read.member.clone()));
            }
        }
        let values = self
            .definition
            .providers
            .iter()
            .map(|p| {
                providers
                    .get(p.as_str())
                    .map(|v| (*v).clone())
                    .ok_or_else(|| SolutionBindingError::UnknownProvider(p.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.variable
            .shared_ranges(values, entity_count)
            .map_err(|_| SolutionBindingError::InvalidFieldRange {
                entity: self.definition.entity_type,
                variable: self.definition.variable.clone(),
            })
    }
}
pub struct SolutionVariableRanges<'s> {
    descriptors: Vec<SolutionVariableRange<'s>>,
    indices: HashMap<(PlanningType, String), usize>,
}
impl<'s> SolutionVariableRanges<'s> {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &SolutionVariableRange<'s>> {
        self.descriptors.iter()
    }
    pub fn get<E: 'static>(&self, name: &str) -> Option<&SolutionVariableRange<'s>> {
        self.get_type(PlanningType::of::<E>(), name)
    }
    pub fn get_type(&self, owner: PlanningType, name: &str) -> Option<&SolutionVariableRange<'s>> {
        self.indices
            .get(&(owner, name.into()))
            .map(|&i| &self.descriptors[i])
    }
}
impl ComposedPlanningSolution {
    /// Resolve basic and list fields to actual declared provider members. Every
    /// genuine field requires exactly one descriptor; shadows cannot own ranges.
    /// A shared provider can be referenced by several independent descriptors.
    pub fn compile_variable_ranges(
        &self,
        fields: Vec<SolutionFieldRange>,
    ) -> Result<SolutionVariableRanges<'_>, SolutionBindingError> {
        let providers: HashMap<_, _> = self
            .members()
            .iter()
            .filter_map(|m| match m.kind() {
                SolutionMemberKind::ValueRange {
                    provider,
                    value_type,
                } => Some((provider.as_str(), *value_type)),
                _ => None,
            })
            .collect();
        let mut bindings = HashMap::new();
        for field in fields {
            let invalid = || SolutionBindingError::InvalidFieldRange {
                entity: field.entity_type,
                variable: field.variable.clone(),
            };
            let variable = self
                .schema()
                .entity_type(field.entity_type)
                .and_then(|e| e.genuine_variable(&field.variable))
                .map_err(|_| invalid())?;
            if field.providers.is_empty()
                || bindings.contains_key(&(field.entity_type, field.variable.clone()))
            {
                return Err(invalid());
            }
            for name in &field.providers {
                let value_type = providers
                    .get(name.as_str())
                    .ok_or_else(|| SolutionBindingError::UnknownProvider(name.clone()))?;
                if *value_type != variable.value_type() {
                    return Err(invalid());
                }
            }
            bindings.insert((field.entity_type, field.variable.clone()), field);
        }
        let mut descriptors = vec![];
        let mut indices = HashMap::new();
        for entity in self.schema().entities() {
            for variable in entity.genuine_variables() {
                let key = (entity.entity_type(), variable.name().into());
                let definition = bindings.remove(&key).ok_or_else(|| {
                    SolutionBindingError::MissingFieldRange {
                        entity: entity.entity_type(),
                        variable: variable.name().into(),
                    }
                })?;
                let ordinal = descriptors.len();
                indices.insert(key, ordinal);
                descriptors.push(SolutionVariableRange {
                    layout: self,
                    variable,
                    definition,
                    ordinal,
                });
            }
        }
        Ok(SolutionVariableRanges {
            descriptors,
            indices,
        })
    }
}
