//! Independently scoped pillar neighborhoods for heterogeneous entity models.
use crate::{
    Assignment, FiniteProblem, ListMove, MixedAssignment, MixedMove, MixedProblem, ModelError,
    Move, MoveProvider, PillarMoveProvider, PillarSchema,
};

impl PillarSchema {
    /// Build aligned rows from genuine basic fields of one entity declaration.
    /// This validates metadata and alignment, not a problem's value types or
    /// ranges. Slot bounds are checked when the schema is used against a problem.
    /// Field order defines column order; slot order aligns the entity instances.
    pub fn from_fields(
        fields: &[crate::SchemaVariableSlots<'_>],
    ) -> Result<Self, crate::SchemaMoveError> {
        use crate::{PlanningVariableKind, SchemaMoveError};
        let first = fields.first().ok_or(SchemaMoveError::NoMatchingVariables)?;
        let entity = first.variable.entity();
        let mut slots = std::collections::BTreeSet::new();
        for (i, field) in fields.iter().enumerate() {
            if !std::ptr::eq(field.variable.entity().solution(), entity.solution()) {
                return Err(SchemaMoveError::ForeignSchema);
            }
            if field.variable.entity() != entity {
                return Err(SchemaMoveError::WrongModel);
            }
            if !matches!(field.variable.kind(), PlanningVariableKind::Basic { .. }) {
                return Err(SchemaMoveError::WrongModel);
            }
            if fields[..i]
                .iter()
                .any(|prior| prior.variable == field.variable)
            {
                return Err(SchemaMoveError::DuplicateBinding(
                    field.variable.name().into(),
                ));
            }
            if field.slots.len() != first.slots.len() {
                return Err(SchemaMoveError::BindingLength(field.variable.name().into()));
            }
            for &slot in &field.slots {
                if !slots.insert(slot) {
                    return Err(SchemaMoveError::AliasedSlot { list: false, slot });
                }
            }
        }
        Self::new(
            (0..first.slots.len())
                .map(|i| fields.iter().map(|f| f.slots[i]).collect())
                .collect(),
        )
        .map_err(|e| SchemaMoveError::InvalidModel(e.to_string()))
    }
}

/// A pillar policy with its own selected entity rows and variable columns.
/// Multiple instances can be composed through `ProviderUnion`, even when their
/// schemas overlap or the problem carries a different default schema. Each
/// instance groups only its own rows anew on every opening. Columns, pins,
/// ranges, optional values and subpillar comparators retain the ordinary pillar
/// semantics. No problem or planning values are cloned when opening candidates.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4])?;
/// let provider = PillarMoveProvider { swaps: false, ..Default::default() }
///     .with_schema(PillarSchema::new(vec![vec![0], vec![2]])?);
/// let a = p.initial_assignment();
/// let moves: Vec<_> = provider.select(&p, &a)?.collect();
/// assert_eq!(moves.len(), 1);
/// let mut next = a.clone();
/// p.apply(&mut next, &moves[0])?.commit();
/// assert_eq!(next.values(), &[Some(1), Some(0), Some(1), Some(0)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedPillarMoveProvider {
    pub policy: PillarMoveProvider,
    pub schema: PillarSchema,
}
impl PillarMoveProvider {
    pub fn with_schema(self, schema: PillarSchema) -> ScopedPillarMoveProvider {
        ScopedPillarMoveProvider {
            policy: self,
            schema,
        }
    }
}
impl ScopedPillarMoveProvider {
    /// Validate both the full assignment and this provider's slot bounds before
    /// yielding candidates. The schema attached to the problem is not consulted.
    pub fn select<'a, V: Eq + Clone>(
        &'a self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
    ) -> Result<impl Iterator<Item = Move<V>> + 'a, ModelError> {
        Ok(self.policy.select_groups(p, a, self.schema.groups(p, a)?))
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for ScopedPillarMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        Box::new(
            self.select(p, a)
                .expect("scoped pillar provider requires valid slots and assignment"),
        )
    }
}
impl<B: Eq + Clone, L: Eq + Clone>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for ScopedPillarMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        p.validate(a)
            .expect("scoped pillar provider requires a valid mixed assignment");
        Box::new(
            self.select(p.basic(), a.basic())
                .expect("scoped pillar provider requires valid slots")
                .map(|basic| MixedMove {
                    basic,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
