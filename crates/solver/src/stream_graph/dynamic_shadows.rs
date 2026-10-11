//! Per-world shadow registrations rebuilt only for explicit membership changes.
use super::*;
use crate::{ShadowSession, ShadowSessionBuilder, ShadowSessionCheckpoint, ShadowSessionError};
use std::rc::Weak;
type Projection<'a, 's, K, W, U> =
    dyn Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a;
/// A complete registration and its projection, constructed for one membership
/// signature by [`StreamGraphBuilder::map_dynamic_shadows`]. Projection may capture
/// the typed handles created by this builder. Construction validates fixed source
/// dependencies but invokes no readers, suppliers or projections.
pub struct ShadowProgram<'a, 's, K, W, U> {
    session: ShadowSession<'a, 's, K, W>,
    project: Box<Projection<'a, 's, K, W, U>>,
}
impl<'a, 's, K: Eq, W, U> ShadowProgram<'a, 's, K, W, U> {
    pub fn new(
        builder: ShadowSessionBuilder<'a, 's, K, W>,
        project: impl Fn(&W, &ShadowSession<'a, 's, K, W>) -> Result<U, ShadowSessionError> + 'a,
    ) -> Result<Self, ShadowSessionError> {
        Ok(Self {
            session: builder.try_build()?,
            project: Box::new(project),
        })
    }
    fn evaluate(&mut self, world: &W) -> Result<U, ShadowSessionError> {
        self.session.update(world)?;
        (self.project)(world, &self.session)
    }
}
struct State<M> {
    membership: M,
    slot: usize,
    lease: Rc<()>,
    session: ShadowSessionCheckpoint,
}
struct ProgramSlot<'a, 's, K, W, U> {
    lease: Weak<()>,
    program: ShadowProgram<'a, 's, K, W, U>,
}
type Factory<'a, 's, M, K, W, U> =
    dyn Fn(&M) -> Result<ShadowProgram<'a, 's, K, W, U>, ShadowSessionError> + 'a;
struct DynamicShadows<'a, 's, I, M, K, W, U> {
    membership: Box<dyn Fn(&W) -> M + 'a>,
    factory: Box<Factory<'a, 's, M, K, W, U>>,
    states: SnapshotMap<I, State<M>>,
    programs: Vec<Option<ProgramSlot<'a, 's, K, W, U>>>,
}
impl<K, W, U, I, M> DynamicShadows<'_, '_, I, M, K, W, U> {
    fn reclaim(&mut self) {
        for slot in &mut self.programs {
            if slot.as_ref().is_some_and(|s| s.lease.strong_count() == 0) {
                *slot = None;
            }
        }
        while self.programs.last().is_some_and(Option::is_none) {
            self.programs.pop();
        }
    }
}
impl<I: Ord + Clone + 'static, M: Eq + 'static, K: Eq, W: 'static, U: 'static, S: AdditiveScore>
    Node<S> for DynamicShadows<'_, '_, I, M, K, W, U>
{
    fn clear(&mut self) {
        self.states.clear();
        self.reclaim();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(self.states.clone())
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.states = *saved.downcast().expect("dynamic shadow checkpoint");
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<I, W>(event);
        let Some(world) = &change.value else {
            if !self.states.contains_key(&change.id) {
                return Ok(vec![]);
            }
            let output = erase([TupleChange::<I, U>::retract(change.id.clone())]);
            self.states.remove(&change.id);
            return Ok(output);
        };
        let membership = (self.membership)(world);
        let old = self.states.get(&change.id);
        if let Some(old) = old.filter(|old| old.membership == membership) {
            let slot = self.programs[old.slot]
                .as_mut()
                .expect("live program lease");
            debug_assert!(Weak::ptr_eq(&slot.lease, &Rc::downgrade(&old.lease)));
            slot.program.session.install(&old.session);
            let value = slot
                .program
                .evaluate(world)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let next = State {
                membership,
                slot: old.slot,
                lease: old.lease.clone(),
                session: slot.program.session.capture(),
            };
            let output = erase([TupleChange::put(change.id.clone(), value)]);
            self.states.insert(change.id.clone(), next);
            Ok(output)
        } else {
            let mut program =
                (self.factory)(&membership).map_err(|e| GraphError::Dependency(e.to_string()))?;
            let value = program
                .evaluate(world)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let checkpoint = program.session.capture();
            let output = erase([TupleChange::put(change.id.clone(), value)]);
            self.reclaim();
            let index = self
                .programs
                .iter()
                .position(Option::is_none)
                .unwrap_or(self.programs.len());
            let lease = Rc::new(());
            let slot = Some(ProgramSlot {
                lease: Rc::downgrade(&lease),
                program,
            });
            if index == self.programs.len() {
                self.programs.push(slot);
            } else {
                self.programs[index] = slot;
            }
            self.states.insert(
                change.id.clone(),
                State {
                    membership,
                    slot: index,
                    lease,
                    session: checkpoint,
                },
            );
            Ok(output)
        }
    }
}
impl<'a, S: AdditiveScore + 'static> StreamGraphBuilder<'a, S> {
    /// Derive one output per world using membership-specific shadow registrations.
    /// Unchanged signatures reuse that world's session cache. A changed signature
    /// calls the factory once and initializes a fresh registration; the output ID
    /// stays unchanged. Retraction removes the world; reinsertion builds fresh.
    /// The stream graph itself remains fixed and ordinary downstream joins,
    /// collectors and scoring continue to consume the same typed port.
    ///
    /// Signature equality must exactly describe every detail affecting registration
    /// or immutable facts captured by the program. Canonicalize entity order when
    /// order is irrelevant. Mutable facts with unchanged signatures must flow
    /// through world readers/dependencies, never be captured by the factory.
    /// Factories and callbacks must be deterministic from their inputs, not call
    /// counts. Each world ID has independent state even for equal signatures.
    ///
    /// Ordinary updates fully poll registered inputs and dynamic dependencies.
    /// Membership changes rebuild the entire registration and initialize all its
    /// fields; no constant-time domain-change claim is made. Signature extraction
    /// and comparison costs also belong to every world update.
    ///
    /// Transaction checkpoints and saved candidates retain registration leases
    /// and immutable cache snapshots. Restoration invokes no factories, readers,
    /// suppliers or projections, including after several registration switches.
    /// Dead registrations are reclaimed on a later rebuild/clear; live snapshots
    /// keep their programs alive. Errors/panics poison the transaction and prevent
    /// partial scores. External callback effects and panicking destructors are
    /// outside graph rollback, as for ordinary map_shadows.
    ///
    /// ```
    /// use timetuck::*;
    /// struct Model; struct Task;
    /// let schema=PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
    ///     PlanningVariableDefinition::basic::<i64>("cost",false),
    /// ])])?;
    /// let mut b=StreamGraphBuilder::<SimpleScore>::new();
    /// let source=b.source::<usize,std::collections::BTreeMap<usize,i64>>();
    /// let costs=b.map_dynamic_shadows(source.port(),
    ///     |world|world.keys().copied().collect::<Vec<_>>(),
    ///     |keys| {
    ///         let mut builder=ShadowSessionBuilder::<usize,std::collections::BTreeMap<usize,i64>>::new(&schema);
    ///         let mut fields=vec![];
    ///         for &key in keys {
    ///             fields.push(builder.input(schema.entity::<Task>().unwrap().variable("cost").unwrap(),
    ///                 key,move |world|world.get(&key).copied())?);
    ///         }
    ///         ShadowProgram::new(builder,move |_,session|fields.iter()
    ///             .map(|field|Ok(*session.read(field)?.value.unwrap()))
    ///             .sum::<Result<i64,ShadowSessionError>>())
    ///     })?;
    /// b.score("cost",&costs,|value|SimpleScore::new(*value))?;
    /// let mut graph=b.build();
    /// let mut tx=graph.transaction();
    /// tx.apply(&source,TupleChange::put(0,[(7,3)].into_iter().collect()))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(3));
    /// tx.apply(&source,TupleChange::put(0,[(7,3),(9,5)].into_iter().collect()))?;
    /// assert_eq!(tx.score()?,SimpleScore::new(8));
    /// tx.commit()?;
    /// graph.assert_consistent()?;
    /// # Ok::<(),Box<dyn std::error::Error>>(())
    /// ```
    pub fn map_dynamic_shadows<'s: 'a, I, M, K, W, U>(
        &mut self,
        input: &StreamPort<I, W>,
        membership: impl Fn(&W) -> M + 'a,
        factory: impl Fn(&M) -> Result<ShadowProgram<'a, 's, K, W, U>, ShadowSessionError> + 'a,
    ) -> Result<StreamPort<I, U>, GraphError>
    where
        I: Ord + Clone + 'static,
        M: Eq + 'static,
        K: Eq + 'a,
        W: Clone + 'static,
        U: Clone + 'static,
    {
        self.validate(input)?;
        let out = self.push(DynamicShadows {
            membership: Box::new(membership),
            factory: Box::new(factory),
            states: SnapshotMap::<I, State<M>>::new(),
            programs: vec![],
        });
        self.connect(input.node, out.node, false);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        PlanningEntityDefinition, PlanningSchema, PlanningVariableDefinition, SimpleScore,
    };
    use std::cell::Cell;
    struct Model;
    struct Task;
    struct Live<'a>(&'a Cell<usize>);
    impl Live<'_> {
        fn alive(&self) -> bool {
            self.0.get() > 0
        }
    }
    impl Drop for Live<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    #[test]
    fn saved_candidates_pin_programs_across_switches_retractions_and_slot_reuse() {
        let schema =
            PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
                PlanningVariableDefinition::basic::<i64>("value", false),
            ])])
            .unwrap();
        let factories = Cell::new(0);
        let projections = &Cell::new(0);
        let live = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (Vec<usize>, i64)>();
        let output = b
            .map_dynamic_shadows(
                source.port(),
                |world| world.0.clone(),
                |keys| {
                    factories.set(factories.get() + 1);
                    live.set(live.get() + 1);
                    let guard = Live(&live);
                    let mut builder =
                        ShadowSessionBuilder::<usize, (Vec<usize>, i64)>::new(&schema);
                    let mut fields = vec![];
                    for &key in keys {
                        let node = builder.input(
                            schema.entity::<Task>().unwrap().variable("value").unwrap(),
                            key,
                            move |w| Some(w.1 + key as i64),
                        )?;
                        fields.push(node);
                    }
                    ShadowProgram::new(builder, move |_, session| {
                        assert!(guard.alive());
                        projections.set(projections.get() + 1);
                        fields
                            .iter()
                            .map(|field| Ok(*session.read(field)?.value.unwrap()))
                            .sum::<Result<i64, ShadowSessionError>>()
                    })
                },
            )
            .unwrap();
        let data = b.dataset(&output).unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, (vec![1, 2], 3)))
            .unwrap();
        tx.apply(&source, TupleChange::put(8, (vec![5], 7)))
            .unwrap();
        let saved = tx.capture().unwrap();
        drop(tx);
        for key in 20..50 {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(7, (vec![key], 0)))
                .unwrap();
            tx.apply(&source, TupleChange::retract(8)).unwrap();
            tx.commit().unwrap();
            assert!(
                live.get() <= 4,
                "saved programs plus current/retired registration"
            );
        }
        let count = (factories.get(), projections.get());
        graph.install(saved);
        assert_eq!((factories.get(), projections.get()), count);
        assert_eq!(graph.dataset_rows(&data).unwrap()[&7], 9);
        assert_eq!(graph.dataset_rows(&data).unwrap()[&8], 12);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(7, (vec![1, 2], 4)))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(factories.get(), count.0);
        assert_eq!(graph.dataset_rows(&data).unwrap()[&7], 11);
        // Drop the old saved registration leases, then force reclamation.
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(7)).unwrap();
        tx.apply(&source, TupleChange::retract(8)).unwrap();
        tx.commit().unwrap();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(9, (vec![100], 0)))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(live.get(), 1);
        graph.assert_consistent().unwrap();
        drop(graph);
        assert_eq!(live.get(), 0);
    }
}
