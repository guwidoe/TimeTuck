//! Original identity, ordering, atomicity and graph regressions.
use std::{
    cell::Cell,
    cmp::Reverse,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Payload(i64);
#[test]
fn ties_duplicates_nullable_payloads_and_captured_retraction() {
    let mut min = MinByCollector::<usize, i64, Option<Payload>>::default();
    let mut max = MaxByCollector::<usize, i64, Option<Payload>>::default();
    for row in [
        (7, 2, Some(Payload(4))),
        (2, 2, None),
        (2, 2, None),
        (9, -5, Some(Payload(3))),
    ] {
        min.insert(&row);
        max.insert(&row);
    }
    assert_eq!(min.finish().unwrap().id, 9);
    let saved = max.finish().unwrap();
    assert_eq!(saved.id, 2);
    assert_eq!(saved.value, None);
    {
        let c = &mut max;
        assert!(c.remove(&(2, 99, Some(Payload(999)))));
        assert_eq!(c.finish().unwrap().id, 2);
        assert!(c.remove(&(2, 99, None)));
        assert_eq!(c.finish().unwrap().id, 7);
        assert!(!c.remove(&(2, 2, None)));
    }
    assert_eq!(saved.id, 2);
    let mut reversed = MinByCollector::<usize, (Reverse<i64>, i64), Payload>::default();
    for row in [
        (0, (Reverse(i64::MIN), 0), Payload(0)),
        (1, (Reverse(i64::MAX), 2), Payload(1)),
        (2, (Reverse(i64::MAX), 1), Payload(2)),
    ] {
        reversed.insert(&row);
    }
    assert_eq!(reversed.finish().unwrap().id, 2);
}
#[test]
fn conflicting_identity_insert_and_replace_are_atomic() {
    let mut c = MinByCollector::<usize, i64, Payload>::default();
    c.insert(&(0, 3, Payload(4)));
    c.insert(&(0, 3, Payload(4)));
    c.insert(&(1, 2, Payload(8)));
    let saved = c.clone();
    for replacement in [false, true] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if replacement {
                    c.replace(&(1, 0, Payload(0)), &(0, 3, Payload(9)));
                } else {
                    c.insert(&(0, 4, Payload(4)));
                }
            }))
            .is_err()
        );
        assert_eq!(c, saved);
    }
    assert!(!c.replace(&(99, 0, Payload(0)), &(0, 9, Payload(0))));
    assert_eq!(c, saved);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || c.replace(&(0, 3, Payload(4)), &(0, 4, Payload(5)))
        ))
        .is_err()
    );
    assert_eq!(c, saved);
    c.remove(&(0, 0, Payload(0)));
    assert!(c.replace(&(0, 0, Payload(0)), &(0, -4, Payload(5))));
    assert_eq!(c.finish().unwrap().value, Payload(5));
    assert_eq!(saved.finish().unwrap().id, 1);
}
#[test]
fn grouped_extrema_trials_and_reconstruction_match_full_sort() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, usize, i64, Payload)>();
    type C =
        PairCollector<MinByCollector<usize, i64, Payload>, MaxByCollector<usize, i64, Payload>>;
    let p = b
        .group_collect_with::<_, _, _, _, C>(s.port(), |r| {
            (r.0, ((r.1, r.2, r.3.clone()), (r.1, r.2, r.3.clone())))
        })
        .unwrap();
    let h = b
        .score("extrema", &p, |(_, (a, z))| {
            SimpleScore::new(a.as_ref().unwrap().value.0 - z.as_ref().unwrap().value.0)
        })
        .unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261225u64;
    for step in 0..600 {
        let saved = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 24) as usize;
            let value = (!draw().is_multiple_of(4)).then(|| {
                (
                    (draw() % 4) as u8,
                    id,
                    (draw() % 7) as i64 - 3,
                    Payload((draw() % 99) as i64),
                )
            });
            if let Some(ref v) = value {
                next.insert(id, v.clone());
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, Vec<_>>::new();
            for r in next.values() {
                groups.entry(r.0).or_default().push(r);
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut total = 0;
            for (k, mut v) in groups {
                v.sort_by_key(|r| (r.2, r.1));
                let a = v[0];
                v.sort_by_key(|r| (Reverse(r.2), r.1));
                let z = v[0];
                let expected = |r: &(u8, usize, i64, Payload)| {
                    Some(Extremum {
                        id: r.1,
                        key: r.2,
                        value: r.3.clone(),
                    })
                };
                assert_eq!(actual[&k].0.1, (expected(a), expected(z)));
                total += a.3.0 - z.3.0;
            }
            assert_eq!(t.score().unwrap(), SimpleScore::new(total));
        }
        if step % 17 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        } else {
            t.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn mapping_and_downstream_failures_restore_captured_payloads() {
    let fail = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, i64, Payload)>();
    let p = b
        .collect::<_, _, _, MinByCollector<usize, i64, Payload>>(s.port(), |r| {
            assert_ne!(fail.get(), 1);
            calls.set(calls.get() + 1);
            r.clone()
        })
        .unwrap();
    let h = b
        .score("minimum", &p, |r| {
            assert_ne!(fail.get(), 2);
            SimpleScore::new(r.as_ref().unwrap().value.0)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..3 {
        t.apply(&s, TupleChange::put(id, (id, 1, Payload(id as i64))))
            .unwrap();
    }
    t.commit().unwrap();
    let saved = g.constraint_matches(&h).unwrap().clone();
    for mode in 1..=2 {
        fail.set(mode);
        let mut t = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || t.apply(&s, TupleChange::put(0, (0, 0, Payload(99))))
            ))
            .is_err()
        );
        drop(t);
        fail.set(0);
        assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        g.assert_consistent().unwrap();
    }
    let before = calls.get();
    fail.set(1);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(g.score(), SimpleScore::new(1));
}
#[test]
fn construction_and_saved_candidates_match_independent_native_tie_policy() {
    // Rank is absolute value; payload remains signed and changes the score.
    let objective = |values: &[Option<&i64>]| {
        let best = values
            .iter()
            .enumerate()
            .filter_map(|(id, v)| v.map(|v| (v.abs(), id, *v)))
            .min();
        SimpleScore::new(best.map_or(0, |(_, _, v)| v))
    };
    let problem = FiniteProblem::new(vec![Variable::new(vec![-2i64, -1, 1, 2]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("selected", (0..4).collect(), objective)
        .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let p = b
            .collect_conditionally::<_, _, _, MinByCollector<usize, i64, Payload>>(
                s.port(),
                |r| r.1.is_some(),
                |r| (r.0, r.1.unwrap().abs(), Payload(r.1.unwrap())),
            )
            .unwrap();
        b.score("selected", &p, |r| {
            SimpleScore::new(r.as_ref().map_or(0, |r| r.value.0))
        })
        .unwrap();
        (b.build(), s)
    };
    let initial = Assignment::new(vec![Some(-2); 4]);
    for cap in 1..=32 {
        let cfg = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&problem, &n, initial.clone(), cfg).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&problem, g, s, initial.clone(), cfg).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
    }
    let (g, s) = factory();
    let cfg = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let actual = greedy_stream_construction(&problem, g, s, cfg).unwrap();
    let expected = greedy_construction(&problem, &n, cfg).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
}
#[test]
fn payload_clone_panics_cannot_install_partial_indexes() {
    #[derive(Debug)]
    struct Bomb {
        value: i64,
        budget: std::rc::Rc<Cell<usize>>,
    }
    impl Clone for Bomb {
        fn clone(&self) -> Self {
            let n = self.budget.get();
            assert_ne!(n, 0, "injected clone panic");
            self.budget.set(n - 1);
            Self {
                value: self.value,
                budget: self.budget.clone(),
            }
        }
    }
    impl PartialEq for Bomb {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }
    let budget = std::rc::Rc::new(Cell::new(usize::MAX));
    let row = |id, key, value| {
        (
            id,
            key,
            Bomb {
                value,
                budget: budget.clone(),
            },
        )
    };
    let mut original = MinByCollector::<usize, i64, Bomb>::default();
    for id in 0..8 {
        original.insert(&row(id, id as i64 % 3, id as i64));
    }
    let mut panics = 0;
    let mut successes = 0;
    for limit in 0..32 {
        for replace in [false, true] {
            budget.set(usize::MAX);
            let mut c = original.clone();
            budget.set(limit);
            let result = catch_unwind(AssertUnwindSafe(|| {
                if replace {
                    assert!(c.replace(&row(0, 99, 99), &row(9, -1, 99)));
                } else {
                    c.insert(&row(9, -1, 99));
                }
            }));
            budget.set(usize::MAX);
            if result.is_err() {
                panics += 1;
                assert_eq!(c, original);
            } else {
                successes += 1;
                assert_eq!(c.finish().unwrap().id, 9);
            }
            assert_eq!(original.finish().unwrap().id, 0);
        }
    }
    assert!(panics > 0 && successes > 0);
}
