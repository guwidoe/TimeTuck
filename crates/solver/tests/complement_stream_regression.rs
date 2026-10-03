//! Original regressions; no upstream-method coverage credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn duplicate_keys_last_match_removal_and_nullable_keys_preserve_occurrences() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let input = b.source::<usize, (Option<i32>, i64)>();
    let universe = b.source::<String, (Option<i32>, i64)>();
    let port = b
        .complement(input.port(), universe.port(), |x| x.0, |x| x.0, |x| *x)
        .unwrap();
    let c = b.score("all", &port, |x| SimpleScore::new(x.1)).unwrap();
    let mut g = b.build();
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    let mut t = g.transaction();
    for (id, row) in [
        ("a", (None, -10)),
        ("b", (None, -20)),
        ("c", (Some(1), -30)),
    ] {
        t.apply(&universe, TupleChange::put(id.into(), row))
            .unwrap();
    }
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-60));
    let mut t = g.transaction();
    t.apply(&input, TupleChange::put(0, (None, 2))).unwrap();
    t.apply(&input, TupleChange::put(1, (None, 3))).unwrap();
    t.apply(&input, TupleChange::put(2, (Some(9), 4))).unwrap();
    t.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&c).unwrap(),
        &BTreeMap::from([
            (ConcatId::Left(0), ((None, 2), SimpleScore::new(2))),
            (ConcatId::Left(1), ((None, 3), SimpleScore::new(3))),
            (ConcatId::Left(2), ((Some(9), 4), SimpleScore::new(4))),
            (
                ConcatId::Right("c".into()),
                ((Some(1), -30), SimpleScore::new(-30))
            )
        ])
    );
    let mut t = g.transaction();
    t.apply(&input, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-23));
    let mut t = g.transaction();
    t.apply(&input, TupleChange::retract(1)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-56));
    let mut t = g.transaction();
    t.apply(&universe, TupleChange::put("a".into(), (Some(9), -100)))
        .unwrap();
    t.apply(&universe, TupleChange::retract("b".into()))
        .unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-26));
    g.assert_consistent().unwrap();
}
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn generated_two_source_edits_match_full_complement_through_commit_and_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let input = b.source::<usize, (i32, i64)>();
    let universe = b.source::<usize, (i32, i64)>();
    let port = b
        .complement(
            input.port(),
            universe.port(),
            |r| r.0,
            |r| r.0,
            |r| (r.0, -r.1),
        )
        .unwrap();
    let c = b.score("all", &port, |r| SimpleScore::new(r.1)).unwrap();
    let mut g = b.build();
    let mut inputs = BTreeMap::new();
    let mut facts = BTreeMap::new();
    let mut seed = 20261111;
    for step in 0..3000 {
        let before = g.constraint_matches(&c).unwrap().clone();
        let mut next_inputs = inputs.clone();
        let mut next_facts = facts.clone();
        let mut t = g.transaction();
        for _ in 0..1 + random(&mut seed) % 4 {
            let id = (random(&mut seed) % 20) as usize;
            let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                (
                    (random(&mut seed) % 7) as i32,
                    (random(&mut seed) % 31) as i64 - 15,
                )
            });
            let side = random(&mut seed).is_multiple_of(2);
            let reference = if side {
                &mut next_inputs
            } else {
                &mut next_facts
            };
            if let Some(v) = value {
                reference.insert(id, v);
            } else {
                reference.remove(&id);
            }
            t.apply(
                if side { &input } else { &universe },
                TupleChange { id, value },
            )
            .unwrap();
            let keys = next_inputs.values().map(|r| r.0).collect::<BTreeSet<_>>();
            let mut expected = next_inputs
                .iter()
                .map(|(&id, &r)| (ConcatId::Left(id), (r, SimpleScore::new(r.1))))
                .collect::<BTreeMap<_, _>>();
            for (&id, &(key, value)) in &next_facts {
                if !keys.contains(&key) {
                    expected.insert(
                        ConcatId::Right(id),
                        ((key, -value), SimpleScore::new(-value)),
                    );
                }
            }
            assert_eq!(
                t.graph().unwrap().constraint_matches(&c).unwrap(),
                &expected,
                "step {step}"
            );
            assert_eq!(
                t.score().unwrap(),
                SimpleScore::new(expected.values().map(|(r, _)| r.1).sum())
            );
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&c).unwrap(), &before);
        } else {
            t.commit().unwrap();
            inputs = next_inputs;
            facts = next_facts;
        }
        if step % 37 == 0 {
            g.assert_consistent().unwrap();
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn shared_self_and_filtered_universes_reconcile_transient_missing_rows() {
    let padding_calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let self_complete = b
        .complement(
            source.port(),
            source.port(),
            |r| r.0,
            |r| r.0,
            |r| {
                padding_calls.set(padding_calls.get() + 1);
                *r
            },
        )
        .unwrap();
    let identity = b
        .score("identity", &self_complete, |_| SimpleScore::ZERO)
        .unwrap();
    let selected = b.filter(source.port(), |r| r.1 > 0).unwrap();
    let complete = b
        .complement(&selected, source.port(), |r| r.0, |r| r.0, |r| (r.0, -100))
        .unwrap();
    let c = b
        .score("selected", &complete, |r| SimpleScore::new(r.1))
        .unwrap();
    let mut g = b.build();
    for (value, expected) in [(4, 4), (-3, -100), (2, 2)] {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::put(0, (7, value))).unwrap();
        t.commit().unwrap();
        assert_eq!(
            g.constraint_matches(&identity).unwrap(),
            &BTreeMap::from([(ConcatId::Left(0), ((7, value), SimpleScore::ZERO))])
        );
        assert_eq!(g.score(), SimpleScore::new(expected));
        assert_eq!(g.constraint_matches(&c).unwrap().len(), 1);
        g.assert_consistent().unwrap();
    }
    assert!(
        padding_calls.get() > 0,
        "shared input delivery can transiently require padding"
    );
    let before = g.constraint_matches(&c).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::retract(0)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::ZERO);
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    g.assert_consistent().unwrap();
}
#[test]
fn padding_key_and_scoring_failures_restore_presence_indexes_and_cached_matches() {
    let fault = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let input = b.source::<usize, (i32, i64)>();
    let universe = b.source::<usize, i32>();
    let completed = b
        .complement(
            input.port(),
            universe.port(),
            |r| {
                assert_ne!(fault.get(), 4, "input key panic");
                r.0
            },
            |v| {
                assert_ne!(fault.get(), 5, "universe key panic");
                *v
            },
            |v| {
                assert_ne!(fault.get(), 1, "padding panic");
                (*v, -10)
            },
        )
        .unwrap();
    let c = b
        .score("all", &completed, |r| {
            assert!(!(fault.get() == 2 && r.1 == -10), "padded score panic");
            SimpleScore::new(r.1).with_structural(if fault.get() == 3 && r.1 == -10 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&input, TupleChange::put(0, (7, 1))).unwrap();
    t.apply(&universe, TupleChange::put(0, 7)).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&c).unwrap().clone();
    for mode in 1..=5 {
        fault.set(mode);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            let result = match mode {
                4 => t.apply(&input, TupleChange::put(0, (8, 2))),
                5 => t.apply(&universe, TupleChange::put(0, 8)),
                _ => t.apply(&input, TupleChange::retract(0)),
            };
            assert!(result.is_err());
            assert!(t.commit().is_err());
        }));
        assert_eq!(outcome.is_err(), mode != 3);
        fault.set(0);
        assert_eq!(g.constraint_matches(&c).unwrap(), &before);
        assert_eq!(g.source_rows(&input).unwrap()[&0], (7, 1));
        assert_eq!(g.source_rows(&universe).unwrap()[&0], 7);
        g.assert_consistent().unwrap();
    }
    fault.set(4);
    assert!(catch_unwind(AssertUnwindSafe(|| g.assert_consistent())).is_err());
    fault.set(0);
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    let mut t = g.transaction();
    t.apply(&input, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-10));
    g.assert_consistent().unwrap();
}
#[test]
fn foreign_ports_are_rejected_before_composite_topology_registration() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let source = a.source::<usize, i64>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let foreign = b.source::<usize, i64>();
    assert!(matches!(
        a.complement(source.port(), foreign.port(), |v| *v, |v| *v, |v| *v),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        a.complement(foreign.port(), source.port(), |v| *v, |v| *v, |v| *v),
        Err(GraphError::ForeignPort)
    ));
    let port = a
        .complement(source.port(), source.port(), |v| *v, |v| *v, |v| *v)
        .unwrap();
    a.score("all", &port, |v| SimpleScore::new(*v)).unwrap();
    let mut g = a.build();
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
}
#[test]
fn construction_and_search_cover_required_skills_with_universe_padding() {
    let build = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let universe = b.source::<usize, usize>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let skills = b.map(&assigned, |(_, v)| (v.unwrap(), 0_i64)).unwrap();
        let coverage = b
            .complement(&skills, universe.port(), |r| r.0, |v| *v, |v| (*v, -1))
            .unwrap();
        b.score("uncovered", &coverage, |r| SimpleScore::new(r.1))
            .unwrap();
        let mut g = b.build();
        let mut t = g.transaction();
        for skill in 0..3 {
            t.apply(&universe, TupleChange::put(skill, skill)).unwrap();
        }
        t.commit().unwrap();
        (g, source)
    };
    let p = FiniteProblem::new((0..3).map(|_| Variable::new(vec![0, 1, 2])).collect()).unwrap();
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
        (0..3)
            .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let (g, s) = build();
    let best = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            assert_scores: true,
            max_evaluations: 200,
            max_steps: 20,
            ..Default::default()
        },
    )
    .unwrap()
    .best
    .unwrap();
    assert!(p.is_complete(&best.assignment));
    assert_eq!(best.score, SimpleScore::ZERO);
    let used = best
        .assignment
        .values()
        .iter()
        .map(|v| v.unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(used, BTreeSet::from([0, 1, 2]));
    assert_eq!(
        best.score,
        SimpleScore::new(-((0..3).filter(|s| !used.contains(s)).count() as i64))
    );
}
