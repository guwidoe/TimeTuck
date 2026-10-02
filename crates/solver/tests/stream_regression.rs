// Original regressions; no upstream compatibility credit.
use std::cell::Cell;
use std::hash::{Hash, Hasher};
use timetuck::{
    Assignment, ConstraintNetwork, ConstraintStream as Stream, ConstructionConfig, FiniteProblem,
    HardSoftScore, IntCollector, IntSummary, LocalSearchConfig, Move, ScoreDirector, SimpleScore,
    StreamScoreError, Variable, exhaustive_search, greedy_construction, local_search_from,
};

fn expected_summary(values: &[i64]) -> IntSummary {
    let mut distinct = values.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    IntSummary {
        count: values.len(),
        distinct_count: distinct.len(),
        sum: values.iter().fold(0_i64, |a, &b| a.wrapping_add(b)),
        min: values.iter().copied().min(),
        max: values.iter().copied().max(),
    }
}

#[test]
fn collector_matches_independent_multiset_after_4096_seeded_edits() {
    let mut seed = 20261008_u64;
    let choices = [i64::MIN, i64::MAX, -2, -1, 0, 1, 2];
    let mut values = Vec::new();
    let mut collector = IntCollector::new();
    for step in 0..4096 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let value = choices[(seed >> 32) as usize % choices.len()];
        if seed >> 63 == 0 {
            values.push(value);
            collector.insert(value);
        } else {
            let position = values.iter().position(|&v| v == value);
            assert_eq!(collector.remove(value), position.is_some());
            if let Some(position) = position {
                values.remove(position);
            }
        }
        assert_eq!(
            collector.summary(),
            expected_summary(&values),
            "step {step}"
        );
    }
    for value in values {
        assert!(collector.remove(value));
    }
    assert_eq!(collector.summary(), IntSummary::default());
}

#[test]
fn collector_absent_retraction_is_atomic_and_clones_are_independent() {
    let mut collector: IntCollector = [i64::MAX, 1, 1].into_iter().collect();
    let before = collector.clone();
    assert!(!collector.remove(0));
    assert_eq!(collector, before);
    assert_eq!(collector.summary().sum, i64::MIN + 1);
    assert!(collector.remove(1));
    assert_eq!(collector.summary().sum, i64::MIN);
    assert!(collector.remove(i64::MAX));
    assert_eq!(collector.summary().sum, 1);
    assert_eq!(before.summary().count, 3);
}

#[test]
fn mapping_flattening_concat_and_distinct_preserve_bag_semantics() {
    let rows = Stream::new([3, 1, 3, 2])
        .filter(|&v| v != 2)
        .map(|v| v % 2)
        .flat_map(|v| [v, v + 1])
        .concat(Stream::new([2, 0]));
    assert_eq!(rows.as_slice(), &[1, 2, 1, 2, 1, 2, 2, 0]);
    assert_eq!(
        rows.score(|_| SimpleScore::new(-1)),
        Ok(SimpleScore::new(-8))
    );
    assert_eq!(rows.distinct().into_vec(), vec![1, 2, 0]);
    assert!(
        Stream::new([1, 2])
            .flat_map(|_| Vec::<i32>::new())
            .is_empty()
    );
}

#[test]
fn unique_pairs_use_positions_and_self_join_includes_diagonal() {
    let rows = Stream::new([1, 1, 2]);
    assert_eq!(
        rows.clone().unique_pairs().into_vec(),
        [(1, 1), (1, 2), (1, 2)]
    );
    assert_eq!(
        rows.clone().join_equal(&rows, |&v| v, |&v| v).into_vec(),
        [(1, 1), (1, 1), (1, 1), (1, 1), (2, 2)]
    );
    assert!(Stream::new([1]).unique_pairs().is_empty());
    assert!(Stream::<i32>::new([]).unique_pairs().is_empty());
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollisionKey(i32);
impl Hash for CollisionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0_u8.hash(state);
    }
}

#[test]
fn indexed_joins_match_nested_reference_even_with_duplicates_and_hash_collisions() {
    let mut seed = 20261009_u64;
    for case in 0..256 {
        let mut generate = |count| {
            (0..count)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    ((seed >> 32) % 5) as i32 - 2
                })
                .collect::<Vec<_>>()
        };
        let left = generate(case % 11);
        let right = generate((case / 11) % 11);
        let mut expected = Vec::new();
        for &a in &left {
            for &b in &right {
                if a == b {
                    expected.push((a, b));
                }
            }
        }
        let right = Stream::new(right);
        let calls = Cell::new(0);
        let indexed = Stream::new(left.clone()).join_equal(
            &right,
            |&v| CollisionKey(v),
            |&v| {
                calls.set(calls.get() + 1);
                CollisionKey(v)
            },
        );
        assert_eq!(indexed.into_vec(), expected, "case {case}");
        assert_eq!(calls.get(), right.len());
        assert_eq!(
            Stream::new(left).join(&right, |a, b| a == b).into_vec(),
            expected
        );
    }
}

#[test]
fn tuple_keys_chained_joins_and_borrowed_facts_keep_identity_and_order() {
    let people = [("Pat", 1, 2), ("Sam", 1, 3), ("Pat", 1, 3)];
    let requirements = Stream::new([((1, 3), "A"), ((1, 3), "B"), ((1, 2), "C")]);
    let matches = Stream::new(people.iter())
        .join_equal(&requirements, |p| (p.1, p.2), |r| r.0)
        .join_equal(&Stream::new([("A", 10), ("C", 20)]), |(_, r)| r.1, |r| r.0)
        .map(|((p, r), (_, weight))| (p.0, r.1, weight));
    assert_eq!(
        matches.into_vec(),
        [("Pat", "C", 20), ("Sam", "A", 10), ("Pat", "A", 10)]
    );
}

#[test]
fn exists_is_a_semijoin_and_other_identity_is_explicit() {
    let left = Stream::new([(0, 1), (1, 1), (2, 2), (2, 2)]);
    let right = Stream::new([1, 1, 1]);
    assert_eq!(left.clone().if_exists(&right, |a, b| a.1 == *b).len(), 2);
    assert_eq!(
        left.clone()
            .if_not_exists(&right, |a, b| a.1 == *b)
            .into_vec(),
        [(2, 2), (2, 2)]
    );
    assert_eq!(
        left.clone()
            .if_exists(&left, |a, b| a.0 != b.0 && a.1 == b.1)
            .into_vec(),
        [(0, 1), (1, 1)]
    );
    assert!(
        left.clone()
            .if_exists(&Stream::<i32>::new([]), |_, _| true)
            .is_empty()
    );
    assert_eq!(
        left.clone()
            .if_not_exists(&Stream::<i32>::new([]), |_, _| true),
        left
    );
}

#[test]
fn grouping_handles_collisions_duplicates_first_occurrence_and_empty_input() {
    let rows = Stream::new([(2, 4), (1, 7), (2, 4), (3, -2), (1, 0)]);
    let groups = rows.clone().group_by(|r| CollisionKey(r.0));
    assert_eq!(
        groups.into_vec(),
        [
            (CollisionKey(2), vec![(2, 4), (2, 4)]),
            (CollisionKey(1), vec![(1, 7), (1, 0)]),
            (CollisionKey(3), vec![(3, -2)]),
        ]
    );
    let mut visits = Vec::new();
    let groups = rows.group_collect(
        |r| CollisionKey(r.0),
        |r| {
            visits.push(*r);
            r.1
        },
    );
    assert_eq!(visits, [(2, 4), (1, 7), (2, 4), (3, -2), (1, 0)]);
    assert_eq!(
        groups.into_vec(),
        [
            (CollisionKey(2), expected_summary(&[4, 4])),
            (CollisionKey(1), expected_summary(&[7, 0])),
            (CollisionKey(3), expected_summary(&[-2])),
        ]
    );
    let empty = Stream::<i64>::new([]);
    assert!(empty.clone().group_by(|v| *v).is_empty());
    assert!(empty.clone().group_collect(|v| *v, |v| *v).is_empty());
    assert_eq!(empty.collect_int(|v| *v), IntSummary::default());
}

#[test]
fn scores_handle_multilevel_weights_overflow_and_empty_streams() {
    let rows = Stream::new([i64::MAX, 1]);
    assert_eq!(
        rows.score(|&v| SimpleScore::new(v)),
        Ok(SimpleScore::new(i64::MIN))
    );
    assert_eq!(
        Stream::new([2, 3]).score(|&v| HardSoftScore::new(-v, 2 * v)),
        Ok(HardSoftScore::new(-5, 10))
    );
    assert_eq!(
        Stream::<()>::new([]).score(|_| panic!("empty must not score")),
        Ok(SimpleScore::ZERO)
    );
}

#[test]
fn structural_contributions_are_rejected_before_they_can_be_discarded() {
    for structural in [-1, 1] {
        let mut calls = 0;
        let score = Stream::new([0, structural, 0]).score(|&v| {
            calls += 1;
            SimpleScore::new(1).with_structural(v)
        });
        assert_eq!(score, Err(StreamScoreError { row: 1 }));
        assert_eq!(calls, 2);
    }
}

const DURATIONS: [i64; 4] = [3, 2, 2, 1];
const PREFERRED: [usize; 4] = [0, 1, 1, 0];
const CONFLICTS: [(usize, usize); 4] = [(0, 1), (0, 2), (1, 3), (2, 3)];

fn scheduling_network() -> ConstraintNetwork<'static, usize, HardSoftScore> {
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("loads", (0..4).collect(), |values| {
            Stream::new(
                values
                    .iter()
                    .enumerate()
                    .filter_map(|(id, v)| v.map(|&m| (id, m))),
            )
            .group_collect(|r| r.1, |r| DURATIONS[r.0])
            .score(|(_, summary)| HardSoftScore::new(-(summary.sum - 4).max(0), 0))
            .unwrap()
        })
        .unwrap();
    network
        .add_constraint("conflicts", (0..4).collect(), |values| {
            let assigned = Stream::new(
                values
                    .iter()
                    .enumerate()
                    .filter_map(|(id, v)| v.map(|&m| (id, m))),
            );
            assigned
                .clone()
                .join_equal(&assigned, |r| r.1, |r| r.1)
                .filter(|(a, b)| a.0 < b.0)
                .if_exists(&Stream::new(CONFLICTS), |(a, b), &(x, y)| {
                    a.0 == x && b.0 == y
                })
                .score(|_| HardSoftScore::new(-1, 0))
                .unwrap()
        })
        .unwrap();
    network
        .add_constraint("preferences", (0..4).collect(), |values| {
            Stream::new(
                values
                    .iter()
                    .enumerate()
                    .filter_map(|(id, v)| v.map(|&m| (id, m))),
            )
            .score(|&(id, machine)| HardSoftScore::new(0, -i64::from(machine != PREFERRED[id])))
            .unwrap()
        })
        .unwrap();
    network
}

// Deliberately uses loops and fixed-size arrays, no streams or collectors.
fn reference_schedule(values: &[Option<usize>]) -> HardSoftScore {
    let mut loads = [0_i64; 2];
    let mut soft = 0;
    let mut structural = 0;
    for id in 0..4 {
        if let Some(machine) = values[id] {
            loads[machine] += DURATIONS[id];
            soft -= i64::from(machine != PREFERRED[id]);
        } else {
            structural -= 1;
        }
    }
    let mut hard = -(loads[0] - 4).max(0) - (loads[1] - 4).max(0);
    for (a, b) in CONFLICTS {
        if values[a].is_some() && values[a] == values[b] {
            hard -= 1;
        }
    }
    HardSoftScore::new(hard, soft).with_structural(structural)
}

#[test]
fn all_partial_assignments_and_move_transactions_match_independent_scoring() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let network = scheduling_network();
    let mut director =
        ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    for state in 0..81 {
        let mut digits = state;
        let values: Vec<_> = (0..4)
            .map(|_| {
                let digit = digits % 3;
                digits /= 3;
                match digit {
                    0 => None,
                    1 => Some(0),
                    _ => Some(1),
                }
            })
            .collect();
        let assignment = Assignment::new(values.clone());
        let expected = reference_schedule(&values);
        assert_eq!(network.calculate(&problem, &assignment).unwrap(), expected);
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let previous = director.assignment().clone();
        let old_score = director.score();
        {
            let candidate = director.apply(&movement).unwrap();
            assert_eq!(candidate.score(), expected);
            assert_eq!(candidate.constraint_evaluations(), 3);
        }
        assert_eq!(director.assignment(), &previous);
        assert_eq!(director.score(), old_score);
        director.apply(&movement).unwrap().commit();
        assert_eq!(director.score(), expected);
        director.assert_consistent().unwrap();
    }
}

#[test]
fn stream_constraints_work_with_exact_construction_and_local_search() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap();
    let network = scheduling_network();
    let exact = exhaustive_search(&problem, |a| network.calculate(&problem, a).unwrap(), 16);
    assert!(exact.is_optimal());
    assert_eq!(exact.evaluations, 16);
    assert_eq!(exact.best.unwrap().score, HardSoftScore::ZERO);
    let constructed = greedy_construction(
        &problem,
        &network,
        ConstructionConfig {
            max_evaluations: 9,
            assert_scores: true,
            ..ConstructionConfig::default()
        },
    )
    .unwrap();
    assert!(constructed.complete);
    assert_eq!(constructed.score, Some(HardSoftScore::ZERO));
    // Independently exercise improvement from a poor complete assignment.
    let improved = local_search_from(
        &problem,
        &network,
        Assignment::new(vec![Some(0); 4]),
        LocalSearchConfig {
            max_evaluations: 200,
            assert_scores: true,
            ..LocalSearchConfig::default()
        },
    )
    .unwrap();
    let best = improved.best.unwrap();
    assert_eq!(best.score, HardSoftScore::ZERO);
    assert_eq!(reference_schedule(best.assignment.values()), best.score);
    assert!(improved.accepted_moves > 0);
}
