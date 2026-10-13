//! Descriptor occurrence distance over actual registered genuine-field snapshots.
use crate::{
    ComposedPlanningSchema, PlanningType, PlanningVariableRef, SolutionSnapshot,
    SolutionTrackingError,
};
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DescriptorMutationError {
    UnknownDescriptor(PlanningType),
    DuplicateDescriptor(PlanningType),
    MissingSnapshotField { entity: PlanningType, field: String },
    InconsistentComposition,
    Snapshot(SolutionTrackingError),
}
impl fmt::Display for DescriptorMutationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "descriptor mutation views: {self:?}")
    }
}
impl Error for DescriptorMutationError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorFieldObservation<'s> {
    pub descriptor: PlanningType,
    pub descriptor_field: PlanningVariableRef<'s>,
    pub concrete_field: PlanningVariableRef<'s>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorMutationReport {
    pub unique_changes: usize,
    pub descriptor_changes: Vec<(PlanningType, usize)>,
}
impl DescriptorMutationReport {
    pub fn occurrence_count(&self) -> usize {
        self.descriptor_changes.iter().map(|(_, n)| n).sum()
    }
}
/// Compile real registered descriptor matching views from explicit composition
/// ancestry and inherited declaring-field identity. Each matching descriptor
/// contributes one observation of its genuine field on a concrete entity. Thus
/// an inherited child field may appear in both base and child views. Ordinary
/// SolutionSnapshot::mutation_count remains unique registered-field/entity distance.
/// No numeric weights, duplicated fixture readers or runtime class discovery are
/// used. Both snapshots must share actual tracker registration and include every
/// concrete field required by these views. Lists count once per field occurrence.
pub struct DescriptorMutationViews<'s> {
    descriptors: Vec<PlanningType>,
    observations: Vec<DescriptorFieldObservation<'s>>,
}
impl<'s> DescriptorMutationViews<'s> {
    pub fn all(composition: &'s ComposedPlanningSchema) -> Result<Self, DescriptorMutationError> {
        let types = composition
            .schema()
            .genuine_entities()
            .map(|e| e.entity_type())
            .collect::<Vec<_>>();
        Self::new(composition, &types)
    }
    pub fn new(
        composition: &'s ComposedPlanningSchema,
        descriptors: &[PlanningType],
    ) -> Result<Self, DescriptorMutationError> {
        let mut observations = Vec::new();
        for (index, &descriptor) in descriptors.iter().enumerate() {
            if descriptors[..index].contains(&descriptor) {
                return Err(DescriptorMutationError::DuplicateDescriptor(descriptor));
            }
            let entity = composition
                .schema()
                .entity_type(descriptor)
                .map_err(|_| DescriptorMutationError::UnknownDescriptor(descriptor))?;
            if !entity.is_genuine() {
                return Err(DescriptorMutationError::UnknownDescriptor(descriptor));
            }
            for concrete in composition
                .schema()
                .genuine_entities()
                .filter(|c| composition.matches_descriptor(c.entity_type(), descriptor))
            {
                for field in entity.genuine_variables() {
                    let observed = concrete
                        .genuine_variable(field.name())
                        .map_err(|_| DescriptorMutationError::InconsistentComposition)?;
                    if observed.kind() != field.kind()
                        || observed.value_type() != field.value_type()
                        || composition.field_origin(concrete.entity_type(), observed.name())
                            != composition.field_origin(descriptor, field.name())
                    {
                        return Err(DescriptorMutationError::InconsistentComposition);
                    }
                    observations.push(DescriptorFieldObservation {
                        descriptor,
                        descriptor_field: field,
                        concrete_field: observed,
                    });
                }
            }
        }
        Ok(Self {
            descriptors: descriptors.to_vec(),
            observations,
        })
    }
    pub fn observations(&self) -> &[DescriptorFieldObservation<'s>] {
        &self.observations
    }
    pub fn compare<E: Ord + Clone>(
        &self,
        before: &SolutionSnapshot<'s, E>,
        after: &SolutionSnapshot<'s, E>,
    ) -> Result<DescriptorMutationReport, DescriptorMutationError> {
        let changes = before
            .changed_variables(after)
            .map_err(DescriptorMutationError::Snapshot)?;
        let registered = before.registered_fields().collect::<Vec<_>>();
        for observation in &self.observations {
            if !registered.contains(&observation.concrete_field) {
                return Err(DescriptorMutationError::MissingSnapshotField {
                    entity: observation.concrete_field.entity().entity_type(),
                    field: observation.concrete_field.name().into(),
                });
            }
        }
        let mut report = DescriptorMutationReport {
            unique_changes: 0,
            descriptor_changes: self
                .descriptors
                .iter()
                .map(|&descriptor| (descriptor, 0))
                .collect(),
        };
        for (field, _) in changes {
            report.unique_changes += 1;
            for observation in self
                .observations
                .iter()
                .filter(|o| o.concrete_field == field)
            {
                let entry = report
                    .descriptor_changes
                    .iter_mut()
                    .find(|(descriptor, _)| *descriptor == observation.descriptor)
                    .expect("compiled descriptor");
                entry.1 += 1;
            }
        }
        Ok(report)
    }
}
