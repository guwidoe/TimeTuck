//! Original regressions; no upstream-method coverage credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn heterogeneous_ids_nullable_payloads_and_same_ids_remain_independent() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<usize, Option<i64>>();
    let right = b.source::<String, Option<i64>>();
    let port = b.concat(left.port(), right.port()).unwrap();
    let c = b.score("all", &port, |_| SimpleScore::new(-1)).unwrap();
    let unique = b.distinct(&port).unwrap();
    let u = b.score("unique", &unique, |_| SimpleScore::ZERO).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&left, TupleChange::put(7, None)).unwrap();
    t.apply(&right, TupleChange::put("7".into(), None)).unwrap();
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&c).unwrap(),
        &BTreeMap::from([
            (ConcatId::Left(7), (None, SimpleScore::new(-1))),
            (ConcatId::Right("7".into()), (None, SimpleScore::new(-1)))
        ])
    );
    assert_eq!(g.constraint_matches(&u).unwrap().len(), 1);
    let mut t = g.transaction();
    t.apply(&left, TupleChange::retract(7)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.constraint_matches(&c).unwrap().len(), 1);
    assert_eq!(g.constraint_matches(&u).unwrap().len(), 1);
    let mut t = g.transaction();
    t.apply(&right, TupleChange::put("7".into(), Some(9)))
        .unwrap();
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&u).unwrap(),
        &BTreeMap::from([(Some(9), (Some(9), SimpleScore::ZERO))])
    );
    let mut t = g.transaction();
    t.apply(&right, TupleChange::retract("7".into())).unwrap();
    t.apply(&left, TupleChange::retract(7)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    g.assert_consistent().unwrap();
}
#[test]
fn nested_self_concatenation_preserves_every_branch_tag_and_rolls_back() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let twice = b.concat(s.port(), s.port()).unwrap();
    let thrice = b.concat(&twice, s.port()).unwrap();
    let c = b.score("three", &thrice, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 5)).unwrap();
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&c).unwrap(),
        &BTreeMap::from([
            (ConcatId::Left(ConcatId::Left(0)), (5, SimpleScore::new(5))),
            (ConcatId::Left(ConcatId::Right(0)), (5, SimpleScore::new(5))),
            (ConcatId::Right(0), (5, SimpleScore::new(5)))
        ])
    );
    let before = g.constraint_matches(&c).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, -2)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(-6));
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    g.assert_consistent().unwrap();
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn generated_shared_branch_updates_match_full_tagged_multisets_and_groups() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (bool, i64)>();
    let left = b.filter(s.port(), |(enabled, _)| *enabled).unwrap();
    let left = b.map(&left, |(_, v)| *v).unwrap();
    let right = b.map(s.port(), |(_, v)| *v).unwrap();
    let right = b
        .flat_map(&right, |v| if *v >= 0 { vec![*v, *v] } else { vec![] })
        .unwrap();
    let combined = b.concat(&left, &right).unwrap();
    let raw = b.score("raw", &combined, |v| SimpleScore::new(*v)).unwrap();
    let groups = b
        .group_collect(&combined, |v| (v.rem_euclid(3), *v))
        .unwrap();
    let grouped = b.score("groups", &groups, |_| SimpleScore::ZERO).unwrap();
    let mut g = b.build();
    let mut inputs = BTreeMap::<usize, (bool, i64)>::new();
    let mut seed = 20261110;
    for step in 0..3000 {
        let id = (random(&mut seed) % 32) as usize;
        let value = (!random(&mut seed).is_multiple_of(5)).then(|| {
            (
                random(&mut seed).is_multiple_of(2),
                (random(&mut seed) % 17) as i64 - 8,
            )
        });
        let before = g.constraint_matches(&raw).unwrap().clone();
        let mut candidate = inputs.clone();
        if let Some(v) = value {
            candidate.insert(id, v);
        } else {
            candidate.remove(&id);
        }
        let mut expected = BTreeMap::new();
        for (&id, &(enabled, value)) in &candidate {
            if enabled {
                expected.insert(ConcatId::Left(id), (value, SimpleScore::new(value)));
            }
            if value >= 0 {
                for slot in 0..2 {
                    expected.insert(
                        ConcatId::Right((id, slot)),
                        (value, SimpleScore::new(value)),
                    );
                }
            }
        }
        let mut t = g.transaction();
        t.apply(&s, TupleChange { id, value }).unwrap();
        assert_eq!(
            t.graph().unwrap().constraint_matches(&raw).unwrap(),
            &expected,
            "step {step}"
        );
        assert_eq!(
            t.score().unwrap(),
            SimpleScore::new(expected.values().map(|x| x.0).sum())
        );
        let mut reference = BTreeMap::<i64, Vec<i64>>::new();
        for (v, _) in expected.values() {
            reference.entry(v.rem_euclid(3)).or_default().push(*v);
        }
        let actual = t.graph().unwrap().constraint_matches(&grouped).unwrap();
        assert_eq!(actual.len(), reference.len());
        for (key, rows) in reference {
            let ((actual_key, summary), score) = &actual[&key];
            assert_eq!(*actual_key, key);
            assert_eq!(*score, SimpleScore::ZERO);
            assert_eq!(summary.count, rows.len());
            assert_eq!(summary.sum, rows.iter().sum());
            assert_eq!(summary.min, rows.iter().min().copied());
            assert_eq!(summary.max, rows.iter().max().copied());
        }
        if step % 4 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&raw).unwrap(), &before);
        } else {
            t.commit().unwrap();
            inputs = candidate;
        }
        if step % 31 == 0 {
            g.assert_consistent().unwrap();
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn errors_after_one_branch_propagates_restore_all_matches_and_source_state() {
    let fault = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let right = b.map(s.port(), |v| v + 10).unwrap();
    let combined = b.concat(s.port(), &right).unwrap();
    let c = b
        .score("values", &combined, |v| {
            assert!(!(fault.get() == 2 && *v >= 10), "right branch weight panic");
            SimpleScore::new(*v).with_structural(if fault.get() == 1 && *v >= 10 { -1 } else { 0 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 1)).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&c).unwrap().clone();
    for mode in [1, 2] {
        fault.set(mode);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            assert!(t.apply(&s, TupleChange::put(0, 3)).is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(result.is_err(), mode == 2);
        fault.set(0);
        assert_eq!(g.constraint_matches(&c).unwrap(), &before);
        assert_eq!(g.source_rows(&s).unwrap()[&0], 1);
        g.assert_consistent().unwrap();
    }
    fault.set(1);
    assert!(matches!(
        g.assert_consistent(),
        Err(GraphError::StructuralContribution(_))
    ));
    fault.set(0);
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    g.assert_consistent().unwrap();
}
#[test]
fn both_foreign_operands_are_rejected_before_registering_connections() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let s = a.source::<usize, i64>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = b.source::<usize, i64>();
    assert!(matches!(
        a.concat(s.port(), foreign.port()),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        a.concat(foreign.port(), s.port()),
        Err(GraphError::ForeignPort)
    ));
    let valid = a.concat(s.port(), s.port()).unwrap();
    a.score("valid", &valid, |v| SimpleScore::new(*v)).unwrap();
    let mut g = a.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 2)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(4));
    g.assert_consistent().unwrap();
}
#[test]
fn construction_and_search_keep_best_concatenated_candidates() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        let assigned = b.filter(s.port(), |(_, v)| v.is_some()).unwrap();
        let values = b.map(&assigned, |(_, v)| v.unwrap()).unwrap();
        let second = b.filter(&values, |&v| v > 0).unwrap();
        let combined = b.concat(&values, &second).unwrap();
        b.score("usage", &combined, |v| SimpleScore::new(-(*v as i64)))
            .unwrap();
        (b.build(), s)
    };
    let p = FiniteProblem::new((0..4).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
    let (g, s) = build();
    let c = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(c.complete);
    assert_eq!(c.score, Some(SimpleScore::ZERO));
    let p = FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(2))
            .collect(),
    )
    .unwrap();
    let (g, s) = build();
    let best = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            max_evaluations: 200,
            max_steps: 20,
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert!(p.is_complete(&best.assignment));
    assert_eq!(best.score, SimpleScore::ZERO);
    assert_eq!(
        best.score,
        SimpleScore::new(
            -2 * best
                .assignment
                .values()
                .iter()
                .map(|v| v.unwrap() as i64)
                .sum::<i64>()
        )
    );
}
