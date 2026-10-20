//! Original live-commit capability controls; no upstream-method credit.
use super::*;
use crate::{Score, SimpleScore};
use std::{cell::Cell, cmp::Ordering, ops};

fn counted<T>(calls: &Cell<usize>, value: T) -> T {
    calls.set(calls.get() + 1);
    value
}

type Pipeline<'a> = (
    StreamGraph<'a, SimpleScore>,
    StreamSource<usize, usize>,
    StreamSource<usize, usize>,
    StreamConstraint<usize, (usize, IntSummary)>,
);

fn pipeline(calls: &Cell<usize>, compare: bool) -> Pipeline<'_> {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, usize>();
    let right = b.source::<usize, usize>();
    let filtered = b
        .filter(left.port(), move |value| counted(calls, *value > 0))
        .unwrap();
    let mapped = b
        .map(&filtered, move |value| counted(calls, *value * 2))
        .unwrap();
    let expanded = b
        .flat_map(&mapped, move |value| counted(calls, [*value, *value + 1]))
        .unwrap();
    let joined = if compare {
        b.join_compare(
            &expanded,
            right.port(),
            JoinComparison::Equal,
            move |value| counted(calls, *value % 2),
            move |value| counted(calls, *value % 2),
        )
    } else {
        b.join_equal(
            &expanded,
            right.port(),
            move |value| counted(calls, *value % 2),
            move |value| counted(calls, *value % 2),
        )
    }
    .unwrap();
    let grouped = b
        .group_collect(&joined, move |(left, right)| {
            counted(calls, (*left % 2, (left + right) as i64))
        })
        .unwrap();
    let constraint = b
        .score("sum", &grouped, move |(_, summary)| {
            counted(calls, SimpleScore::new(summary.sum))
        })
        .unwrap();
    (b.build(), left, right, constraint)
}

#[test]
fn eligible_pipeline_preserves_callbacks_matches_rollback_and_fixed_source_conversion() {
    for compare in [false, true] {
        let calls = Cell::new(0);
        let old_calls = Cell::new(0);
        let (mut graph, left, right, constraint) = pipeline(&calls, compare);
        let (mut control, old_left, old_right, old_constraint) = pipeline(&old_calls, compare);
        assert!(graph.supports_live_commit());
        assert!(control.supports_live_commit());
        control.disable_live_commit_for_tests();
        assert!(!control.supports_live_commit());
        for (round, value) in [1, 3, 0, 2].into_iter().enumerate() {
            let mut tx = graph.transaction();
            let mut old_tx = control.transaction();
            tx.apply(&left, TupleChange::put(0, value)).unwrap();
            old_tx.apply(&old_left, TupleChange::put(0, value)).unwrap();
            tx.apply(&right, TupleChange::put(0, 2)).unwrap();
            old_tx.apply(&old_right, TupleChange::put(0, 2)).unwrap();
            assert_eq!(tx.score().unwrap(), old_tx.score().unwrap());
            assert_eq!(tx.score_evaluations(), old_tx.score_evaluations());
            assert_eq!(calls.get(), old_calls.get());
            assert_eq!(
                tx.graph().unwrap().constraint_matches(&constraint).unwrap(),
                old_tx
                    .graph()
                    .unwrap()
                    .constraint_matches(&old_constraint)
                    .unwrap()
            );
            if round == 1 {
                drop(tx);
                drop(old_tx);
            } else {
                tx.commit().unwrap();
                old_tx.commit().unwrap();
            }
            assert_eq!(graph.score(), control.score());
            assert_eq!(
                graph.source_rows(&left).unwrap(),
                control.source_rows(&old_left).unwrap()
            );
            assert!(graph.supports_live_commit());
        }
        graph.bind_fixed_planning_source(&left, 1).unwrap();
        graph.bind_fixed_planning_source(&right, 1).unwrap();
        assert!(graph.supports_live_commit());
        assert!(
            calls.get() > 0,
            "borrowed callback effects remain observable"
        );
    }
}

#[test]
fn eligible_expanded_self_join_preserves_nested_ids_and_four_coordinate_group_keys() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, usize, usize)>();
    let assigned = b.filter(source.port(), |row| row.0 > 0).unwrap();
    let pairs = b
        .join_equal(&assigned, &assigned, |row| row.1, |row| row.1)
        .unwrap();
    b.score("pairs", &pairs, |_| SimpleScore::new(1)).unwrap();
    let expanded = b
        .flat_map(&assigned, |row| [*row, (row.0, row.1, row.2 + 1)])
        .unwrap();
    let joined = b
        .join_equal(&expanded, &expanded, |row| row.1, |row| row.1)
        .unwrap();
    let grouped = b
        .group_collect(&joined, |(left, right)| {
            ((left.0, right.0, left.2, right.2), 1)
        })
        .unwrap();
    let constraint = b
        .score("nested", &grouped, |(_, summary)| {
            SimpleScore::new(summary.sum)
        })
        .unwrap();
    let mut graph = b.build();
    assert!(graph.supports_live_commit());
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (1, 2, 3))).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(5));
    tx.commit().unwrap();
    assert_eq!(graph.constraint_matches(&constraint).unwrap().len(), 4);
    graph.bind_fixed_planning_source(&source, 1).unwrap();
    assert!(graph.supports_live_commit());
    let mut trial = graph.transaction();
    trial.apply(&source, TupleChange::retract(0)).unwrap();
    assert_eq!(trial.score().unwrap(), SimpleScore::ZERO);
    drop(trial);
    assert_eq!(graph.score(), SimpleScore::new(5));
    assert_eq!(graph.constraint_matches(&constraint).unwrap().len(), 4);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CustomKey(usize);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DropKey(usize);
impl Drop for DropKey {
    fn drop(&mut self) {}
}

#[test]
fn primitive_identity_allowlist_is_exact_including_supported_nested_shapes() {
    assert!(live_commit_key::<usize>());
    assert!(live_commit_key::<(usize, usize)>());
    assert!(live_commit_key::<(usize, usize, usize)>());
    assert!(live_commit_key::<((usize, usize), usize)>());
    assert!(live_commit_key::<(usize, (usize, usize))>());
    assert!(live_commit_key::<((usize, usize), (usize, usize))>());
    assert!(live_commit_key::<(usize, usize, usize, usize)>());
    assert!(!live_commit_key::<CustomKey>());
    assert!(!live_commit_key::<DropKey>());
    assert!(!live_commit_key::<i64>());
    assert!(!live_commit_key::<(usize, i64)>());
    assert!(!live_commit_key::<((usize, usize, usize), usize)>());
    for custom_drop in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if custom_drop {
            b.source::<DropKey, usize>();
        } else {
            b.source::<CustomKey, usize>();
        }
        assert!(!b.build().supports_live_commit());
    }
}

#[test]
fn all_payload_owners_and_callback_environments_require_no_drop() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    let owned = b.source::<usize, String>();
    assert!(!b.live_commit_nodes[owned.0.node]);
    let mapped = b.map(source.port(), |_| String::new()).unwrap();
    assert!(!b.live_commit_nodes[mapped.node]);
    let expanded = b.flat_map(source.port(), |_| [String::new()]).unwrap();
    assert!(!b.live_commit_nodes[expanded.node]);
    let owned_filter = b.filter(owned.port(), |_| true).unwrap();
    assert!(!b.live_commit_nodes[owned_filter.node]);
    let owned_map = b.map(owned.port(), |_| 0usize).unwrap();
    assert!(!b.live_commit_nodes[owned_map.node]);
    let owned_flat = b.flat_map(owned.port(), |_| [0usize]).unwrap();
    assert!(!b.live_commit_nodes[owned_flat.node]);
    let owned_group = b.group_collect(owned.port(), |_| (0usize, 1)).unwrap();
    assert!(!b.live_commit_nodes[owned_group.node]);
    let owned_score = b
        .score("owned", owned.port(), |_| SimpleScore::ZERO)
        .unwrap();
    assert!(!b.live_commit_nodes[owned_score.0.node]);

    let captured = Rc::new(Cell::new(0usize));
    let filter = b
        .filter(source.port(), {
            let captured = captured.clone();
            move |_| captured.get() == 0
        })
        .unwrap();
    assert!(!b.live_commit_nodes[filter.node]);
    let mapped = b
        .map(source.port(), {
            let captured = captured.clone();
            move |_| captured.get()
        })
        .unwrap();
    assert!(!b.live_commit_nodes[mapped.node]);
    let expanded = b
        .flat_map(source.port(), {
            let captured = captured.clone();
            move |_| [captured.get()]
        })
        .unwrap();
    assert!(!b.live_commit_nodes[expanded.node]);
    let grouped = b
        .group_collect(source.port(), {
            let captured = captured.clone();
            move |_| (captured.get(), 1)
        })
        .unwrap();
    assert!(!b.live_commit_nodes[grouped.node]);
    let scored = b
        .score("captured", source.port(), move |_| {
            SimpleScore::new(captured.get() as i64)
        })
        .unwrap();
    assert!(!b.live_commit_nodes[scored.0.node]);
    assert!(!b.build().supports_live_commit());
}

#[test]
fn group_keys_and_both_join_sides_are_checked_independently() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    let owned = b.source::<usize, String>();
    let custom_ids = b.source::<CustomKey, usize>();
    let join_custom_left = b
        .join_equal(custom_ids.port(), source.port(), |v| *v, |v| *v)
        .unwrap();
    assert!(!b.live_commit_nodes[join_custom_left.node]);
    let join_custom_right = b
        .join_equal(source.port(), custom_ids.port(), |v| *v, |v| *v)
        .unwrap();
    assert!(!b.live_commit_nodes[join_custom_right.node]);
    let key = b
        .group_collect(source.port(), |_| (CustomKey(0), 1))
        .unwrap();
    assert!(!b.live_commit_nodes[key.node]);
    let drop_key = b.group_collect(source.port(), |_| (DropKey(0), 1)).unwrap();
    assert!(!b.live_commit_nodes[drop_key.node]);
    let join_key = b
        .join_equal(
            source.port(),
            source.port(),
            |v| CustomKey(*v),
            |v| CustomKey(*v),
        )
        .unwrap();
    assert!(!b.live_commit_nodes[join_key.node]);
    let join_left = b
        .join_equal(owned.port(), source.port(), |_| 0usize, |v| *v)
        .unwrap();
    assert!(!b.live_commit_nodes[join_left.node]);
    let join_right = b
        .join_equal(source.port(), owned.port(), |v| *v, |_| 0usize)
        .unwrap();
    assert!(!b.live_commit_nodes[join_right.node]);
    for right_capture in [false, true] {
        let captured = Rc::new(Cell::new(0usize));
        let joined = if right_capture {
            b.join_compare(
                source.port(),
                source.port(),
                JoinComparison::Equal,
                |v| *v,
                move |_| captured.get(),
            )
        } else {
            b.join_compare(
                source.port(),
                source.port(),
                JoinComparison::Equal,
                move |_| captured.get(),
                |v| *v,
            )
        }
        .unwrap();
        assert!(!b.live_commit_nodes[joined.node]);
    }
    assert!(!b.build().supports_live_commit());
}

#[test]
fn activity_initial_observers_and_unknown_nodes_keep_legacy_path() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    b.source::<usize, usize>();
    assert!(!b.build().supports_live_commit());

    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    b.after_initial_facts(source.port(), |_| {}).unwrap();
    let mut graph = b.build();
    assert!(!graph.supports_live_commit());
    graph.finish_initial_facts();
    assert!(!graph.supports_live_commit());

    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    b.dataset(source.port()).unwrap();
    assert!(!b.build().supports_live_commit());

    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    b.source::<usize, usize>();
    let unknown: StreamPort<usize, usize> = b.push(SourceNode::<usize, usize>(SnapshotMap::new()));
    assert!(!b.live_commit_nodes[unknown.node]);
    assert!(!b.build().supports_live_commit());

    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    b.score_only("unsupported terminal", source.port(), |_| SimpleScore::ZERO)
        .unwrap();
    assert!(!b.build().supports_live_commit());
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CustomScore(i64);
impl ops::Add for CustomScore {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}
impl ops::Sub for CustomScore {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}
impl Score for CustomScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        std::iter::once(self.0)
    }
    fn structural(&self) -> i64 {
        0
    }
    fn hard_cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
    fn is_feasible(&self) -> bool {
        true
    }
}
impl AdditiveScore for CustomScore {
    const ZERO: Self = Self(0);
    const POOLED_SAME_CONTEXT_MEMO: bool = true;
    fn with_structural(self, _: i64) -> Self {
        self
    }
}
fn score_capability<S: AdditiveScore + 'static>() -> bool {
    let mut b = StreamGraphBuilder::<S>::new();
    let source = b.source::<usize, usize>();
    b.score("value", source.port(), |_| S::ZERO).unwrap();
    b.build().supports_live_commit()
}
#[test]
fn only_exact_builtin_integer_scores_enable_live_commit() {
    assert!(score_capability::<SimpleScore>());
    assert!(score_capability::<crate::HardSoftScore>());
    assert!(score_capability::<crate::HardMediumSoftScore>());
    assert!(!std::mem::needs_drop::<CustomScore>());
    assert!(!score_capability::<CustomScore>());
    assert!(!score_capability::<crate::SimpleDecimalScore>());
}
