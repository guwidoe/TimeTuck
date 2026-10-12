//! Checked provider construction from declared basic fields and aligned slots.
use crate::{
    EntitySwapOrder, EntitySwapProvider, PillarChangeMode, PillarMoveProvider, PillarSchema,
    PillarSelectionPolicy, PlanningVariableKind, SampledSubListNeighborhood,
    SampledSubListProvider, SchemaMoveError, SchemaVariableSlots, ScopedPillarMoveProvider,
    SubListChangeConfig,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldProviderError {
    EmptyFields,
    Schema(SchemaMoveError),
}
impl fmt::Display for FieldProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFields => f.write_str("selected variable list is empty"),
            Self::Schema(error) => error.fmt(f),
        }
    }
}
impl Error for FieldProviderError {}

fn field_provider_schema(
    fields: &[SchemaVariableSlots<'_>],
) -> Result<PillarSchema, FieldProviderError> {
    if fields.is_empty() {
        return Err(FieldProviderError::EmptyFields);
    }
    PillarSchema::from_fields(fields).map_err(FieldProviderError::Schema)
}

impl EntitySwapProvider {
    /// Select one declared column. Entity orders refer to binding encounter order.
    pub fn from_planning_field(
        field: &SchemaVariableSlots<'_>,
        order: EntitySwapOrder,
    ) -> Result<Self, FieldProviderError> {
        Self::from_planning_fields(std::slice::from_ref(field), order)
    }
    /// Select nonempty aligned basic columns from one entity declaration. Validate
    /// metadata, duplicate fields and aliased slots before constructing a provider.
    /// Bindings must contain entity slots. Problem slot bounds and assignment
    /// compatibility are checked at opening as usual.
    pub fn from_planning_fields(
        fields: &[SchemaVariableSlots<'_>],
        order: EntitySwapOrder,
    ) -> Result<Self, FieldProviderError> {
        field_provider_schema(fields)?;
        // Entity swap order is positional; do not substitute canonical pillar rows.
        let rows = (0..fields[0].slots.len())
            .map(|i| fields.iter().map(|field| field.slots[i]).collect())
            .collect();
        Self::new(rows, order).map_err(|error| {
            FieldProviderError::Schema(SchemaMoveError::InvalidModel(error.to_string()))
        })
    }
}

impl ScopedPillarMoveProvider {
    /// Whole-pillar swaps on one declared basic field.
    pub fn swap_field(field: &SchemaVariableSlots<'_>) -> Result<Self, FieldProviderError> {
        Self::swap_fields(std::slice::from_ref(field))
    }
    /// Whole-pillar swaps on nonempty aligned basic fields. Grouping and moves
    /// involve only these columns, independently of the problem's default schema.
    pub fn swap_fields(fields: &[SchemaVariableSlots<'_>]) -> Result<Self, FieldProviderError> {
        Self::subpillar_swap_fields(
            fields,
            PillarSelectionPolicy::whole(),
            PillarSelectionPolicy::whole(),
        )
    }
    pub fn subpillar_swap_field(
        field: &SchemaVariableSlots<'_>,
        left: PillarSelectionPolicy,
        right: PillarSelectionPolicy,
    ) -> Result<Self, FieldProviderError> {
        Self::subpillar_swap_fields(std::slice::from_ref(field), left, right)
    }
    /// Independent left/right subset policies; both orientations are retained.
    pub fn subpillar_swap_fields(
        fields: &[SchemaVariableSlots<'_>],
        left: PillarSelectionPolicy,
        right: PillarSelectionPolicy,
    ) -> Result<Self, FieldProviderError> {
        let schema = field_provider_schema(fields)?;
        Ok(PillarMoveProvider {
            left,
            right,
            changes: PillarChangeMode::Disabled,
            swaps: true,
        }
        .with_schema(schema))
    }
    /// Explicit unassignment requires every selected field to be optional in the
    /// declaration. Opening still checks the actual model's receiving domains.
    pub fn subpillar_change_fields(
        fields: &[SchemaVariableSlots<'_>],
        selection: PillarSelectionPolicy,
        crossing_unassigned: bool,
    ) -> Result<Self, FieldProviderError> {
        let schema = field_provider_schema(fields)?;
        if crossing_unassigned
            && let Some(field) = fields.iter().find(|field| {
                field.variable.kind()
                    != (PlanningVariableKind::Basic {
                        allows_unassigned: true,
                    })
            })
        {
            return Err(FieldProviderError::Schema(
                SchemaMoveError::CrossingUnassigned(field.variable.name().into()),
            ));
        }
        Ok(PillarMoveProvider {
            left: selection,
            changes: if crossing_unassigned {
                PillarChangeMode::All
            } else {
                PillarChangeMode::AssignedOnly
            },
            swaps: false,
            ..Default::default()
        }
        .with_schema(schema))
    }
}

impl SampledSubListProvider {
    /// Default bounded change sampling: spans of 2 through 10 values. Explicit
    /// configurations passed to `new` retain their own bounds, including larger
    /// spans. Seed, attempt ceiling and existing native sampling semantics apply.
    pub fn default_change(seed: u64, attempts_per_step: u64) -> Self {
        Self::new(
            SampledSubListNeighborhood::Change(
                SubListChangeConfig::new(2, 10).expect("valid default bounds"),
            ),
            seed,
            attempts_per_step,
        )
    }
}
