// Original regressions, with no upstream compatibility credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    GraphError, HardSoftScore, IntSummary, SimpleScore, StreamGraphBuilder, TupleChange as Change,
};

#[test]
fn shared_nodes_checkpoint_once_and_undo_does_not_replay_callbacks() {
    let filter_calls = Cell::new(0);
    let score_calls = Cell::new(0);
    let unrelated_calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let source = b.source::<usize, i64>();
    let unrelated = b.source::<usize, i64>();
    let filtered = b
        .filter(source.port(), |&v| {
            filter_calls.set(filter_calls.get() + 1);
            v >= 0
        })
        .unwrap();
    let joined = b.join_equal(&filtered, &filtered, |_| (), |_| ()).unwrap();
    let pairs = b
        .score("pairs", &joined, |_| {
            score_calls.set(score_calls.get() + 1);
            HardSoftScore::new(-1, 0)
        })
        .unwrap();
    let groups = b.group_collect(&filtered, |&v| (0, v)).unwrap();
    let loads = b
        .score("load", &groups, |(_, s)| {
            score_calls.set(score_calls.get() + 1);
            HardSoftScore::new(0, -s.sum)
        })
        .unwrap();
    b.score("unrelated", unrelated.port(), |&v| {
        unrelated_calls.set(unrelated_calls.get() + 1);
        HardSoftScore::new(0, v)
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, 1)).unwrap();
    tx.apply(&source, Change::put(1, 2)).unwrap();
    tx.apply(&unrelated, Change::put(0, 10)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), HardSoftScore::new(-4, 7));
    let before_pairs = graph.constraint_matches(&pairs).unwrap().clone();
    let before_loads = graph.constraint_matches(&loads).unwrap().clone();
    let before_filter = filter_calls.get();
    let before_unrelated = unrelated_calls.get();
    let calls_after_trial;
    {
        let mut tx = graph.transaction();
        tx.apply(&source, Change::put(0, 3)).unwrap();
        tx.apply(&source, Change::put(1, -1)).unwrap();
        assert_eq!(tx.score(), Ok(HardSoftScore::new(-1, 7)));
        assert_eq!(tx.checkpoint_count(), 6);
        assert_eq!(
            tx.graph()
                .unwrap()
                .constraint_matches(&pairs)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(filter_calls.get(), before_filter + 2);
        assert_eq!(unrelated_calls.get(), before_unrelated);
        calls_after_trial = score_calls.get();
    }
    assert_eq!(score_calls.get(), calls_after_trial);
    assert_eq!(graph.score(), HardSoftScore::new(-4, 7));
    assert_eq!(graph.constraint_matches(&pairs).unwrap(), &before_pairs);
    assert_eq!(graph.constraint_matches(&loads).unwrap(), &before_loads);
    assert_eq!(
        graph.source_rows(&source).unwrap(),
        &BTreeMap::from([(0, 1), (1, 2)])
    );
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, 3)).unwrap();
    let calls = score_calls.get();
    tx.commit().unwrap();
    assert_eq!(score_calls.get(), calls);
    assert_eq!(graph.score(), HardSoftScore::new(-4, 5));
}

#[test]
fn errors_and_caught_panics_abort_the_entire_multi_input_transaction() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let first = b
        .score("first", source.port(), |&v| SimpleScore::new(v))
        .unwrap();
    let second = b
        .score("second", source.port(), |&v| {
            assert_ne!(v, 98, "callback failure");
            if v == 99 {
                SimpleScore::ZERO.with_structural(1)
            } else {
                SimpleScore::new(-v)
            }
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, 1)).unwrap();
    tx.commit().unwrap();
    for bad in [99, 98] {
        let mut tx = graph.transaction();
        tx.apply(&source, Change::put(0, 2)).unwrap();
        if bad == 99 {
            assert_eq!(
                tx.apply(&source, Change::put(1, bad)),
                Err(GraphError::StructuralContribution("second".into()))
            );
        } else {
            assert!(
                catch_unwind(AssertUnwindSafe(|| tx.apply(&source, Change::put(1, bad)))).is_err()
            );
        }
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
        assert_eq!(
            tx.apply(&source, Change::put(0, 3)),
            Err(GraphError::FailedTransaction)
        );
        assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
        assert_eq!(
            graph.source_rows(&source).unwrap(),
            &BTreeMap::from([(0, 1)])
        );
        assert_eq!(
            graph.constraint_matches(&first).unwrap(),
            &BTreeMap::from([(0, (1, SimpleScore::new(1)))])
        );
        assert_eq!(
            graph.constraint_matches(&second).unwrap(),
            &BTreeMap::from([(0, (1, SimpleScore::new(-1)))])
        );
        assert_eq!(graph.score(), SimpleScore::ZERO);
    }
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, 4)).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        graph.constraint_scores().collect::<Vec<_>>(),
        [
            ("first", SimpleScore::new(4)),
            ("second", SimpleScore::new(-4))
        ]
    );
}

#[derive(Debug)]
struct Fragile(i64, Rc<Cell<usize>>);
impl Clone for Fragile {
    fn clone(&self) -> Self {
        let n = self.1.get();
        if n > 0 {
            self.1.set(n - 1);
            assert_ne!(n, 1, "clone failure");
        }
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Fragile {
    fn eq(&self, r: &Self) -> bool {
        self.0 == r.0
    }
}
impl Eq for Fragile {}
impl PartialOrd for Fragile {
    fn partial_cmp(&self, r: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(r))
    }
}
impl Ord for Fragile {
    fn cmp(&self, r: &Self) -> std::cmp::Ordering {
        self.0.cmp(&r.0)
    }
}

#[test]
// The mutable countdown only injects Clone failures; Eq/Ord depend on the
// immutable integer field exclusively.
#[allow(clippy::mutable_key_type)]
fn clone_panics_in_checkpoint_projection_commit_or_fanout_restore_original_state() {
    let mut panics = 0;
    for fail_at in 1..45 {
        let remaining = Rc::new(Cell::new(0));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Fragile>();
        let filter = b.filter(source.port(), |_| true).unwrap();
        let join = b
            .join_equal(&filter, &filter, |v| v.clone(), |v| v.clone())
            .unwrap();
        let groups = b.group_collect(&join, |(a, _)| (a.clone(), a.0)).unwrap();
        let constraint = b
            .score("sum", &groups, |(_, s)| SimpleScore::new(s.sum))
            .unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(&source, Change::put(0, Fragile(1, remaining.clone())))
            .unwrap();
        tx.commit().unwrap();
        remaining.set(fail_at);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            tx.apply(&source, Change::put(0, Fragile(2, remaining.clone())))
                .unwrap();
            assert_eq!(tx.score(), Ok(SimpleScore::new(2)));
        }));
        panics += usize::from(outcome.is_err());
        remaining.set(0);
        assert_eq!(graph.score(), SimpleScore::new(1), "fail at {fail_at}");
        assert_eq!(graph.source_rows(&source).unwrap().get(&0).unwrap().0, 1);
        let matches = graph.constraint_matches(&constraint).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches.first_key_value().unwrap().0.0, 1);
        assert_eq!(matches.first_key_value().unwrap().1.1, SimpleScore::new(1));
        // A restored formerly poisoned operator remains usable.
        let mut tx = graph.transaction();
        tx.apply(&source, Change::put(0, Fragile(3, remaining.clone())))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(graph.score(), SimpleScore::new(3));
    }
    assert!(panics > 15);
}

#[test]
fn foreign_handles_and_duplicate_names_are_rejected_without_partial_registration() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let other = a.source::<usize, i64>();
    let other_constraint = a
        .score("other", other.port(), |&v| SimpleScore::new(v))
        .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let local = b.source::<usize, i64>();
    assert!(matches!(
        b.filter(other.port(), |_| true),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.map(other.port(), |v| *v),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.join_equal(local.port(), other.port(), |v| *v, |v| *v),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.group_collect(other.port(), |v| (0, *v)),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.score("bad", other.port(), |&v| SimpleScore::new(v)),
        Err(GraphError::ForeignPort)
    ));
    b.score("good", local.port(), |&v| SimpleScore::new(v))
        .unwrap();
    assert!(matches!(
        b.score("good", local.port(), |_| SimpleScore::ZERO),
        Err(GraphError::DuplicateConstraint(_))
    ));
    let mut graph = b.build();
    assert!(matches!(
        graph.source_rows(&other),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        graph.constraint_matches(&other_constraint),
        Err(GraphError::ForeignPort)
    ));
    let mut tx = graph.transaction();
    tx.apply(&local, Change::put(0, 3)).unwrap();
    assert_eq!(
        tx.apply(&other, Change::put(0, 4)),
        Err(GraphError::ForeignPort)
    );
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert!(graph.source_rows(&local).unwrap().is_empty());
    assert_eq!(
        graph.constraint_scores().collect::<Vec<_>>(),
        [("good", SimpleScore::ZERO)]
    );
}

#[test]
fn empty_graph_and_deep_graph_use_bounded_stack_and_wrapping_score_arithmetic() {
    let mut empty = StreamGraphBuilder::<SimpleScore>::new().build();
    let tx = empty.transaction();
    assert_eq!(tx.checkpoint_count(), 0);
    tx.commit().unwrap();
    assert_eq!(empty.score(), SimpleScore::ZERO);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let mut tail = source.port().clone();
    for _ in 0..2000 {
        tail = b.map(&tail, |v| v.wrapping_add(1)).unwrap();
    }
    b.score("deep", &tail, |&v| SimpleScore::new(v)).unwrap();
    let mut graph = b.build();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, Change::put(0, i64::MAX)).unwrap();
        assert_eq!(
            tx.score(),
            Ok(SimpleScore::new(i64::MAX.wrapping_add(2000)))
        );
    }
    assert_eq!(graph.score(), SimpleScore::ZERO);
    let mut tx = graph.transaction();
    tx.apply(&source, Change::put(0, 0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(2000));
}

#[test]
fn multi_source_candidates_match_independent_recomputation_across_commit_and_undo() {
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let jobs = b.source::<usize, (i64, i64)>();
    let resources = b.source::<usize, (i64, i64)>();
    let active = b.filter(jobs.port(), |row| row.1 >= 0).unwrap();
    let joined = b
        .join_equal(&active, resources.port(), |row| row.0, |row| row.0)
        .unwrap();
    let mapped = b
        .map(&joined, |(job, resource)| (resource.0, resource.1, job.1))
        .unwrap();
    let grouped = b
        .group_collect(&mapped, |&(machine, capacity, weight)| {
            ((machine, capacity), weight)
        })
        .unwrap();
    let handle = b
        .score("load", &grouped, |((_, cap), s)| {
            HardSoftScore::new(-(s.sum - cap).max(0), -(s.count as i64))
        })
        .unwrap();
    let mut graph = b.build();
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut seed = 20261012_u64;
    for step in 0..1000 {
        let mut next_left = left.clone();
        let mut next_right = right.clone();
        let mut tx = graph.transaction();
        for write in 0..3 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let id = (seed >> 32) as usize % 9;
            let row = ((seed >> 48) % 3) as i64;
            let weight = ((seed >> 40) % 9) as i64 - 1;
            let value = (!seed.is_multiple_of(5)).then_some((row, weight));
            let (input, table) = if write == 0 {
                (&resources, &mut next_right)
            } else {
                (&jobs, &mut next_left)
            };
            if let Some(value) = value {
                table.insert(id, value);
            } else {
                table.remove(&id);
            }
            tx.apply(input, Change { id, value }).unwrap();
        }
        let mut grouped_values: BTreeMap<(i64, i64), Vec<i64>> = BTreeMap::new();
        for &(key, weight) in next_left.values() {
            if weight >= 0 {
                for &(machine, cap) in next_right.values() {
                    if key == machine {
                        grouped_values
                            .entry((machine, cap))
                            .or_default()
                            .push(weight);
                    }
                }
            }
        }
        let mut expected = HardSoftScore::ZERO;
        let mut matches = BTreeMap::new();
        for (key, mut values) in grouped_values {
            values.sort_unstable();
            let count = values.len();
            let min = values.first().copied();
            let max = values.last().copied();
            let sum = values.iter().sum();
            values.dedup();
            let summary = IntSummary {
                count,
                distinct_count: values.len(),
                sum,
                min,
                max,
            };
            let score = HardSoftScore::new(-(sum - key.1).max(0), -(count as i64));
            expected = expected + score;
            matches.insert(key, ((key, summary), score));
        }
        assert_eq!(tx.score(), Ok(expected), "step {step}");
        assert_eq!(
            tx.graph().unwrap().constraint_matches(&handle).unwrap(),
            &matches
        );
        if step % 3 != 0 {
            tx.commit().unwrap();
            left = next_left;
            right = next_right;
        } else {
            drop(tx);
        }
        assert_eq!(graph.source_rows(&jobs).unwrap(), &left);
        assert_eq!(graph.source_rows(&resources).unwrap(), &right);
        let recomputed = graph
            .constraint_matches(&handle)
            .unwrap()
            .values()
            .fold(HardSoftScore::ZERO, |s, (_, part)| s + *part);
        assert_eq!(graph.score(), recomputed);
    }
}

#[test]
fn reconverging_paths_keep_raw_facts_and_group_summaries_consistent() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (i64, i64)>();
    let grouped = builder.group_collect(source.port(), |&row| row).unwrap();
    // The same source reaches this join directly and through three operators.
    let mapped = builder.map(&grouped, |row| *row).unwrap();
    let delayed = builder.filter(&mapped, |(_, s)| s.count > 0).unwrap();
    let joined = builder
        .join_equal(source.port(), &delayed, |row| row.0, |row| row.0)
        .unwrap();
    let constraint = builder
        .score("deviation", &joined, |(row, (_, s))| {
            SimpleScore::new(row.1 * s.count as i64 - s.sum)
        })
        .unwrap();
    let mut graph = builder.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261013_u64;
    for step in 0..600 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 9;
        let value = (!seed.is_multiple_of(4))
            .then_some((((seed >> 40) % 3) as i64, ((seed >> 48) % 11) as i64 - 5));
        let mut candidate_rows = rows.clone();
        if let Some(row) = value {
            candidate_rows.insert(id, row);
        } else {
            candidate_rows.remove(&id);
        }
        let before = graph.constraint_matches(&constraint).unwrap().clone();
        let mut trial = graph.transaction();
        trial.apply(&source, Change { id, value }).unwrap();
        let matches = trial
            .graph()
            .unwrap()
            .constraint_matches(&constraint)
            .unwrap();
        assert_eq!(matches.len(), candidate_rows.len());
        for (&id, &(key, weight)) in &candidate_rows {
            let mut group: Vec<_> = candidate_rows
                .values()
                .filter(|row| row.0 == key)
                .map(|row| row.1)
                .collect();
            group.sort_unstable();
            let count = group.len();
            let sum: i64 = group.iter().sum();
            let min = group.first().copied();
            let max = group.last().copied();
            group.dedup();
            let summary = IntSummary {
                count,
                distinct_count: group.len(),
                sum,
                min,
                max,
            };
            assert_eq!(
                matches[&(id, key)],
                (
                    ((key, weight), (key, summary)),
                    SimpleScore::new(weight * count as i64 - sum)
                ),
                "step {step}"
            );
        }
        // Deviations from each group's mean sum to zero.
        assert_eq!(trial.score(), Ok(SimpleScore::ZERO));
        if step % 3 == 0 {
            drop(trial);
            assert_eq!(graph.constraint_matches(&constraint).unwrap(), &before);
        } else {
            trial.commit().unwrap();
            rows = candidate_rows;
        }
        assert_eq!(graph.source_rows(&source).unwrap(), &rows);
    }
}
