//! Original calendar arithmetic, aggregation, transaction and solver regressions.
use std::{
    cell::Cell,
    collections::{BTreeMap, HashSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn p(v: [i32; 3]) -> CalendarPeriod {
    CalendarPeriod::new(v[0], v[1], v[2])
}
fn reference(rows: impl Iterator<Item = CalendarPeriod>) -> CalendarPeriod {
    let mut out = [0i64; 3];
    for row in rows {
        for (i, v) in row.components().into_iter().enumerate() {
            out[i] += i64::from(v);
        }
    }
    p(out.map(|v| i32::try_from(v).unwrap()))
}
fn objective(total: CalendarPeriod) -> HardMediumSoftScore {
    let [a, b, c] = total.components().map(i64::from);
    HardMediumSoftScore::new(-a * a, -b * b, -c * c)
}
#[test]
fn components_remain_distinct_and_calendar_format_preserves_signs() {
    let values = [
        CalendarPeriod::ZERO,
        CalendarPeriod::from_years(1),
        CalendarPeriod::from_months(12),
        CalendarPeriod::new(1, -12, 0),
        CalendarPeriod::new(-2, 15, -30),
        CalendarPeriod::new(i32::MIN, i32::MAX, -1),
    ];
    assert_eq!(HashSet::from(values).len(), 6);
    for (v, s) in values.into_iter().zip([
        "P0D",
        "P1Y",
        "P12M",
        "P1Y-12M",
        "P-2Y15M-30D",
        "P-2147483648Y2147483647M-1D",
    ]) {
        assert_eq!(v.to_string(), s);
    }
    assert_eq!(
        CalendarPeriod::from_months(12)
            .checked_add(CalendarPeriod::from_months(1))
            .unwrap(),
        CalendarPeriod::from_months(13)
    );
    assert_eq!(
        CalendarPeriod::new(1, 12, -3)
            .checked_sub(CalendarPeriod::new(-2, 1, 7))
            .unwrap(),
        CalendarPeriod::new(3, 11, -10)
    );
}
#[test]
fn component_arithmetic_matches_widened_integer_reference_at_all_boundaries() {
    let edge = [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX];
    for axis in 0..3 {
        for a in edge {
            for b in edge {
                let mut av = [7, -9, 11];
                let mut bv = [2, -3, -4];
                av[axis] = a;
                bv[axis] = b;
                for subtract in [false, true] {
                    let expected = (0..3)
                        .map(|i| {
                            i32::try_from(
                                i64::from(av[i])
                                    + if subtract {
                                        -i64::from(bv[i])
                                    } else {
                                        i64::from(bv[i])
                                    },
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                        .map(|v| p(v.try_into().unwrap()))
                        .map_err(|_| PeriodOverflow);
                    assert_eq!(
                        if subtract {
                            p(av).checked_sub(p(bv))
                        } else {
                            p(av).checked_add(p(bv))
                        },
                        expected
                    );
                }
            }
        }
    }
    assert_eq!(
        p([i32::MIN; 3]).checked_sub(p([i32::MIN; 3])),
        Ok(CalendarPeriod::ZERO)
    );
}
#[test]
fn duplicate_zero_and_failed_updates_preserve_captures_and_snapshots() {
    for axis in 0..3 {
        let row = |v| {
            let mut x = [0; 3];
            x[axis] = v;
            p(x)
        };
        let mut c = PeriodSumCollector::default();
        for v in [-1, i32::MAX, 1] {
            c.try_insert(row(v)).unwrap();
        }
        let saved = c.clone();
        let observed = c.finish();
        assert_eq!(c.try_insert(row(1)), Err(PeriodOverflow));
        assert_eq!(c, saved);
        assert_eq!(c.try_remove(row(-1)), Err(PeriodOverflow));
        assert_eq!(c, saved);
        for new in [-1, -2] {
            assert_eq!(c.try_replace(row(-1), row(new)), Err(PeriodOverflow));
            assert_eq!(c, saved);
        }
        // Subtraction succeeds; insertion fails after the staged capture removal.
        assert_eq!(c.try_replace(row(1), row(2)), Err(PeriodOverflow));
        assert_eq!(c, saved);
        assert!(!c.try_replace(row(5), row(1)).unwrap());
        assert!(!c.try_remove(row(5)).unwrap());
        assert_eq!(c.try_replace(row(i32::MAX), row(i32::MIN)), Ok(true));
        assert_eq!(c.finish(), row(i32::MIN));
        assert_eq!(observed, row(i32::MAX));
        assert_eq!(saved.finish(), observed);
        assert_eq!(c.try_replace(row(i32::MIN), row(i32::MAX)), Ok(true));
        assert_eq!(c, saved);
    }
    let mut c = PeriodSumCollector::default();
    let a = CalendarPeriod::new(1, 12, -3);
    for v in [a, a, CalendarPeriod::ZERO] {
        c.try_insert(v).unwrap();
    }
    assert_eq!(c.finish(), CalendarPeriod::new(2, 24, -6));
    for v in [a, a, CalendarPeriod::ZERO] {
        assert!(c.try_remove(v).unwrap());
    }
    assert!(!c.try_remove(CalendarPeriod::ZERO).unwrap());
    assert_eq!(c.finish(), CalendarPeriod::ZERO);
}
#[test]
fn grouped_signed_components_commit_undo_and_rebuild_match_full_sums() {
    let mut b = StreamGraphBuilder::<HardMediumSoftScore>::new();
    let s = b.source::<usize, (u8, CalendarPeriod)>();
    let port = b
        .group_collect_with::<_, _, _, _, PeriodSumCollector>(s.port(), |r| *r)
        .unwrap();
    let h = b.score("calendar", &port, |(_, v)| objective(*v)).unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261224u64;
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
                    p(std::array::from_fn(|_| (draw() % 51) as i32 - 25)),
                )
            });
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, Vec<CalendarPeriod>>::new();
            for &(k, v) in next.values() {
                groups.entry(k).or_default().push(v);
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut expected = HardMediumSoftScore::ZERO;
            for (k, values) in groups {
                let sum = reference(values.into_iter());
                assert_eq!(actual[&k].0.1, sum);
                expected = expected + objective(sum);
            }
            assert_eq!(t.score().unwrap(), expected);
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
fn overflow_mapping_and_downstream_failures_restore_graph() {
    let fail = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, CalendarPeriod>();
    let port = b
        .collect::<_, _, _, PeriodSumCollector>(s.port(), |p| {
            assert_ne!(fail.get(), 1);
            calls.set(calls.get() + 1);
            *p
        })
        .unwrap();
    let h = b
        .score("period", &port, |p| {
            assert_ne!(fail.get(), 2);
            SimpleScore::new(i64::from(p.days())).with_structural(if fail.get() == 3 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, v) in [-1, i32::MAX, 1].into_iter().enumerate() {
        t.apply(&s, TupleChange::put(id, CalendarPeriod::from_days(v)))
            .unwrap();
    }
    t.commit().unwrap();
    let saved = g.constraint_matches(&h).unwrap().clone();
    for mode in 1..=4 {
        fail.set(mode);
        let mut t = g.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            if mode == 4 {
                t.apply(&s, TupleChange::put(0, CalendarPeriod::from_days(-1)))
            } else {
                t.apply(&s, TupleChange::put(1, CalendarPeriod::from_days(3)))
            }
        }));
        if mode == 3 {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        assert!(t.score().is_err());
        drop(t);
        fail.set(0);
        assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        g.assert_consistent().unwrap();
    }
    let old = calls.get();
    fail.set(1);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(1)).unwrap();
    t.commit().unwrap();
    assert_eq!(calls.get(), old);
    assert_eq!(g.score(), SimpleScore::ZERO);
}
#[test]
fn construction_and_bounded_search_preserve_all_calendar_score_levels() {
    // Explicit model policy: minimize squared totals separately for each calendar
    // field. This is not a conversion of years/months/days into elapsed time.
    let options = [
        CalendarPeriod::new(1, 0, -2),
        CalendarPeriod::new(-1, 1, 3),
        CalendarPeriod::new(0, -2, 1),
    ];
    let problem = FiniteProblem::new(vec![Variable::new(options.to_vec()); 4]).unwrap();
    let mut n = ConstraintNetwork::<CalendarPeriod, HardMediumSoftScore>::new(4);
    n.add_constraint("calendar", (0..4).collect(), |v| {
        objective(reference(v.iter().filter_map(|v| v.copied())))
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<CalendarPeriod>)>();
        let c = b
            .collect_conditionally::<_, _, _, PeriodSumCollector>(
                s.port(),
                |r| r.1.is_some(),
                |r| r.1.unwrap(),
            )
            .unwrap();
        b.score("calendar", &c, |v| objective(*v)).unwrap();
        (b.build(), s)
    };
    let initial = Assignment::new(vec![Some(options[0]); 4]);
    for cap in 1..=32 {
        let config = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&problem, &n, initial.clone(), config).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&problem, g, s, initial.clone(), config).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
    }
    let (g, s) = factory();
    let actual = greedy_stream_construction(
        &problem,
        g,
        s,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    let expected = greedy_construction(
        &problem,
        &n,
        ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
}
