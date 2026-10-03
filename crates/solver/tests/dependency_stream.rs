//! Original transactional and solver integration for cached derived fields.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Row {
    parent: Option<usize>,
    weight: i64,
    label: usize,
}
fn row(parent: Option<usize>, weight: i64) -> Row {
    Row {
        parent,
        weight,
        label: 0,
    }
}
#[test]
fn stream_cycle_recovery_retraction_metadata_and_undo_restore_cached_values() {
    let calls = Rc::new(RefCell::new(vec![]));
    let count = calls.clone();
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let s = b.source::<usize, Row>();
    let derived = b
        .derive_dependencies(
            s.port(),
            0..4,
            |r| r.parent.into_iter().collect(),
            move |k, r, ins| {
                count.borrow_mut().push(*k);
                Some(r.weight + ins.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    let c = b
        .score("values", &derived, |(_, v)| {
            HardSoftScore::new(-i64::from(v.inconsistent), v.value.unwrap_or(0))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (k, r) in [
        (0, row(None, 1)),
        (1, row(Some(0), 2)),
        (2, row(Some(1), 3)),
        (3, row(None, 10)),
    ] {
        t.apply(&s, TupleChange::put(k, r)).unwrap();
    }
    t.commit().unwrap();
    let original = g.constraint_matches(&c).unwrap().clone();
    calls.borrow_mut().clear();
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, row(None, 5))).unwrap();
        assert_eq!(*calls.borrow(), [0, 1, 2]);
        assert_eq!(t.score().unwrap(), HardSoftScore::new(0, 32));
        calls.borrow_mut().clear();
    }
    assert!(calls.borrow().is_empty());
    assert_eq!(g.constraint_matches(&c).unwrap(), &original);
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, row(Some(1), 1))).unwrap();
        assert_eq!(t.score().unwrap(), HardSoftScore::new(-3, 10));
        assert!(calls.borrow().is_empty());
        t.commit().unwrap();
    }
    g.assert_consistent().unwrap();
    calls.borrow_mut().clear();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, row(None, 1))).unwrap();
    t.commit().unwrap();
    assert_eq!(*calls.borrow(), [0, 1, 2]);
    assert_eq!(g.constraint_matches(&c).unwrap(), &original);
    // Changing only metadata must update the source tuple, not dependent rows.
    calls.borrow_mut().clear();
    let mut t = g.transaction();
    let mut r = row(None, 1);
    r.label = 99;
    t.apply(&s, TupleChange::put(0, r)).unwrap();
    t.commit().unwrap();
    assert_eq!(*calls.borrow(), [0]);
    assert_eq!(g.constraint_matches(&c).unwrap()[&0].0.0.label, 99);
    let before = g.constraint_matches(&c).unwrap().clone();
    calls.borrow_mut().clear();
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::retract(0)).unwrap();
        assert_eq!(*calls.borrow(), [1, 2]);
        assert_eq!(t.score().unwrap(), HardSoftScore::new(0, 17));
        assert!(
            !t.graph()
                .unwrap()
                .constraint_matches(&c)
                .unwrap()
                .contains_key(&0)
        );
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    g.assert_consistent().unwrap();
}
#[test]
fn stream_errors_and_unwind_roll_back_plan_rows_values_and_scores() {
    let panic_on = Rc::new(Cell::new(false));
    let flag = panic_on.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Row>();
    let d = b
        .derive_dependencies(
            s.port(),
            0..3,
            |r| r.parent.into_iter().collect(),
            move |k, r, ins| {
                assert!(!flag.get() || *k != 1, "supplier panic");
                Some(r.weight + ins.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    let c = b
        .score("sum", &d, |(row, v)| {
            SimpleScore::new(v.value.unwrap_or(0)).with_structural(i64::from(row.weight == 99))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, row(None, 2))).unwrap();
    t.apply(&s, TupleChange::put(1, row(Some(0), 3))).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&c).unwrap().clone();
    let source = g.source_rows(&s).unwrap().clone();
    for bad in [(1, row(Some(99), 0)), (99, row(None, 1))] {
        let mut t = g.transaction();
        assert!(matches!(
            t.apply(&s, TupleChange::put(bad.0, bad.1)),
            Err(GraphError::Dependency(_))
        ));
        assert_eq!(t.score(), Err(GraphError::FailedTransaction));
        drop(t);
        assert_eq!(g.constraint_matches(&c).unwrap(), &before);
        assert_eq!(g.source_rows(&s).unwrap(), &source);
    }
    {
        let mut t = g.transaction();
        assert!(matches!(
            t.apply(&s, TupleChange::put(1, row(Some(2), 99))),
            Err(GraphError::StructuralContribution(_))
        ));
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    assert_eq!(g.source_rows(&s).unwrap(), &source);
    panic_on.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&s, TupleChange::put(0, row(None, 20))).unwrap();
        }))
        .is_err()
    );
    panic_on.set(false);
    assert_eq!(g.constraint_matches(&c).unwrap(), &before);
    assert_eq!(g.source_rows(&s).unwrap(), &source);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, row(None, 4))).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(11));
    g.assert_consistent().unwrap();
}
fn reference(values: &[Option<usize>]) -> Vec<DerivedValue<i64>> {
    fn visit(i: usize, values: &[Option<usize>], active: &mut Vec<usize>) -> DerivedValue<i64> {
        if active.contains(&i) {
            return DerivedValue {
                value: None,
                inconsistent: true,
            };
        }
        let Some(parent) = values[i] else {
            return DerivedValue {
                value: None,
                inconsistent: false,
            };
        };
        if parent == values.len() {
            return DerivedValue {
                value: Some(i as i64 + 1),
                inconsistent: false,
            };
        }
        active.push(i);
        let prev = visit(parent, values, active);
        active.pop();
        DerivedValue {
            value: prev.value.map(|v| v + i as i64 + 1),
            inconsistent: prev.inconsistent,
        }
    }
    (0..values.len())
        .map(|i| visit(i, values, &mut vec![]))
        .collect()
}
fn full(values: &[Option<usize>]) -> HardSoftScore {
    let rows = reference(values);
    HardSoftScore::new(
        -(rows.iter().filter(|v| v.inconsistent).count() as i64),
        -rows.iter().filter_map(|v| v.value).sum::<i64>(),
    )
}
type Output = StreamConstraint<usize, ((usize, Option<usize>), DerivedValue<i64>)>;
fn graph() -> (
    StreamGraph<'static, HardSoftScore>,
    PlanningStreamSource<usize>,
    Output,
) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    let d = b
        .derive_dependencies(
            s.port(),
            0..4,
            |r| r.1.filter(|&p| p < 4).into_iter().collect(),
            |k, r, ins| {
                r.1.and_then(|p| {
                    if p == 4 {
                        Some(*k as i64 + 1)
                    } else {
                        ins[0].1.value.map(|v| v + *k as i64 + 1)
                    }
                })
            },
        )
        .unwrap();
    let c = b
        .score("dependency", &d, |(_, v)| {
            HardSoftScore::new(-i64::from(v.inconsistent), -v.value.unwrap_or(0))
        })
        .unwrap();
    (b.build(), s, c)
}
#[test]
fn every_partial_parent_assignment_and_composite_undo_matches_independent_scoring() {
    let p = FiniteProblem::new(vec![Variable::new((0..5).collect()); 4]).unwrap();
    let (g, s, c) = graph();
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for code in 0..1296 {
        let mut digits = code;
        let values: Vec<_> = (0..4)
            .map(|_| {
                let digit = digits % 6;
                digits /= 6;
                if digit == 5 { None } else { Some(digit) }
            })
            .collect();
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let before = d.graph().constraint_matches(&c).unwrap().clone();
        let t = d.apply(&movement).unwrap();
        assert_eq!(
            t.score(),
            full(&values).with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
        );
        let expected = reference(&values);
        for (k, v) in expected.iter().enumerate() {
            assert_eq!(&t.graph().constraint_matches(&c).unwrap()[&k].0.1, v);
        }
        if code % 3 == 0 {
            t.commit();
            d.assert_consistent().unwrap();
        } else {
            drop(t);
            assert_eq!(d.graph().constraint_matches(&c).unwrap(), &before);
        }
    }
}
#[test]
fn construction_and_saved_search_candidates_match_independent_network() {
    let p = FiniteProblem::new(vec![Variable::new((0..5).collect()); 4]).unwrap();
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("reference", vec![0, 1, 2, 3], |values| {
            full(&values.iter().map(|v| v.copied()).collect::<Vec<_>>())
        })
        .unwrap();
    let cfg = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let expected = greedy_construction(&p, &network, cfg).unwrap();
    let (g, s, _) = graph();
    let actual = greedy_stream_construction(&p, g, s, cfg).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    for limit in [1, 2, 7, 40, 200] {
        let cfg = LocalSearchConfig {
            max_evaluations: limit,
            assert_scores: true,
            ..Default::default()
        };
        let initial = Assignment::new(vec![Some(1), Some(0), Some(1), Some(2)]);
        let mut expected = local_search_from(&p, &network, initial.clone(), cfg).unwrap();
        let (g, s, _) = graph();
        let mut actual = stream_local_search_from(&p, g, s, initial, cfg).unwrap();
        expected.constraint_evaluations = 0;
        actual.constraint_evaluations = 0;
        assert_eq!(actual, expected);
    }
}
#[test]
fn dependency_builder_rejects_foreign_ports_and_duplicate_universe() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let x = a.source::<usize, i64>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let y = b.source::<usize, i64>();
    assert!(matches!(
        a.derive_dependencies(y.port(), 0..2, |_| vec![], |_, v, _| Some(*v)),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        a.derive_dependencies(x.port(), [0, 0], |_| vec![], |_, v, _| Some(*v)),
        Err(GraphError::Dependency(_))
    ));
    a.derive_dependencies(x.port(), 0..2, |_| vec![], |_, v, _| Some(*v))
        .unwrap();
}
