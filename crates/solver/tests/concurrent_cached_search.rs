//! Original solver/cache regressions using attributed concurrent test fixtures.
#[path = "../examples/concurrent_support/mod.rs"]
mod support;
use std::{cell::Cell, rc::Rc};
use support::Schedule;
use timetuck::*;
#[test]
fn cached_schedule_trials_restore_warm_supplier_state() {
    let schedule = Schedule::new(&[vec![0, 2]], 4);
    let work = Rc::new(Cell::new(0));
    let observe = work.clone();
    let (g, s, c) = schedule.graph_observed(2, move |n| observe.set(observe.get() + n));
    let p = ListProblem::new(
        (0..4).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert_eq!(
        d.graph().constraint_matches(&c).unwrap()[&()].0,
        schedule.project(d.assignment().lists()).0
    );
    assert_eq!(d.score(), schedule.score(d.assignment().lists()));
    let before = d.graph().constraint_matches(&c).unwrap().clone();
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 1,
            index: 0,
        },
        destination: ListPosition {
            entity: 0,
            index: 2,
        },
    };
    work.set(0);
    {
        let t = d.apply(&movement).unwrap();
        assert_eq!(t.score(), HardSoftScore::new(-3, -30));
        assert_eq!(
            t.graph().constraint_matches(&c).unwrap()[&()].0,
            schedule.project(t.assignment().lists()).0
        );
    }
    let calls = work.get();
    assert!(calls > 0);
    assert_eq!(d.graph().constraint_matches(&c).unwrap(), &before);
    // The score director publishes each touched entity even when a composite
    // returns to the original assignment; both projections must stay warm.
    let inverse = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 2,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    d.apply(&ListMove::Composite(vec![movement, inverse]))
        .unwrap()
        .commit();
    assert_eq!(
        work.get(),
        calls,
        "undo restored the cached schedule without suppliers"
    );
    assert_eq!(d.graph().constraint_matches(&c).unwrap(), &before);
    d.assert_consistent().unwrap();
}
#[test]
fn cached_construction_and_saved_search_match_independent_group_scoring() {
    let schedule = Schedule::new(&[vec![0, 2]], 4);
    let p = ListProblem::new((0..4).collect(), vec![ListEntity::new(vec![]); 2]).unwrap();
    let mut reference = ListConstraintNetwork::new(2, 4);
    let fixture = schedule.clone();
    reference
        .add_constraint(
            "independent",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            move |facts| {
                let lists: Vec<_> = facts
                    .iter()
                    .map(|f| match f {
                        ListFact::Entity(v) => v.to_vec(),
                        _ => unreachable!(),
                    })
                    .collect();
                fixture.reference_score(&lists)
            },
        )
        .unwrap();
    let config = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let expected = greedy_list_construction(&p, &reference, config).unwrap();
    let (g, s, _) = schedule.graph(2);
    let actual = greedy_list_stream_construction(&p, g, s, config).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
    for acceptance in [
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
    ] {
        for max_evaluations in [1, 2, 7, 40, 200] {
            let config = LocalSearchConfig {
                max_evaluations,
                max_steps: 8,
                acceptance,
                assert_scores: true,
                ..Default::default()
            };
            let initial = ListAssignment::new(vec![vec![0, 1, 2], vec![3]]);
            let mut expected =
                list_local_search_from(&p, &reference, initial.clone(), config).unwrap();
            let (g, s, _) = schedule.graph(2);
            let mut actual = list_stream_local_search_from(&p, g, s, initial, config).unwrap();
            actual.constraint_evaluations = 0;
            expected.constraint_evaluations = 0;
            assert_eq!(actual, expected);
            if let Some(best) = actual.best {
                assert_eq!(
                    best.score,
                    schedule.reference_score(best.assignment.lists())
                );
            }
        }
    }
}
