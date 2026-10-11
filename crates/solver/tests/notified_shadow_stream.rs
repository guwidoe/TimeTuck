//! Original regressions for explicit notifications in graph-owned shadow sessions.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Task;
#[derive(Clone, Debug, PartialEq, Eq)]
struct World {
    values: Vec<i64>,
    parents: Vec<Option<usize>>,
    bias: i64,
}
impl World {
    fn new(n: usize) -> Self {
        Self {
            values: vec![1; n],
            parents: vec![None; n],
            bias: 0,
        }
    }
}
#[derive(Eq, PartialEq)]
struct Amount(i64); // Cached values need not implement Clone.
#[derive(Default)]
struct Stats {
    reads: Cell<usize>,
    bindings: Cell<usize>,
    suppliers: Cell<usize>,
    projections: Cell<usize>,
    failure: Cell<u8>,
    events: RefCell<Vec<(World, World)>>,
}
impl Stats {
    fn work(&self) -> (usize, usize, usize) {
        (self.reads.get(), self.bindings.get(), self.suppliers.get())
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
        PlanningVariableDefinition::shadow::<Amount>("amount"),
    ])])
    .unwrap()
}
type Output = Vec<(Option<i64>, bool)>;
fn graph<'a, 's: 'a>(
    schema: &'s PlanningSchema,
    n: usize,
    stats: &'a Stats,
    notified: bool,
) -> (
    StreamGraph<'a, SimpleScore>,
    StreamSource<usize, World>,
    StreamDataset<usize, Output>,
) {
    let mut shadows = ShadowSessionBuilder::<usize, World>::new(schema);
    let fields = schema.entity::<Task>().unwrap();
    let mut inputs = vec![];
    let mut amounts = vec![];
    for k in 0..n {
        let input = shadows
            .input(fields.variable("value").unwrap(), k, move |w| {
                stats.reads.set(stats.reads.get() + 1);
                assert_ne!(stats.failure.get(), 3, "reader");
                Some(w.values[k])
            })
            .unwrap();
        let read = input.clone();
        let amount = shadows
            .derive(fields.variable("amount").unwrap(), k, move |w, values| {
                stats.suppliers.set(stats.suppliers.get() + 1);
                assert_ne!(stats.failure.get(), 5, "supplier");
                Some(Amount(
                    values.get(&read).unwrap().copied().unwrap_or(0)
                        + w.bias
                        + values
                            .for_field::<Amount>(fields.variable("amount").unwrap())
                            .unwrap()
                            .into_iter()
                            .flatten()
                            .map(|a| a.0)
                            .sum::<i64>(),
                ))
            })
            .unwrap();
        inputs.push(input);
        amounts.push(amount);
    }
    for k in 0..n {
        let nodes = amounts.clone();
        let own = inputs[k].clone();
        shadows
            .bind_dependencies(&amounts[k], move |w| {
                stats.bindings.set(stats.bindings.get() + 1);
                if stats.failure.get() == 4 {
                    return Err(ShadowSessionError::UnregisteredSource);
                }
                let mut sources = vec![own.erase()];
                if let Some(parent) = w.parents[k] {
                    sources.push(nodes[parent].erase())
                }
                Ok(sources)
            })
            .unwrap();
    }
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, World>();
    let outputs = amounts.clone();
    let project = move |_: &World, s: &ShadowSession<'a, 's, usize, World>| {
        stats.projections.set(stats.projections.get() + 1);
        if stats.failure.get() == 6 {
            return Err(ShadowSessionError::UnregisteredSource);
        }
        assert_ne!(stats.failure.get(), 7, "projection");
        outputs
            .iter()
            .map(|node| {
                let r = s.read(node)?;
                Ok((r.value.map(|a| a.0), r.inconsistent))
            })
            .collect::<Result<Output, ShadowSessionError>>()
    };
    let out = if notified {
        b.map_shadows_notified(
            source.port(),
            shadows,
            move |before, after, s| {
                stats
                    .events
                    .borrow_mut()
                    .push((before.clone(), after.clone()));
                for k in 0..n {
                    if before.values[k] != after.values[k] {
                        s.mark_dirty(&inputs[k])?;
                    }
                    if before.parents[k] != after.parents[k] || before.bias != after.bias {
                        s.mark_dirty(&amounts[k])?;
                    }
                }
                // Fail after making session edits, proving rollback restores pending dirtiness too.
                if stats.failure.get() == 1 {
                    return Err(ShadowSessionError::UnregisteredSource);
                }
                assert_ne!(stats.failure.get(), 2, "notifier");
                Ok(())
            },
            project,
        )
        .unwrap()
    } else {
        b.map_shadows(source.port(), shadows, project).unwrap()
    };
    let data = b.dataset(&out).unwrap();
    b.score("sum", &out, move |rows| {
        assert_ne!(stats.failure.get(), 8, "score");
        SimpleScore::new(rows.iter().map(|(v, _)| v.unwrap_or(0)).sum())
    })
    .unwrap();
    (b.build(), source, data)
}
fn apply(
    g: &mut StreamGraph<'_, SimpleScore>,
    s: &StreamSource<usize, World>,
    id: usize,
    w: World,
) {
    let mut tx = g.transaction();
    tx.apply(s, TupleChange::put(id, w)).unwrap();
    tx.commit().unwrap();
}
#[test]
fn notifications_avoid_unaffected_callbacks_on_tiny_and_large_graphs() {
    for n in [1, 512] {
        let schema = schema();
        let stats = Stats::default();
        let (mut g, s, _) = graph(&schema, n, &stats, true);
        let mut w = World::new(n);
        apply(&mut g, &s, 7, w.clone());
        assert_eq!(stats.work(), (n, n, n));
        assert!(stats.events.borrow().is_empty());
        apply(&mut g, &s, 7, w.clone());
        assert_eq!(stats.work(), (n, n, n));
        assert_eq!(stats.events.borrow().len(), 1);
        w.values[n - 1] = 9;
        apply(&mut g, &s, 7, w.clone());
        assert_eq!(stats.work(), (n + 1, n, n + 1));
        assert_eq!(g.score(), SimpleScore::new(n as i64 + 8));
        w.bias = 2;
        apply(&mut g, &s, 7, w);
        assert_eq!(stats.work(), (n + 1, 2 * n, 2 * n + 1));
        assert_eq!(g.score(), SimpleScore::new(3 * n as i64 + 8));
        // Explicit independent diagnostics fully read/rebuild the world, then restore caches.
        let events = stats.events.borrow().len();
        g.assert_consistent().unwrap();
        assert_eq!(stats.events.borrow().len(), events);
        assert_eq!(stats.reads.get(), 2 * n + 1);
    }
}
#[test]
fn polling_retains_full_reader_and_binding_behavior() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, _) = graph(&schema, 3, &stats, false);
    apply(&mut g, &s, 0, World::new(3));
    apply(&mut g, &s, 0, World::new(3));
    assert_eq!(stats.work(), (6, 6, 3));
    assert!(stats.events.borrow().is_empty());
    g.assert_consistent().unwrap();
}
#[test]
fn per_id_before_worlds_follow_rollback_retraction_and_reintroduction() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, 2, &stats, true);
    let a = World::new(2);
    let mut b = a.clone();
    b.values[0] = 8;
    apply(&mut g, &s, 7, a.clone());
    apply(&mut g, &s, 8, b.clone());
    let rows = g.dataset_rows(&d).unwrap().clone();
    let mut x = a.clone();
    x.values[1] = 3;
    let mut y = x.clone();
    y.values[0] = 4;
    let count;
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(7, x.clone())).unwrap();
        tx.apply(&s, TupleChange::put(7, y.clone())).unwrap();
        tx.apply(&s, TupleChange::retract(8)).unwrap();
        count = stats.work();
    }
    assert_eq!(stats.work(), count);
    assert_eq!(g.dataset_rows(&d).unwrap(), &rows);
    assert_eq!(
        *stats.events.borrow(),
        vec![(a.clone(), x.clone()), (x, y.clone())]
    );
    apply(&mut g, &s, 8, b.clone());
    apply(&mut g, &s, 7, y.clone());
    assert_eq!(
        &stats.events.borrow()[2..],
        &[(b.clone(), b), (a, y.clone())]
    );
    let count = stats.work();
    let events = stats.events.borrow().len();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(7)).unwrap();
    tx.apply(&s, TupleChange::put(7, y)).unwrap();
    tx.commit().unwrap();
    assert_eq!(stats.events.borrow().len(), events);
    assert_eq!(stats.work(), (count.0 + 2, count.1 + 2, count.2 + 2));
    g.assert_consistent().unwrap();
}
#[test]
fn notified_rewires_cycles_descendants_and_external_facts_match_fresh_polling() {
    let schema = schema();
    let sparse = Stats::default();
    let full = Stats::default();
    let (mut g, s, d) = graph(&schema, 4, &sparse, true);
    let mut w = World::new(4);
    w.parents = vec![None, Some(0), Some(1), None];
    for step in 0..7 {
        match step {
            1 => w.values[0] = 5,
            2 => w.parents[0] = Some(1),
            3 => w.values[3] = 9,
            4 => {
                w.parents[0] = None;
                w.parents[1] = Some(3)
            }
            5 => w.bias = 3,
            6 => w.parents[2] = None,
            _ => {}
        }
        apply(&mut g, &s, 7, w.clone());
        // Fresh polling is independent of prior dirty flags, including external
        // supplier facts that are deliberately not registered input readers.
        let (mut reference, rs, rd) = graph(&schema, 4, &full, false);
        apply(&mut reference, &rs, 7, w.clone());
        assert_eq!(
            g.dataset_rows(&d).unwrap(),
            reference.dataset_rows(&rd).unwrap()
        );
        assert_eq!(g.score(), reference.score());
        if step == 2 {
            assert_eq!(
                g.dataset_rows(&d).unwrap()[&7],
                vec![(None, true), (None, true), (None, true), (Some(1), false)]
            );
        }
    }
    assert!(sparse.reads.get() < full.reads.get());
    assert!(sparse.bindings.get() < full.bindings.get());
    g.assert_consistent().unwrap();
}
#[test]
fn all_callback_failures_restore_before_worlds_and_session_edits() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, 2, &stats, true);
    let initial = World::new(2);
    apply(&mut g, &s, 7, initial.clone());
    let rows = g.dataset_rows(&d).unwrap().clone();
    for mode in 1..=8 {
        let mut changed = initial.clone();
        changed.values[0] = 8;
        changed.parents[1] = Some(0);
        stats.failure.set(mode);
        let mut tx = g.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            tx.apply(&s, TupleChange::put(7, changed.clone()))
        }));
        if [1, 4, 6].contains(&mode) {
            assert!(matches!(result.unwrap(), Err(GraphError::Dependency(_))))
        } else {
            assert!(result.is_err());
        }
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        stats.failure.set(0);
        assert_eq!(g.dataset_rows(&d).unwrap(), &rows);
        assert_eq!(g.source_rows(&s).unwrap()[&7], initial);
        assert_eq!(g.score(), SimpleScore::new(2));
        let work = stats.work();
        apply(&mut g, &s, 7, initial.clone());
        assert_eq!(
            stats.work(),
            work,
            "rollback restores cached state and pending notifications"
        );
        assert_eq!(
            stats.events.borrow().last().unwrap(),
            &(initial.clone(), initial.clone())
        );
    }
    g.assert_consistent().unwrap();
}

#[test]
fn retaining_previous_worlds_adds_no_world_clones_and_undo_restores_identity() {
    struct CountedWorld {
        value: i64,
        clones: std::rc::Rc<Cell<usize>>,
    }
    impl Clone for CountedWorld {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }
    let schema = schema();
    let mut counts = vec![];
    for notified in [false, true] {
        let clones = std::rc::Rc::new(Cell::new(0));
        let events = &RefCell::new(vec![]);
        let mut shadows = ShadowSessionBuilder::<usize, CountedWorld>::new(&schema);
        let input = shadows
            .input(
                schema.entity::<Task>().unwrap().variable("value").unwrap(),
                0,
                |w| Some(w.value),
            )
            .unwrap();
        let dirty = input.clone();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, CountedWorld>();
        let project = move |_: &CountedWorld, s: &ShadowSession<'_, '_, usize, CountedWorld>| {
            Ok(*s.read(&input)?.value.unwrap())
        };
        let out = if notified {
            b.map_shadows_notified(
                source.port(),
                shadows,
                move |before, after, s| {
                    events.borrow_mut().push((before.value, after.value));
                    if before.value != after.value {
                        s.mark_dirty(&dirty)?;
                    }
                    Ok(())
                },
                project,
            )
            .unwrap()
        } else {
            b.map_shadows(source.port(), shadows, project).unwrap()
        };
        b.score("value", &out, |value| SimpleScore::new(*value))
            .unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(
            &source,
            TupleChange::put(
                0,
                CountedWorld {
                    value: 1,
                    clones: clones.clone(),
                },
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        {
            let mut tx = graph.transaction();
            tx.apply(
                &source,
                TupleChange::put(
                    0,
                    CountedWorld {
                        value: 9,
                        clones: clones.clone(),
                    },
                ),
            )
            .unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(9));
        }
        assert_eq!(graph.score(), SimpleScore::new(1));
        assert_eq!(graph.source_rows(&source).unwrap()[&0].value, 1);
        let mut tx = graph.transaction();
        tx.apply(
            &source,
            TupleChange::put(
                0,
                CountedWorld {
                    value: 3,
                    clones: clones.clone(),
                },
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        if notified {
            assert_eq!(*events.borrow(), vec![(1, 9), (1, 3)]);
        } else {
            assert!(events.borrow().is_empty());
        }
        assert_eq!(graph.score(), SimpleScore::new(3));
        counts.push(clones.get());
        graph.assert_consistent().unwrap();
    }
    assert_eq!(
        counts[0], counts[1],
        "notified retention adds no clones to the ordinary polling path"
    );
}
