//! Explicit semantic group-move identity, separate from executable write snapshots.
use crate::{
    Assignment, MassChange, Move, Pillar, PillarError, PlanningVariableKind, PlanningVariableRef,
};
use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    hash::{Hash, Hasher},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupMoveMetadataError {
    Pillar(PillarError),
    NotBasic { name: String },
    DuplicateVariable { name: String },
}
impl fmt::Display for GroupMoveMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "group move metadata: {self:?}")
    }
}
impl Error for GroupMoveMetadataError {}
impl From<PillarError> for GroupMoveMetadataError {
    fn from(error: PillarError) -> Self {
        Self::Pillar(error)
    }
}
fn validate(variable: PlanningVariableRef<'_>) -> Result<(), GroupMoveMetadataError> {
    if matches!(variable.kind(), PlanningVariableKind::Basic { .. }) {
        Ok(())
    } else {
        Err(GroupMoveMetadataError::NotBasic {
            name: variable.name().into(),
        })
    }
}
fn hash_variable(variable: PlanningVariableRef<'_>, state: &mut impl Hasher) {
    std::ptr::from_ref(variable.entity().solution()).hash(state);
    variable.entity().ordinal().hash(state);
    variable.name().hash(state);
}
fn label(variable: PlanningVariableRef<'_>, entity_name: Option<&str>) -> String {
    format!(
        "{}.{}",
        entity_name.unwrap_or(variable.entity().entity_type().name()),
        variable.name()
    )
}
/// Ordered selected basic metadata and oriented pillars. Row order inside each
/// pillar is canonical; left/right orientation and column metadata remain significant.
/// Equality does not read an assignment or compare generated writes. Native callers
/// bind each metadata column to the corresponding column of slot rows. Execution
/// regenerates a real ordinary move against the current assignment; existing search
/// and tabu policies do not use this optional identity automatically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PillarSwapPlan<'s> {
    variables: Vec<PlanningVariableRef<'s>>,
    left: Pillar,
    right: Pillar,
}
impl Hash for PillarSwapPlan<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.variables.len().hash(state);
        for &variable in &self.variables {
            hash_variable(variable, state);
        }
        self.left.hash(state);
        self.right.hash(state);
    }
}
impl<'s> PillarSwapPlan<'s> {
    pub fn new(
        variables: Vec<PlanningVariableRef<'s>>,
        left: Pillar,
        right: Pillar,
    ) -> Result<Self, GroupMoveMetadataError> {
        for pillar in [&left, &right] {
            if pillar.column_count() != variables.len() {
                return Err(PillarError::Arity {
                    expected: pillar.column_count(),
                    actual: variables.len(),
                }
                .into());
            }
        }
        for (index, &variable) in variables.iter().enumerate() {
            validate(variable)?;
            if variables[..index].contains(&variable) {
                return Err(GroupMoveMetadataError::DuplicateVariable {
                    name: variable.name().into(),
                });
            }
        }
        let right_variables: BTreeSet<_> = right.variables().into_iter().collect();
        if let Some(variable) = left
            .variables()
            .into_iter()
            .find(|v| right_variables.contains(v))
        {
            return Err(PillarError::Overlap { variable }.into());
        }
        Ok(Self {
            variables,
            left,
            right,
        })
    }
    pub fn variables(&self) -> &[PlanningVariableRef<'s>] {
        &self.variables
    }
    pub fn left(&self) -> &Pillar {
        &self.left
    }
    pub fn right(&self) -> &Pillar {
        &self.right
    }
    pub fn movement<V: Eq + Clone>(&self, current: &Assignment<V>) -> Result<Move<V>, PillarError> {
        self.left.swap_move(&self.right, current)
    }
    /// Default labels are native Rust type names. An optional application label
    /// replaces the entity type's display name only, without changing identity.
    pub fn describe(&self, entity_name: Option<&str>) -> String {
        format!(
            "PillarSwapMove({})",
            self.variables
                .iter()
                .map(|&v| label(v, entity_name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
/// A real common-destination move paired with its selected basic metadata.
/// Labels are diagnostics; semantic equality includes schema variable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedMassChange<'s, V> {
    variable: PlanningVariableRef<'s>,
    change: MassChange<V>,
}
impl<V: Hash> Hash for NamedMassChange<'_, V> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_variable(self.variable, state);
        self.change.hash(state);
    }
}
impl<'s, V> NamedMassChange<'s, V> {
    pub fn new(
        variable: PlanningVariableRef<'s>,
        change: MassChange<V>,
    ) -> Result<Self, GroupMoveMetadataError> {
        validate(variable)?;
        Ok(Self { variable, change })
    }
    pub fn variable(&self) -> PlanningVariableRef<'s> {
        self.variable
    }
    pub fn change(&self) -> &MassChange<V> {
        &self.change
    }
    pub fn movement(&self) -> &Move<V> {
        self.change.movement()
    }
    pub fn into_move(self) -> Move<V> {
        self.change.into_move()
    }
    pub fn describe(&self, entity_name: Option<&str>) -> String {
        format!("MassChangeMove({})", label(self.variable, entity_name))
    }
}
