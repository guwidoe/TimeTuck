//! Queued construction of entities represented by several basic variable slots.
use crate::{
    Assignment, BasicEntityRows, BasicEntityRowsError, FiniteProblem, ListMove, MixedAssignment,
    MixedMove, MixedProblem, Move, MoveProduct, MoveProvider, OptionalConstruction,
    PlacementMoveFactory, PlacementQueueOrder, QueuedMoveProvider,
};
use std::{marker::PhantomData, sync::Arc};

/// How the selected variables of one entity compete during construction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EntityConstructionPlacement {
    /// Finish each missing variable in column order, then advance to the next row.
    #[default]
    SeparateVariables,
    /// Score the Cartesian product of missing columns as atomic composite moves.
    /// The last column varies fastest. A placement initializes the entire eligible
    /// row, allowing constraints between columns to guide the joint choice.
    JointVariables,
}
struct EntityMoves<V> {
    marker: PhantomData<fn() -> V>,
    optional: OptionalConstruction,
}
impl<V: Clone + Eq> PlacementMoveFactory<FiniteProblem<V>, Assignment<V>, Arc<[usize]>, Move<V>>
    for EntityMoves<V>
{
    fn candidates<'a>(
        &mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        row: &Arc<[usize]>,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        let optional = self.optional;
        let variables: Vec<_> = row
            .iter()
            .copied()
            .filter(|&i| {
                let v = &p.variables()[i];
                !v.is_pinned()
                    && a.values()[i].is_none()
                    && (!v.allows_unassigned() || optional != OptionalConstruction::Preserve)
            })
            .collect();
        Box::new(
            MoveProduct::new(variables.len(), false, move |column| {
                let variable = variables[column];
                let v = &p.variables()[variable];
                v.domain()
                    .iter()
                    .map(move |value| {
                        Some(Move::Change {
                            variable,
                            value: Some(value.clone()),
                        })
                    })
                    .chain(v.allows_unassigned().then_some(None))
            })
            .filter_map(|changes| {
                let changes: Vec<_> = changes.into_iter().flatten().collect();
                // The provider construction engine adds its one no-change candidate
                // for EvaluateUnassigned. Do not duplicate it as an empty product.
                (!changes.is_empty()).then_some(Move::Composite(changes))
            }),
        )
    }
}

/// Ready-to-use construction provider for multi-variable basic entities.
/// Rows are disjoint, nonempty selected variable lists in entity/column order;
/// rows may have different widths and may omit model variables. Invalid, duplicate
/// and out-of-bounds slots are rejected at creation. Empty row collections are valid.
///
/// Existing values and pinned slots remain untouched. Pins apply to individual
/// slots; represent an entity pin by pinning all its slots. Missing optional slots
/// are skipped under Preserve. Both other policies offer leaving an optional
/// column empty within a joint move, even when its domain is empty. The engine
/// evaluates the whole composite score; EvaluateUnassigned additionally scores its
/// single final no-change choice. NonWorsening preserves the base if no candidate
/// qualifies. Structural progress dominates ordinary score levels as usual.
/// Pass the same optional policy to the construction configuration. When optional
/// slots remain after queue exhaustion, the existing EvaluateUnassigned engine
/// may score a final no-change probe, counted as an additional placement.
///
/// Uses only linear row/cursor storage; the joint product is generated lazily.
/// Providers bind to a particular problem. Repeating a completed-step counter
/// retries the same placement; advancing it moves to the next one. Decreasing it
/// rewinds. Call reset for a new phase with the same counter or to revisit exhausted
/// rows. Empty/fully initialized rows are skipped. All engine budgets, atomic scan
/// discard, score assertions and construction step limits apply to one placement.
///
/// Implements both basic and mixed MoveProvider; direct mixed use changes only
/// basic slots. Use MixedConstructionProvider to also schedule list construction.
/// No Java listener lifecycle or object-layout equivalence is implied.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1,2]); 4])?;
/// let n = ConstraintNetwork::<_,SimpleScore>::new(4);
/// let mut provider = EntityConstruction::new(&p, vec![vec![0,1],vec![2,3]],
///     EntityConstructionPlacement::JointVariables, OptionalConstruction::Preserve)?;
/// let r = construction_from_with_provider(&p,&n,p.initial_assignment(),
///     ProviderConstructionConfig { max_steps:Some(1), ..Default::default() }, &mut provider)?;
/// assert_eq!(r.assignment.values(), &[Some(1),Some(1),None,None]);
/// assert_eq!(r.completed_steps,1);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct EntityConstruction<'p, V> {
    problem: &'p FiniteProblem<V>,
    queue: QueuedMoveProvider<Arc<[usize]>, EntityMoves<V>>,
}
impl<'p, V: Clone + Eq> EntityConstruction<'p, V> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        rows: Vec<Vec<usize>>,
        placement: EntityConstructionPlacement,
        optional: OptionalConstruction,
    ) -> Result<Self, BasicEntityRowsError> {
        BasicEntityRows::new(problem.variables().len(), rows.clone())?;
        let keys = match placement {
            EntityConstructionPlacement::SeparateVariables => {
                rows.into_iter().flatten().map(|i| Arc::from([i])).collect()
            }
            EntityConstructionPlacement::JointVariables => {
                rows.into_iter().map(Arc::from).collect()
            }
        };
        Ok(Self {
            problem,
            queue: QueuedMoveProvider::new(
                keys,
                PlacementQueueOrder::Once,
                EntityMoves {
                    optional,
                    marker: PhantomData,
                },
            ),
        })
    }
    pub fn reset(&mut self) {
        self.queue.reset();
    }
}
impl<V: Clone + Eq> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for EntityConstruction<'_, V>
{
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.queue.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        assert!(
            std::ptr::eq(p, self.problem),
            "entity construction used with a different problem"
        );
        self.queue.moves(p, a, step)
    }
}
impl<B: Clone + Eq, L: Clone + Eq>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for EntityConstruction<'_, B>
{
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.queue.construction_placement_completed(installed)
    }
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        Box::new(
            self.moves(p.basic(), a.basic(), step)
                .map(|basic| MixedMove {
                    basic,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
