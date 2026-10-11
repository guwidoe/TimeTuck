//! Original component-local activity replay, checked totals and rollback cases.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Entity;
fn apply<I: Ord + Clone + 'static, T: Clone + 'static>(
    tx: &mut GraphTransaction<'_, '_, SimpleScore>,
    source: &StreamSource<I, T>,
    change: TupleChange<I, T>,
    batch: bool,
) -> Result<(), GraphError> {
    if batch {
        let mut changes = StreamBatch::new();
        changes.push(source, change)?;
        tx.apply_batch(changes)
    } else {
        tx.apply(source, change)
    }
}
#[test]
fn independent_shadow_programs_survive_transitions_and_outside_batch_updates() {
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
    ])])
    .unwrap();
    let input = schema
        .entity::<Entity>()
        .unwrap()
        .variable("input")
        .unwrap();
    for batch in [false, true] {
        let factories = Cell::new(0);
        let reads = Cell::new(0);
        let projects = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let left = b.source::<usize, i64>();
        let world = b.source::<usize, Vec<i64>>();
        let right = b.source::<usize, i64>();
        let joined = b
            .join_equal(left.port(), right.port(), |_| (), |_| ())
            .unwrap();
        b.score("join", &joined, |(a, c)| SimpleScore::new(a + c))
            .unwrap();
        let shadows = b
            .map_dynamic_shadows(
                world.port(),
                |w| w.len(),
                |n| {
                    factories.set(factories.get() + 1);
                    let mut builder = ShadowSessionBuilder::new(&schema);
                    let mut handles = vec![];
                    for i in 0..*n {
                        let reads = &reads;
                        handles.push(builder.input(input, i, move |w: &Vec<i64>| {
                            reads.set(reads.get() + 1);
                            Some(w[i])
                        })?);
                    }
                    let projects = &projects;
                    ShadowProgram::new(builder, move |_, session| {
                        projects.set(projects.get() + 1);
                        Ok(handles
                            .iter()
                            .map(|h| *session.read(h).unwrap().value.unwrap())
                            .sum::<i64>())
                    })
                },
            )
            .unwrap();
        let data = b.dataset(&shadows).unwrap();
        b.score("shadows", &shadows, |v| SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        let mut seed = StreamBatch::new();
        seed.push(&left, TupleChange::put(0, 3)).unwrap();
        seed.push(&world, TupleChange::put(0, vec![2, 5])).unwrap();
        let mut tx = g.transaction();
        tx.apply_batch(seed).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(7));
        let counters = (factories.get(), reads.get(), projects.get());
        for step in 0..8 {
            let mut tx = g.transaction();
            let change = if step % 2 == 0 {
                TupleChange::put(0, 4)
            } else {
                TupleChange::retract(0)
            };
            apply(&mut tx, &right, change, batch).unwrap();
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(if step % 2 == 0 { 14 } else { 7 })
            );
            tx.commit().unwrap();
            assert_eq!((factories.get(), reads.get(), projects.get()), counters);
            assert_eq!(g.dataset_rows(&data).unwrap()[&0], 7);
        }
        // The queued world change is outside the rebuilt join component and must
        // survive pending-event replacement. Its same-membership program is reused.
        let mut changes = StreamBatch::new();
        changes.push(&right, TupleChange::put(0, 10)).unwrap();
        changes
            .push(&world, TupleChange::put(0, vec![8, 9]))
            .unwrap();
        let mut tx = g.transaction();
        tx.apply_batch(changes).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(30));
        tx.commit().unwrap();
        assert_eq!(factories.get(), counters.0);
        assert_eq!(projects.get(), counters.2 + 1);
        assert_eq!(reads.get(), counters.1 + 2);
        assert_eq!(g.dataset_rows(&data).unwrap()[&0], 17);
        g.assert_consistent().unwrap();
    }
}
#[test]
fn shared_sources_join_concat_and_aggregate_edges_unify_replay_components() {
    for batched in [false, true] {
        let connected_calls = Cell::new(0);
        let separate_calls = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let a = b.source::<usize, i64>();
        let separate = b.source::<usize, i64>();
        let c = b.source::<usize, i64>();
        let gate = b.source::<usize, i64>();
        let cm = b
            .map(c.port(), |v| {
                connected_calls.set(connected_calls.get() + 1);
                *v
            })
            .unwrap();
        let sm = b
            .map(separate.port(), |v| {
                separate_calls.set(separate_calls.get() + 1);
                *v
            })
            .unwrap();
        b.score("separate", &sm, |v| SimpleScore::new(*v)).unwrap();
        let merged = b.concat(a.port(), &cm).unwrap();
        let groups = b
            .group_collect_with::<_, _, _, _, CountCollector>(&merged, |_| ((), ()))
            .unwrap();
        let data = b.dataset(&groups).unwrap();
        let joined = b.join_equal(a.port(), gate.port(), |_| (), |_| ()).unwrap();
        b.score("pairs", &joined, |_| SimpleScore::new(1)).unwrap();
        let mut g = b.build();
        let mut seed = StreamBatch::new();
        for (source, v) in [(&a, 2), (&c, 4), (&separate, 9)] {
            seed.push(source, TupleChange::put(0, v)).unwrap();
        }
        let mut tx = g.transaction();
        tx.apply_batch(seed).unwrap();
        tx.commit().unwrap();
        let separate_before = separate_calls.get();
        for step in 0..6 {
            let before = connected_calls.get();
            let mut tx = g.transaction();
            apply(
                &mut tx,
                &gate,
                if step % 2 == 0 {
                    TupleChange::put(0, 0)
                } else {
                    TupleChange::retract(0)
                },
                batched,
            )
            .unwrap();
            tx.commit().unwrap();
            assert_eq!(connected_calls.get(), before + 1);
            assert_eq!(separate_calls.get(), separate_before);
            assert_eq!(g.dataset_rows(&data).unwrap()[&()].1, 2);
            assert_eq!(
                g.score(),
                SimpleScore::new(if step % 2 == 0 { 10 } else { 9 })
            );
        }
        g.assert_consistent().unwrap();
    }
}
#[test]
fn partial_replay_defers_checked_graph_totals_and_restores_failures() {
    for batched in [false, true] {
        let untouched = Cell::new(0);
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let positive = b.source::<usize, i64>();
        let extra = b.source::<usize, i64>();
        let negative = b.source::<usize, i64>();
        let gate = b.source::<usize, i64>();
        b.score("positive", positive.port(), |v| {
            untouched.set(untouched.get() + 1);
            SimpleScore::new(*v)
        })
        .unwrap();
        b.score("extra", extra.port(), |v| {
            untouched.set(untouched.get() + 1);
            SimpleScore::new(*v)
        })
        .unwrap();
        b.score("negative", negative.port(), |v| SimpleScore::new(*v))
            .unwrap();
        let mapped = b
            .map(gate.port(), |v| {
                assert!(!fail.get(), "replay panic");
                *v
            })
            .unwrap();
        let pairs = b
            .join_equal(negative.port(), &mapped, |_| (), |_| ())
            .unwrap();
        b.score("joined", &pairs, |(_, v)| SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        let mut seed = StreamBatch::new();
        for (source, v) in [(&positive, i64::MAX), (&extra, 5), (&negative, -10)] {
            seed.push(source, TupleChange::put(0, v)).unwrap();
        }
        let mut tx = g.transaction();
        tx.apply_batch(seed).unwrap();
        tx.commit().unwrap();
        let count = untouched.get();
        let mut tx = g.transaction();
        apply(&mut tx, &gate, TupleChange::put(0, 1), batched).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(i64::MAX - 4));
        drop(tx);
        assert_eq!(g.score(), SimpleScore::new(i64::MAX - 5));
        assert!(g.source_rows(&gate).unwrap().is_empty());
        assert_eq!(untouched.get(), count);
        fail.set(true);
        {
            let mut tx = g.transaction();
            assert!(
                catch_unwind(AssertUnwindSafe(|| apply(
                    &mut tx,
                    &gate,
                    TupleChange::put(0, 1),
                    batched
                )))
                .is_err()
            );
            assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        }
        fail.set(false);
        {
            let mut tx = g.transaction();
            assert!(matches!(
                apply(&mut tx, &gate, TupleChange::put(0, 6), batched),
                Err(GraphError::ScoreOverflow { constraint: None })
            ));
            assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        }
        assert_eq!(g.score(), SimpleScore::new(i64::MAX - 5));
        assert!(g.source_rows(&gate).unwrap().is_empty());
        assert_eq!(untouched.get(), count);
        let mut tx = g.transaction();
        apply(&mut tx, &gate, TupleChange::put(0, 1), batched).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(i64::MAX - 4));
        g.assert_consistent().unwrap();
    }
}
