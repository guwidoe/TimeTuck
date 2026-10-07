//! Original multi-field entity, absence, rollback and mixed-model checks.
use timetuck::*;
#[test]
fn every_partial_row_and_atomic_rollback_matches_assignment_projection() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 4]).unwrap();
    for include in [false, true] {
        let schema = BasicEntityRows::new(4, vec![vec![2, 0], vec![1]]).unwrap();
        let mut b = StreamGraphBuilder::new();
        let source = b.source();
        let rows = basic_entity_stream(&mut b, &source, &schema, include).unwrap();
        let matches = b
            .score("entities", &rows, |(_, v)| {
                SimpleScore::new(1 + v.iter().map(|v| v.unwrap_or(0) as i64).sum::<i64>())
            })
            .unwrap();
        let mut d =
            StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
        for code in 0..81 {
            let mut digits = code;
            let values = (0..4)
                .map(|_| {
                    let v = digits % 3;
                    digits /= 3;
                    if v == 2 { None } else { Some(v) }
                })
                .collect::<Vec<_>>();
            let before = d.assignment().clone();
            let before_score = d.score();
            let before_matches = d.graph().constraint_matches(&matches).unwrap().clone();
            let movement = Move::Composite(
                values
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(variable, &value)| Move::Change { variable, value })
                    .collect(),
            );
            let trial = d.apply(&movement).unwrap();
            let expected = [vec![values[2], values[0]], vec![values[1]]]
                .into_iter()
                .enumerate()
                .filter(|(_, v)| include || v.iter().all(Option::is_some))
                .collect::<Vec<_>>();
            let actual = trial
                .graph()
                .constraint_matches(&matches)
                .unwrap()
                .iter()
                .map(|(&id, (payload, _))| (id, payload.1.clone()))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert_eq!(
                trial.score(),
                SimpleScore::new(
                    expected
                        .iter()
                        .map(|(_, v)| 1 + v.iter().map(|v| v.unwrap_or(0) as i64).sum::<i64>())
                        .sum()
                )
            );
            if code % 2 == 0 {
                trial.commit();
            } else {
                drop(trial);
                assert_eq!(d.assignment(), &before);
                assert_eq!(d.score(), before_score);
                assert_eq!(
                    d.graph().constraint_matches(&matches).unwrap(),
                    &before_matches
                );
            }
        }
        d.assert_consistent().unwrap();
    }
}
#[test]
fn absent_fields_do_not_create_rows_and_reinsertion_preserves_columns() {
    let schema = BasicEntityRows::new(3, vec![vec![2, 0], vec![1]]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let rows = basic_entity_stream(&mut b, &source, &schema, true).unwrap();
    let matches = b.score("rows", &rows, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    for (changes, expected) in [
        (vec![TupleChange::put(0, (0, Some(10)))], vec![]),
        (
            vec![TupleChange::put(2, (2, None))],
            vec![(0, vec![None, Some(10)])],
        ),
        (
            vec![TupleChange::put(1, (1, Some(20)))],
            vec![(0, vec![None, Some(10)]), (1, vec![Some(20)])],
        ),
        (vec![TupleChange::retract(2)], vec![(1, vec![Some(20)])]),
        (
            vec![TupleChange::put(2, (2, Some(30)))],
            vec![(0, vec![Some(30), Some(10)]), (1, vec![Some(20)])],
        ),
    ] {
        let mut tx = g.transaction();
        for change in changes {
            tx.apply(&source, change).unwrap();
        }
        tx.commit().unwrap();
        let actual = g
            .constraint_matches(&matches)
            .unwrap()
            .values()
            .map(|(payload, _)| payload.clone())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }
}
#[test]
fn schema_errors_are_rejected_and_unselected_updates_do_not_rescore() {
    assert_eq!(
        BasicEntityRows::new(2, vec![vec![]]).unwrap_err(),
        BasicEntityRowsError::EmptyRow { entity: 0 }
    );
    assert_eq!(
        BasicEntityRows::new(2, vec![vec![2]]).unwrap_err(),
        BasicEntityRowsError::UnknownVariable {
            variable: 2,
            variable_count: 2
        }
    );
    assert_eq!(
        BasicEntityRows::new(2, vec![vec![0], vec![0]]).unwrap_err(),
        BasicEntityRowsError::DuplicateVariable { variable: 0 }
    );
    assert_eq!(
        BasicEntityRows::new(2, vec![vec![0, 0]]).unwrap_err(),
        BasicEntityRowsError::DuplicateVariable { variable: 0 }
    );
    assert!(BasicEntityRows::new(0, vec![]).unwrap().is_empty());
    let calls = std::cell::Cell::new(0);
    let schema = BasicEntityRows::new(2, vec![vec![0]]).unwrap();
    assert_eq!(schema.len(), 1);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source();
    let rows = basic_entity_stream(&mut b, &source, &schema, false).unwrap();
    b.score("pinned fact", &rows, |_| {
        calls.set(calls.get() + 1);
        SimpleScore::new(1)
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(1));
    let before = calls.get();
    d.apply(&Move::Change {
        variable: 1,
        value: Some(1),
    })
    .unwrap()
    .commit();
    assert_eq!(calls.get(), before);
}
#[test]
fn mixed_entity_projection_tracks_joint_trials_and_restores_on_scoring_panic() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap(),
        ListProblem::new(
            vec![10],
            vec![ListEntity::new(vec![10]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let schema = BasicEntityRows::new(2, vec![vec![1, 0]]).unwrap();
    let panic_score = std::cell::Cell::new(false);
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    let rows = basic_entity_stream(&mut b, &sources.basic, &schema, false).unwrap();
    b.score("entity", &rows, |(_, v)| {
        assert!(!panic_score.get(), "score panic");
        SimpleScore::new(v.iter().map(|v| v.unwrap() as i64).sum())
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    };
    {
        let t = d.apply(&movement).unwrap();
        assert_eq!(t.score(), SimpleScore::new(1));
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), SimpleScore::ZERO);
    panic_score.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&movement);
        }))
        .is_err()
    );
    panic_score.set(false);
    assert_eq!(d.assignment(), &p.initial_assignment());
    d.assert_consistent().unwrap();
}
