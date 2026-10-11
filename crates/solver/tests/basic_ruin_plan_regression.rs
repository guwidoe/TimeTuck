//! Original checked reconstruction and provider integration cases; no source credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;

#[test]
fn preparation_validates_selection_and_preserves_current_values_and_metadata() {
    let schema = PillarSchema::new(vec![vec![0, 1], vec![2, 3]]).unwrap();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 0])
            .with_initial(0)
            .with_range_diagnostics("A", "x", |v| format!("value {v}")),
        Variable::new(vec![2, 3]).with_initial(2),
        Variable::new(vec![4]).allowing_unassigned(),
        Variable::new(vec![5]).with_initial(5).with_pinned(true),
    ])
    .unwrap()
    .with_pillar_schema(schema.clone())
    .unwrap();
    let before = Assignment::new(vec![Some(1), Some(3), None, Some(5)]);
    for (selected, error) in [
        (vec![], BasicRuinPlanError::EmptySelection),
        (
            vec![4],
            BasicRuinPlanError::UnknownVariable {
                variable: 4,
                count: 4,
            },
        ),
        (
            vec![0, 0],
            BasicRuinPlanError::DuplicateVariable { variable: 0 },
        ),
        (vec![3], BasicRuinPlanError::PinnedVariable { variable: 3 }),
    ] {
        assert_eq!(
            BasicRuinPlan::new(&p, &before, &selected).unwrap_err(),
            error
        );
    }
    let plan = BasicRuinPlan::new(&p, &before, &[0]).unwrap();
    let repair = plan.repair_problem();
    assert_eq!(repair.pillar_schema(), Some(&schema));
    assert_eq!(repair.variables()[0].domain(), &[1, 0]);
    assert_eq!(
        repair.initial_assignment().values(),
        &[None, Some(3), None, Some(5)]
    );
    assert!(!repair.variables()[0].is_pinned());
    assert!(repair.variables()[1..].iter().all(Variable::is_pinned));
    assert!(repair.variables()[2].allows_unassigned());
    let invalid = Assignment::new(vec![Some(9), Some(3), None, Some(5)]);
    let error = plan.finish(&invalid).unwrap_err();
    let BasicRuinPlanError::Model(error) = error else {
        panic!("wrong error")
    };
    assert!(error.range_report().is_some());
    assert_eq!(
        error.without_range_report(),
        &ModelError::ValueOutsideDomain { variable: 0 }
    );
    for changed in [
        Assignment::new(vec![Some(0), Some(2), None, Some(5)]),
        Assignment::new(vec![Some(0), Some(3), Some(4), Some(5)]),
    ] {
        assert!(matches!(
            plan.finish(&changed),
            Err(BasicRuinPlanError::Model(
                ModelError::PinnedValueChanged { .. }
            ))
        ));
    }
    assert!(plan.finish(&before).is_ok());
    assert_eq!(before.values(), &[Some(1), Some(3), None, Some(5)]);
    assert_eq!(
        p.initial_assignment().values(),
        &[Some(0), Some(2), None, Some(5)]
    );
}

#[test]
fn required_partial_inputs_and_reconstructions_are_rejected_but_repair_can_resume() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![2, 3]).with_initial(2),
    ])
    .unwrap();
    let partial = Assignment::new(vec![Some(0), None]);
    assert_eq!(
        BasicRuinPlan::new(&p, &partial, &[1]).unwrap_err(),
        BasicRuinPlanError::IncompleteInitialAssignment
    );
    let before = p.initial_assignment();
    let plan = BasicRuinPlan::new(&p, &before, &[1, 0]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(2);
    let interrupted = greedy_construction(
        plan.repair_problem(),
        &n,
        ConstructionConfig {
            max_evaluations: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!interrupted.complete);
    assert_eq!(
        plan.finish(&interrupted.assignment).unwrap_err(),
        BasicRuinPlanError::IncompleteReconstruction
    );
    let completed = greedy_construction_from(
        plan.repair_problem(),
        &n,
        interrupted.assignment,
        ConstructionConfig::default(),
    )
    .unwrap();
    assert!(completed.complete);
    assert!(plan.finish(&completed.assignment).is_ok());
    let changed = Assignment::new(vec![Some(1), Some(3)]);
    assert_eq!(
        plan.finish(&changed).unwrap(),
        Move::Composite(vec![
            Move::Change {
                variable: 1,
                value: Some(3)
            },
            Move::Change {
                variable: 0,
                value: Some(1)
            }
        ])
    );
}

#[test]
fn optional_nulls_and_complete_repair_states_match_exhaustive_reference() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1])
            .allowing_unassigned()
            .with_initial(1),
    ])
    .unwrap();
    let before = p.initial_assignment();
    for mask in 1..8 {
        let selected: Vec<_> = (0..3).filter(|i| mask & (1 << i) != 0).collect();
        let plan = BasicRuinPlan::new(&p, &before, &selected).unwrap();
        for left in [None, Some(0), Some(1)] {
            for middle in [None, Some(0), Some(1)] {
                for right in [None, Some(0), Some(1)] {
                    let rebuilt = Assignment::new(vec![left, middle, right]);
                    let expected = left.is_some()
                        && (0..3).all(|i| {
                            selected.contains(&i) || rebuilt.values()[i] == before.values()[i]
                        });
                    let result = plan.finish(&rebuilt);
                    assert_eq!(result.is_ok(), expected);
                    if let Ok(movement) = result {
                        let mut assignment = before.clone();
                        {
                            let trial = p.apply(&mut assignment, &movement).unwrap();
                            assert_eq!(*trial, rebuilt);
                        }
                        assert_eq!(assignment, before);
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct Payload(u8, &'static str);
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Payload {}

#[test]
fn equivalent_and_unselected_payloads_survive_recording_and_failure_rollback() {
    let p = FiniteProblem::new(
        (0..3)
            .map(|i| {
                Variable::new(vec![Payload(0, "domain"), Payload(1, "domain")])
                    .with_initial(Payload(0, if i == 0 { "selected" } else { "original" }))
            })
            .collect(),
    )
    .unwrap();
    let before = p.initial_assignment();
    let rebuilt = Assignment::new(vec![
        Some(Payload(1, "new")),
        Some(Payload(0, "discard")),
        Some(Payload(0, "discard")),
    ]);
    let movement = BasicRuinPlan::new(&p, &before, &[0, 1])
        .unwrap()
        .finish(&rebuilt)
        .unwrap();
    assert_eq!(
        movement,
        Move::Composite(vec![Move::Change {
            variable: 0,
            value: Some(Payload(1, "new"))
        }])
    );
    let mut n = ConstraintNetwork::<Payload, SimpleScore>::new(3);
    n.add_constraint("payload", vec![0, 1, 2], |values| {
        SimpleScore::new(values.iter().flatten().map(|v| v.1.len() as i64).sum())
    })
    .unwrap();
    let mut d = ScoreDirector::new(&p, &n, before.clone()).unwrap();
    let (_, record) = TemporaryMoves::new(&mut d)
        .recorded(
            &movement,
            TemporaryMoveOptions {
                restored_score: RestoredScorePolicy::CheckFresh,
                ..Default::default()
            },
            |a, _| {
                assert_eq!(
                    a.values().iter().flatten().map(|v| v.1).collect::<Vec<_>>(),
                    vec!["new", "original", "original"]
                );
                Ok::<_, ()>(())
            },
        )
        .unwrap();
    d.apply(record.forward(d.assignment()).unwrap())
        .unwrap()
        .commit();
    d.apply(record.undo(d.assignment()).unwrap())
        .unwrap()
        .commit();
    let error =
        TemporaryMoves::new(&mut d).process(&movement, TemporaryMoveOptions::default(), |_, _| {
            Err::<(), _>("failed")
        });
    assert!(matches!(
        error,
        Err(TemporaryMoveError::Postprocessor("failed"))
    ));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            TemporaryMoves::new(&mut d)
                .process(
                    &movement,
                    TemporaryMoveOptions::default(),
                    |_, _| -> Result<(), ()> { panic!("failed") },
                )
                .unwrap();
        }))
        .is_err()
    );
    assert_eq!(
        d.assignment()
            .values()
            .iter()
            .flatten()
            .map(|v| v.1)
            .collect::<Vec<_>>(),
        vec!["selected", "original", "original"]
    );
    d.assert_consistent().unwrap();
}

#[test]
fn reconstructed_moves_drive_scoped_and_stream_custom_provider_search() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 2]).with_initial(0),
        Variable::new(vec![0, 3]).with_initial(0),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<i32, SimpleScore>::new(3);
    n.add_constraint("sum", vec![0, 1], |values| {
        SimpleScore::new(values.iter().flatten().map(|v| i64::from(**v)).sum())
    })
    .unwrap();
    let mut provider = |problem: &FiniteProblem<i32>, before: &Assignment<i32>, _: u64| {
        let plan = BasicRuinPlan::new(problem, before, &[0, 1]).unwrap();
        let rebuilt = greedy_construction(
            plan.repair_problem(),
            &n,
            ConstructionConfig {
                max_evaluations: 100,
                assert_scores: true,
                ..Default::default()
            },
        )
        .unwrap();
        vec![plan.finish(&rebuilt.assignment).unwrap()]
    };
    let config = LocalSearchConfig {
        max_steps: 1,
        max_evaluations: 10,
        assert_scores: true,
        ..Default::default()
    };
    let scoped =
        local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
            .unwrap();
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<i32>)>();
    builder
        .score("sum", source.port(), |(i, value)| {
            SimpleScore::new(if *i < 2 {
                i64::from(value.unwrap_or(0))
            } else {
                0
            })
        })
        .unwrap();
    let stream = stream_local_search_from_with_provider(
        &p,
        builder.build(),
        source,
        p.initial_assignment(),
        config,
        &mut provider,
    )
    .unwrap();
    assert_eq!(scoped.best, stream.best);
    let best = scoped.best.unwrap();
    assert_eq!(best.assignment.values(), &[Some(2), Some(3), Some(9)]);
    assert_eq!(best.score, SimpleScore::new(5));
    assert_eq!(scoped.evaluations, stream.evaluations);
    assert_eq!(
        p.initial_assignment().values(),
        &[Some(0), Some(0), Some(9)]
    );
}
