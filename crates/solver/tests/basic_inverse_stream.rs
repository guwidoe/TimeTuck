//! Original inverse relationship integration and rollback regressions.
use std::{
    cell::Cell,
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Matches = StreamConstraint<usize, (i32, SnapshotMap<usize, ()>)>;
fn entity(variable: usize) -> Option<usize> {
    match variable {
        0 | 1 => Some(0),
        2 => Some(1),
        _ => None,
    }
}
fn expected(values: &[Option<i32>], value: i32) -> BTreeSet<usize> {
    values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| if *v == Some(value) { entity(i) } else { None })
        .collect()
}
fn score(values: &[Option<i32>]) -> SimpleScore {
    SimpleScore::new(
        -(0..3)
            .map(|v| {
                let delta = expected(values, v).len() as i64 - 1;
                delta * delta
            })
            .sum::<i64>(),
    )
}
fn graph() -> (
    StreamGraph<'static, SimpleScore>,
    PlanningStreamSource<i32>,
    Matches,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let basics = b.source();
    let values = b.source::<usize, i32>();
    let inverse = basic_inverse_stream(&mut b, &basics, values.port(), entity).unwrap();
    let m = b
        .score("one entity per value", &inverse, |(_, entities)| {
            let delta = entities.len() as i64 - 1;
            SimpleScore::new(-delta * delta)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for i in 0..3 {
        t.apply(&values, TupleChange::put(i, i as i32)).unwrap();
    }
    t.commit().unwrap();
    (g, basics, m)
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 4]).unwrap()
}
#[test]
fn every_partial_assignment_preserves_set_membership_and_reversible_counts() {
    let p = problem();
    let (g, s, m) = graph();
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    for code in 0..81 {
        let mut digits = code;
        let values: Vec<_> = (0..4)
            .map(|_| {
                let v = digits % 3;
                digits /= 3;
                if v == 2 { None } else { Some(v) }
            })
            .collect();
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let before = d.graph().constraint_matches(&m).unwrap().clone();
        let before_assignment = d.assignment().clone();
        let t = d.apply(&movement).unwrap();
        assert_eq!(t.score(), score(&values));
        let rows = t.graph().constraint_matches(&m).unwrap();
        assert_eq!(rows.len(), 3);
        for v in 0..3 {
            assert_eq!(rows[&v].0.0, v as i32);
            assert_eq!(
                rows[&v].0.1.keys().copied().collect::<BTreeSet<_>>(),
                expected(&values, v as i32)
            );
        }
        if code % 2 == 0 {
            t.commit();
        } else {
            drop(t);
            assert_eq!(d.assignment(), &before_assignment);
            assert_eq!(d.graph().constraint_matches(&m).unwrap(), &before);
        }
    }
    d.assert_consistent().unwrap();
}
#[test]
fn excluded_variables_and_undo_do_not_rescore_memberships_and_panics_restore_indexes() {
    let p = problem();
    let panic_score = Cell::new(false);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let basics = b.source();
    let values = b.source::<usize, i32>();
    let inverse = basic_inverse_stream(&mut b, &basics, values.port(), entity).unwrap();
    let m = b
        .score("members", &inverse, |(_, members)| {
            calls.set(calls.get() + 1);
            assert!(!panic_score.get(), "score panic");
            SimpleScore::new(members.len() as i64)
        })
        .unwrap();
    let mut g = b.build();
    let mut facts = g.transaction();
    facts.apply(&values, TupleChange::put(0, 0)).unwrap();
    facts.apply(&values, TupleChange::put(1, 1)).unwrap();
    facts.commit().unwrap();
    let initial = Assignment::new(vec![Some(0), Some(0), Some(1), None]);
    let mut d = StreamScoreDirector::new(&p, g, basics, initial.clone()).unwrap();
    calls.set(0);
    d.apply(&Move::Change {
        variable: 3,
        value: Some(1),
    })
    .unwrap()
    .commit();
    assert_eq!(
        calls.get(),
        0,
        "a different basic field does not affect this relationship"
    );
    let original = d.graph().constraint_matches(&m).unwrap().clone();
    let movement = Move::Change {
        variable: 0,
        value: Some(1),
    };
    panic_score.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&movement);
        }))
        .is_err()
    );
    panic_score.set(false);
    assert_eq!(d.graph().constraint_matches(&m).unwrap(), &original);
    {
        let mut outer = d.edit_session().unwrap();
        outer.apply(&movement).unwrap();
        assert_eq!(
            outer.director().graph().constraint_matches(&m).unwrap()[&0]
                .0
                .1
                .len(),
            1,
            "second reference retains entity"
        );
        {
            let mut inner = outer.nested().unwrap();
            inner
                .apply(&Move::Change {
                    variable: 1,
                    value: Some(1),
                })
                .unwrap();
            assert!(
                inner.director().graph().constraint_matches(&m).unwrap()[&0]
                    .0
                    .1
                    .is_empty()
            );
            inner.commit();
        }
        calls.set(0);
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(d.graph().constraint_matches(&m).unwrap(), &original);
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(3));
    d.assert_consistent().unwrap();
}
#[test]
fn universe_changes_preserve_membership_and_stable_output_identity() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let basics = b.source();
    let values = b.source::<usize, i32>();
    let inverse = basic_inverse_stream(&mut b, &basics, values.port(), Some).unwrap();
    let m = b
        .score("members", &inverse, |(_, members)| {
            SimpleScore::new(members.len() as i64)
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&basics, TupleChange::put(0, (0, Some(7)))).unwrap();
    t.apply(&values, TupleChange::put(100, 7)).unwrap();
    t.apply(&values, TupleChange::put(200, 8)).unwrap();
    t.commit().unwrap();
    let original = g.constraint_matches(&m).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&values, TupleChange::retract(100)).unwrap();
        t.apply(&values, TupleChange::put(300, 7)).unwrap();
        t.apply(&values, TupleChange::put(400, 7)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(2));
        let rows = t.graph().unwrap().constraint_matches(&m).unwrap();
        assert!(!rows.contains_key(&100));
        assert!(rows[&200].0.1.is_empty());
        assert_eq!(rows[&300].0.1, rows[&400].0.1);
    }
    assert_eq!(g.constraint_matches(&m).unwrap(), &original);
    let mut t = g.transaction();
    t.apply(&basics, TupleChange::retract(0)).unwrap();
    t.commit().unwrap();
    assert!(g.constraint_matches(&m).unwrap()[&100].0.1.is_empty());
    g.assert_consistent().unwrap();
}
#[test]
fn search_selects_and_saves_candidates_using_inverse_collections() {
    let p = problem();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("reference", vec![0, 1, 2, 3], |values| {
        score(&values.iter().map(|v| v.copied()).collect::<Vec<_>>())
    })
    .unwrap();
    let initial = Assignment::new(vec![Some(0); 4]);
    for limit in [1, 8, 60] {
        let config = LocalSearchConfig {
            max_steps: 15,
            max_evaluations: limit,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&p, &n, initial.clone(), config.clone())
            .unwrap()
            .best
            .unwrap();
        let (g, s, _) = graph();
        let actual = stream_local_search_from(&p, g, s, initial.clone(), config)
            .unwrap()
            .best
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.score, score(actual.assignment.values()));
    }
}
