// Original regressions; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::{
    FilterNode, GroupNode, HardSoftScore, IntSummary, JoinNode, ScoreNode, SimpleScore,
    StructuralStreamContribution, TupleChange as Change,
};

#[derive(Debug)]
struct FragileKey {
    value: i32,
    clones_until_panic: std::rc::Rc<Cell<usize>>,
}
impl Clone for FragileKey {
    fn clone(&self) -> Self {
        let left = self.clones_until_panic.get();
        if left > 0 {
            self.clones_until_panic.set(left - 1);
            assert_ne!(left, 1, "key clone failed");
        }
        Self {
            value: self.value,
            clones_until_panic: self.clones_until_panic.clone(),
        }
    }
}
impl PartialEq for FragileKey {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for FragileKey {}
impl PartialOrd for FragileKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FragileKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

#[test]
fn key_clone_panics_during_commit_poison_join_and_group_nodes() {
    let count = std::rc::Rc::new(Cell::new(0));
    let mut groups = GroupNode::new();
    groups.apply(Change::put(
        0,
        (
            FragileKey {
                value: 1,
                clones_until_panic: count.clone(),
            },
            10,
        ),
    ));
    count.set(3); // Old/new keys are captured, then insertion clones during commit.
    assert!(
        catch_unwind(AssertUnwindSafe(|| groups.apply(Change::put(
            0,
            (
                FragileKey {
                    value: 2,
                    clones_until_panic: count.clone()
                },
                20
            )
        ))))
        .is_err()
    );
    assert!(groups.is_poisoned());
    assert!(catch_unwind(AssertUnwindSafe(|| groups.groups().count())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| groups.apply(Change::retract(0)))).is_err());

    count.set(0);
    let mut join: JoinNode<'_, i32, i32, i32, i32, FragileKey> = JoinNode::new(
        |&value| FragileKey {
            value,
            clones_until_panic: count.clone(),
        },
        |&value| FragileKey {
            value,
            clones_until_panic: count.clone(),
        },
    );
    join.apply_right(Change::put(0, 1));
    count.set(1); // Projection returns an owned key; the index clones it during commit.
    assert!(catch_unwind(AssertUnwindSafe(|| join.apply_left(Change::put(0, 1)))).is_err());
    assert!(join.is_poisoned());
    assert!(catch_unwind(AssertUnwindSafe(|| join.matches().count())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| join.apply_right(Change::retract(0)))).is_err());
}

#[test]
fn panicking_identity_order_poisoned_terminal_cannot_report_an_old_score() {
    struct Id(std::rc::Rc<Cell<usize>>);
    impl PartialEq for Id {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for Id {}
    impl PartialOrd for Id {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Id {
        fn cmp(&self, _: &Self) -> std::cmp::Ordering {
            let left = self.0.get();
            if left > 0 {
                self.0.set(left - 1);
                assert_ne!(left, 1, "identity ordering failed");
            }
            std::cmp::Ordering::Equal
        }
    }
    let count = std::rc::Rc::new(Cell::new(0));
    let mut score = ScoreNode::new(|&v| SimpleScore::new(v));
    score.apply(Change::put(Id(count.clone()), 1)).unwrap();
    count.set(2); // Lookup succeeds; committing the replacement panics.
    assert!(
        catch_unwind(AssertUnwindSafe(
            || score.apply(Change::put(Id(count.clone()), 2))
        ))
        .is_err()
    );
    assert!(score.is_poisoned());
    assert!(catch_unwind(AssertUnwindSafe(|| score.score())).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| score.contributions().count())).is_err());
}

fn apply_table<I: Ord, T>(
    table: &mut BTreeMap<I, T>,
    changes: impl IntoIterator<Item = Change<I, T>>,
) {
    for change in changes {
        if let Some(value) = change.value {
            table.insert(change.id, value);
        } else {
            table.remove(&change.id);
        }
    }
}

#[test]
fn filter_transitions_cache_rejections_and_do_not_replay_on_retraction() {
    let calls = Cell::new(0);
    let mut filter = FilterNode::new(|value: &i32| {
        calls.set(calls.get() + 1);
        *value > 0
    });
    assert_eq!(filter.apply(Change::put(1, -1)), None);
    assert_eq!(filter.apply(Change::put(1, 2)), Some(Change::put(1, 2)));
    assert_eq!(filter.apply(Change::put(1, 3)), Some(Change::put(1, 3)));
    assert_eq!(filter.apply(Change::put(1, 0)), Some(Change::retract(1)));
    assert_eq!(filter.apply(Change::retract(1)), None);
    assert_eq!(filter.apply(Change::retract(1)), None);
    assert_eq!(calls.get(), 4);
    assert_eq!(filter.rows().count(), 0);
}

#[test]
fn join_updates_evaluate_only_the_changed_key_and_matching_bucket() {
    let left_calls = Cell::new(0);
    let right_calls = Cell::new(0);
    let mut join = JoinNode::new(
        |r: &(i32, i32)| {
            left_calls.set(left_calls.get() + 1);
            r.0
        },
        |r: &(i32, i32)| {
            right_calls.set(right_calls.get() + 1);
            r.0
        },
    );
    let mut matches = BTreeMap::new();
    for id in 0..100 {
        apply_table(&mut matches, join.apply_right(Change::put(id, (id, 10))));
        apply_table(&mut matches, join.apply_left(Change::put(id, (id, 20))));
    }
    assert_eq!(matches.len(), 100);
    let changes = join.apply_left(Change::put(50, (51, 99)));
    assert_eq!(
        changes,
        vec![
            Change::retract((50, 50)),
            Change::put((50, 51), ((51, 99), (51, 10)))
        ]
    );
    apply_table(&mut matches, changes);
    assert_eq!((left_calls.get(), right_calls.get()), (101, 100));
    apply_table(&mut matches, join.apply_left(Change::retract(50)));
    apply_table(&mut matches, join.apply_left(Change::retract(50)));
    assert_eq!((left_calls.get(), right_calls.get()), (101, 100));
    assert_eq!(matches.len(), 99);
    let inspected: BTreeMap<_, _> = join
        .matches()
        .map(|((a, b), (l, r))| ((*a, *b), (*l, *r)))
        .collect();
    assert_eq!(matches, inspected);
}

#[test]
fn self_join_survives_alternating_left_and_right_first_updates() {
    let mut join = JoinNode::new(|x: &i32| *x, |x: &i32| *x);
    let mut table = BTreeMap::new();
    let mut input = BTreeMap::new();
    let mut seed = 20261010_u64;
    for step in 0..1500 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) % 13;
        let value = ((seed >> 48) % 6) as i32;
        let change = Change {
            id,
            value: (value < 5).then_some(value),
        };
        apply_table(&mut input, [change.clone()]);
        if step % 2 == 0 {
            apply_table(&mut table, join.apply_left(change.clone()));
            apply_table(&mut table, join.apply_right(change));
        } else {
            apply_table(&mut table, join.apply_right(change.clone()));
            apply_table(&mut table, join.apply_left(change));
        }
        let mut expected = BTreeMap::new();
        for (&a, &x) in &input {
            for (&b, &y) in &input {
                if x == y {
                    expected.insert((a, b), (x, y));
                }
            }
        }
        assert_eq!(table, expected, "step {step}");
    }
}

#[test]
fn groups_capture_old_values_and_emit_no_phantom_empty_groups() {
    let mut groups = GroupNode::new();
    assert!(
        groups
            .apply(Change::<i32, (&str, i64)>::retract(1))
            .is_empty()
    );
    let first = groups.apply(Change::put(1, ("a", i64::MAX)));
    assert_eq!(first[0].value.unwrap().sum, i64::MAX);
    groups.apply(Change::put(2, ("a", 1)));
    let replaced = groups.apply(Change::put(1, ("a", 2)));
    assert_eq!(replaced.len(), 1);
    assert_eq!(
        replaced[0].value.unwrap(),
        IntSummary {
            count: 2,
            distinct_count: 2,
            sum: 3,
            min: Some(1),
            max: Some(2)
        }
    );
    let moved = groups.apply(Change::put(1, ("b", 5)));
    assert_eq!(moved.iter().map(|d| d.id).collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(moved[0].value.unwrap().count, 1);
    assert_eq!(groups.apply(Change::retract(2)), [Change::retract("a")]);
    assert_eq!(groups.apply(Change::retract(1)), [Change::retract("b")]);
    assert_eq!(groups.groups().count(), 0);
}

#[test]
fn score_terminal_retracts_cached_contributions_and_rejects_structural_scores_atomically() {
    let calls = Cell::new(0);
    let mut score = ScoreNode::new(|value: &SimpleScore| {
        calls.set(calls.get() + 1);
        *value
    });
    score
        .apply(Change::put(1, SimpleScore::new(i64::MAX)))
        .unwrap();
    score.apply(Change::put(2, SimpleScore::new(1))).unwrap();
    assert_eq!(score.score(), SimpleScore::new(i64::MIN));
    for structural in [-1, 1] {
        assert_eq!(
            score.apply(Change::put(
                1,
                SimpleScore::new(0).with_structural(structural)
            )),
            Err(StructuralStreamContribution)
        );
        assert_eq!(score.score(), SimpleScore::new(i64::MIN));
        assert!(!score.is_poisoned());
    }
    score.apply(Change::put(1, SimpleScore::new(-4))).unwrap();
    assert_eq!(score.score(), SimpleScore::new(-3));
    let before = calls.get();
    score.apply(Change::retract(1)).unwrap();
    score.apply(Change::retract(1)).unwrap();
    score.apply(Change::retract(2)).unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(score.score(), SimpleScore::ZERO);
    assert_eq!(score.contributions().count(), 0);
}

#[test]
fn user_callback_panics_before_mutation_leave_nodes_usable() {
    let mut filter = FilterNode::new(|v: &i32| {
        assert_ne!(*v, 9);
        true
    });
    filter.apply(Change::put(0, 1));
    assert!(catch_unwind(AssertUnwindSafe(|| filter.apply(Change::put(0, 9)))).is_err());
    assert!(!filter.is_poisoned());
    assert_eq!(
        filter.rows().map(|(i, v)| (*i, *v)).collect::<Vec<_>>(),
        [(0, 1)]
    );
    let mut join = JoinNode::new(
        |v: &i32| {
            assert_ne!(*v, 9);
            *v
        },
        |v: &i32| *v,
    );
    join.apply_left(Change::put(0, 1));
    join.apply_right(Change::put(0, 1));
    assert!(catch_unwind(AssertUnwindSafe(|| join.apply_left(Change::put(0, 9)))).is_err());
    assert!(!join.is_poisoned());
    assert_eq!(join.matches().count(), 1);
    let mut score = ScoreNode::new(|v: &i32| {
        assert_ne!(*v, 9);
        SimpleScore::new(i64::from(*v))
    });
    score.apply(Change::put(0, 1)).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| score.apply(Change::put(0, 9)))).is_err());
    assert!(!score.is_poisoned());
    assert_eq!(score.score(), SimpleScore::new(1));
}

type Join = JoinNode<'static, usize, usize, (i64, i64), (i64, i64), i64>;
type Groups = GroupNode<(usize, usize), (i64, i64)>;
type Terminal = ScoreNode<'static, (i64, i64), ((i64, i64), IntSummary), HardSoftScore>;

type PairChange = Change<(usize, usize), ((i64, i64), (i64, i64))>;

fn deliver(changes: Vec<PairChange>, groups: &mut Groups, score: &mut Terminal) {
    for change in changes {
        // Group by machine and its current capacity; retain pair identities.
        for summary in groups.apply(change.map(|(job, capacity)| ((capacity.0, capacity.1), job.1)))
        {
            let key = summary.id;
            score.apply(summary.map(|value| (key, value))).unwrap();
        }
    }
}

#[test]
fn composed_filter_join_group_and_score_match_full_recomputation_for_2000_edits() {
    let mut filter = FilterNode::new(|job: &(i64, i64)| job.1 >= 0);
    let mut join = Join::new(|job| job.0, |resource| resource.0);
    let mut groups = Groups::new();
    let mut score = Terminal::new(|((_, capacity), s)| {
        HardSoftScore::new(-(s.sum - capacity).max(0), -(s.count as i64))
    });
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut seed = 20261011_u64;
    for step in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let is_left = step % 3 != 0;
        let id = (seed >> 32) as usize % if is_left { 17 } else { 5 };
        let key = ((seed >> 48) % 4) as i64;
        let value = ((seed >> 40) % 9) as i64 - 1;
        let change = Change {
            id,
            value: (!seed.is_multiple_of(7)).then_some((key, value)),
        };
        if is_left {
            apply_table(&mut left, [change.clone()]);
            if let Some(change) = filter.apply(change) {
                deliver(join.apply_left(change), &mut groups, &mut score);
            }
        } else {
            apply_table(&mut right, [change.clone()]);
            deliver(join.apply_right(change), &mut groups, &mut score);
        }
        let mut reference: BTreeMap<(i64, i64), Vec<i64>> = BTreeMap::new();
        let mut pairs = BTreeMap::new();
        for (&a, &(key, load)) in &left {
            if load < 0 {
                continue;
            }
            for (&b, &(machine, capacity)) in &right {
                if key == machine {
                    pairs.insert((a, b), ((key, load), (machine, capacity)));
                    reference.entry((machine, capacity)).or_default().push(load);
                }
            }
        }
        assert_eq!(
            join.matches()
                .map(|((a, b), (l, r))| ((*a, *b), (*l, *r)))
                .collect::<BTreeMap<_, _>>(),
            pairs
        );
        let mut expected_score = HardSoftScore::ZERO;
        let summaries: BTreeMap<_, _> = reference
            .into_iter()
            .map(|(key, mut values)| {
                let sum: i64 = values.iter().sum();
                let count = values.len();
                values.sort_unstable();
                let min = values.first().copied();
                let max = values.last().copied();
                values.dedup();
                expected_score =
                    expected_score + HardSoftScore::new(-(sum - key.1).max(0), -(count as i64));
                (
                    key,
                    IntSummary {
                        count,
                        distinct_count: values.len(),
                        sum,
                        min,
                        max,
                    },
                )
            })
            .collect();
        assert_eq!(
            groups
                .groups()
                .map(|(k, v)| (*k, v))
                .collect::<BTreeMap<_, _>>(),
            summaries,
            "step {step}"
        );
        assert_eq!(score.score(), expected_score, "step {step}");
        assert_eq!(score.contributions().count(), summaries.len());
    }
}
