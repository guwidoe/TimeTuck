//! Per-world shadow registrations rebuilt only for explicit membership changes.
use super::*;
use crate::{ShadowSession, ShadowSessionBuilder, ShadowSessionCheckpoint, ShadowSessionError};
use std::{cell::RefCell, rc::Weak};
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
    lease: Rc<ProgramLease>,
    session: ShadowSessionCheckpoint,
}
// Only the last live or checkpoint reference retires a registration. The arena
// owns the queue, not the leases, so saved state cannot keep a dead arena alive.
struct ProgramLease {
    slot: usize,
    retired: Weak<RefCell<Vec<usize>>>,
}
impl Drop for ProgramLease {
    fn drop(&mut self) {
        if let Some(retired) = self.retired.upgrade() {
            retired.borrow_mut().push(self.slot);
        }
    }
}
struct ProgramSlot<P> {
    lease: Weak<ProgramLease>,
    program: P,
}
struct ProgramArena<P> {
    slots: Vec<Option<ProgramSlot<P>>>,
    free: Vec<usize>,
    retired: Rc<RefCell<Vec<usize>>>,
}
impl<P> ProgramArena<P> {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            retired: Rc::new(RefCell::new(Vec::new())),
        }
    }
    fn reclaim(&mut self) {
        loop {
            // Release the queue borrow before dropping any user-owned program
            // data. If that drop panics, remaining retirement events stay queued.
            let Some(index) = self.retired.borrow_mut().pop() else {
                break;
            };
            let slot = self.slots[index].take().expect("retired program slot");
            debug_assert_eq!(slot.lease.strong_count(), 0);
            self.free.push(index);
            drop(slot);
        }
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
    }
    fn insert(&mut self, program: P) -> (usize, Rc<ProgramLease>) {
        self.reclaim();
        // Tail trimming may leave free indices beyond the new length. Each
        // retired index is pushed and inspected at most once before reuse.
        let index = loop {
            match self.free.pop() {
                Some(index) if index < self.slots.len() => break index,
                Some(_) => continue,
                None => break self.slots.len(),
            }
        };
        let lease = Rc::new(ProgramLease {
            slot: index,
            retired: Rc::downgrade(&self.retired),
        });
        let slot = Some(ProgramSlot {
            lease: Rc::downgrade(&lease),
            program,
        });
        if index == self.slots.len() {
            self.slots.push(slot);
        } else {
            debug_assert!(self.slots[index].is_none());
            self.slots[index] = slot;
        }
        (index, lease)
    }
    fn get_mut(&mut self, index: usize, lease: &Rc<ProgramLease>) -> &mut P {
        let slot = self.slots[index].as_mut().expect("live program lease");
        debug_assert!(Weak::ptr_eq(&slot.lease, &Rc::downgrade(lease)));
        &mut slot.program
    }
}
type Factory<'a, 's, M, K, W, U> =
    dyn Fn(&M) -> Result<ShadowProgram<'a, 's, K, W, U>, ShadowSessionError> + 'a;
struct DynamicShadows<'a, 's, I, M, K, W, U> {
    membership: Box<dyn Fn(&W) -> M + 'a>,
    factory: Box<Factory<'a, 's, M, K, W, U>>,
    states: SnapshotMap<I, State<M>>,
    programs: ProgramArena<ShadowProgram<'a, 's, K, W, U>>,
}
impl<I: Ord + Clone + 'static, M: Eq + 'static, K: Eq, W: 'static, U: 'static, S: AdditiveScore>
    Node<S> for DynamicShadows<'_, '_, I, M, K, W, U>
{
    fn clear(&mut self) {
        self.states.clear();
        self.programs.reclaim();
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
            let program = self.programs.get_mut(old.slot, &old.lease);
            program.session.install(&old.session);
            let value = program
                .evaluate(world)
                .map_err(|e| GraphError::Dependency(e.to_string()))?;
            let next = State {
                membership,
                slot: old.slot,
                lease: old.lease.clone(),
                session: program.session.capture(),
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
            let (index, lease) = self.programs.insert(program);
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
    /// keep their programs alive. Last-lease drops queue retirement in amortized
    /// constant time. Reclamation visits retired slots only, and slot reuse is
    /// amortized constant time; unrelated live programs are not scanned. Arena
    /// metadata retains capacity for its high-water registration count.
    /// Errors/panics poison the transaction and prevent partial scores.
    /// External callback effects and panicking destructors are
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
            programs: ProgramArena::new(),
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
    #[test]
    fn arena_retires_only_after_the_last_live_or_snapshot_lease() {
        let live = Cell::new(0);
        let mut arena = ProgramArena::new();
        let mut states = SnapshotMap::new();
        for id in 0..1024 {
            live.set(live.get() + 1);
            let (slot, lease) = arena.insert(Live(&live));
            states.insert(id, (slot, lease));
        }
        assert!(arena.retired.borrow().is_empty());
        assert!(arena.free.is_empty());
        let saved = states.clone();
        for id in 0..1024 {
            if id % 2 == 0 {
                states.remove(&id);
            }
        }
        arena.reclaim();
        assert_eq!(live.get(), 1024);
        assert!(arena.retired.borrow().is_empty());
        drop(saved);
        assert_eq!(arena.retired.borrow().len(), 512);
        arena.reclaim();
        assert_eq!(live.get(), 512);
        assert_eq!(arena.free.len(), 512);
        for (_, (slot, lease)) in &states {
            assert!(arena.get_mut(*slot, lease).alive());
        }
        for _ in 0..512 {
            live.set(live.get() + 1);
            let (slot, lease) = arena.insert(Live(&live));
            assert!(arena.get_mut(slot, &lease).alive());
            // The next insert reclaims this one slot; all other leases survive.
            drop(lease);
        }
        arena.reclaim();
        assert_eq!(live.get(), 512);
        assert!(arena.slots.len() <= 1024);
        states.clear();
        arena.reclaim();
        assert_eq!(live.get(), 0);
        assert!(arena.slots.is_empty());
        live.set(1);
        let (slot, lease) = arena.insert(Live(&live));
        assert_eq!(slot, 0);
        assert!(arena.free.is_empty());
        drop(arena);
        assert_eq!(live.get(), 0);
        // A lease can outlive its arena without retaining programs or a queue.
        assert!(lease.retired.upgrade().is_none());
        drop(lease);
    }
    #[test]
    fn arena_keeps_unprocessed_retirements_after_a_program_drop_panics() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        struct Guard<'a> {
            panic: &'a Cell<bool>,
            drops: &'a Cell<usize>,
        }
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.drops.set(self.drops.get() + 1);
                assert!(!self.panic.replace(false), "program drop panic");
            }
        }
        let panic = Cell::new(false);
        let drops = Cell::new(0);
        let mut arena = ProgramArena::new();
        let mut leases = Vec::new();
        for _ in 0..3 {
            leases.push(
                arena
                    .insert(Guard {
                        panic: &panic,
                        drops: &drops,
                    })
                    .1,
            );
        }
        drop(leases);
        panic.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| arena.reclaim())).is_err());
        assert_eq!(drops.get(), 1);
        assert_eq!(arena.retired.borrow().len(), 2);
        arena.reclaim();
        assert_eq!(drops.get(), 3);
        assert!(arena.slots.is_empty());
    }
}
