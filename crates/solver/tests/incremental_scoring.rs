// Original regressions; these are not counted as upstream adapted methods.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::{
    ConstraintNetwork, FiniteProblem, HardSoftScore, Move, ScoreDirector, ScoringError,
    SimpleScore, Variable,
};

#[test]
fn composite_recomputes_each_affected_constraint_once_and_defers_cache_updates() {
    let calls = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 3]).unwrap();
    let mut network = ConstraintNetwork::new(3);
    network
        .add_constraint("shared", vec![0, 1, 0], |v| {
            calls[0].set(calls[0].get() + 1);
            assert_eq!(v[0], v[2]);
            SimpleScore::new(i64::from(*v[0].unwrap() + *v[1].unwrap()))
        })
        .unwrap();
    network
        .add_constraint("untouched", vec![2], |v| {
            calls[1].set(calls[1].get() + 1);
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    network
        .add_constraint("constant", vec![], |v| {
            calls[2].set(calls[2].get() + 1);
            assert!(v.is_empty());
            SimpleScore::new(10)
        })
        .unwrap();
    let mut director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    let movement = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 0,
            value: Some(2),
        },
        Move::Change {
            variable: 1,
            value: Some(1),
        },
    ]);
    {
        let candidate = director.apply(&movement).unwrap();
        assert_eq!(candidate.score(), SimpleScore::new(13));
        assert_eq!(candidate.constraint_evaluations(), 1);
    }
    assert_eq!(director.score(), SimpleScore::new(10));
    assert_eq!(director.assignment().values(), &[Some(0); 3]);
    director.apply(&movement).unwrap().commit();
    assert_eq!(director.score(), SimpleScore::new(13));
    assert_eq!(calls.each_ref().map(Cell::get), [3, 1, 1]);
    assert_eq!(
        director.constraint_scores().collect::<Vec<_>>(),
        vec![
            ("shared", SimpleScore::new(3)),
            ("untouched", SimpleScore::ZERO),
            ("constant", SimpleScore::new(10))
        ]
    );
    director.assert_consistent().unwrap();
}

#[test]
fn committed_and_rejected_random_sequences_match_independent_full_formula() {
    let variables: Vec<_> = (0..6)
        .map(|i| {
            let v = Variable::new(vec![0, 1, 2]).with_initial(0);
            if i == 5 { v.allowing_unassigned() } else { v }
        })
        .collect();
    let model = FiniteProblem::new(variables).unwrap();
    let mut network = ConstraintNetwork::new(6);
    for i in 0..6 {
        network
            .add_constraint(format!("value{i}"), vec![i], |v| {
                HardSoftScore::new(0, v[0].map_or(0, |v| i64::from(*v) * 3))
            })
            .unwrap();
        if i > 0 {
            network
                .add_constraint(format!("edge{i}"), vec![i - 1, i], |v| {
                    HardSoftScore::new(-i64::from(v[0].is_some() && v[0] == v[1]), 0)
                })
                .unwrap();
        }
    }
    let mut director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    let mut seed = 20261001_u64;
    for step in 0..2_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let variable = (seed >> 32) as usize % 6;
        let option = ((seed >> 48) % 4) as i32;
        let movement = if step % 5 == 0 {
            Move::Swap {
                left: variable,
                right: (variable + 1) % 6,
            }
        } else {
            Move::Change {
                variable,
                value: if option == 3 { None } else { Some(option) },
            }
        };
        {
            let candidate = director.apply(&movement).unwrap();
            let values = candidate.assignment().values();
            let hard = -(values
                .windows(2)
                .filter(|pair| pair[0].is_some() && pair[0] == pair[1])
                .count() as i64);
            let soft = values.iter().flatten().map(|&v| i64::from(v) * 3).sum();
            let structural = -(values[..5].iter().filter(|v| v.is_none()).count() as i64);
            assert_eq!(
                candidate.score(),
                HardSoftScore::new(hard, soft).with_structural(structural)
            );
            if step % 3 != 0 {
                candidate.commit();
            }
        }
        assert_eq!(
            director.score(),
            network.calculate(&model, director.assignment()).unwrap()
        );
        director.assert_consistent().unwrap();
    }
}

#[test]
fn failed_scoring_and_panics_leave_assignment_and_cache_unchanged() {
    let mode = Cell::new(0);
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("fallible", vec![0], |v| match mode.get() {
            1 => panic!("callback failure"),
            2 => SimpleScore::new(5).with_structural(1),
            _ => SimpleScore::new(i64::from(*v[0].unwrap())),
        })
        .unwrap();
    let mut director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some(1),
    };
    mode.set(1);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = director.apply(&movement);
        }))
        .is_err()
    );
    mode.set(2);
    assert!(matches!(
        director.apply(&movement),
        Err(ScoringError::StructuralContribution { constraint: 0 })
    ));
    mode.set(0);
    assert_eq!(director.score(), SimpleScore::ZERO);
    assert_eq!(director.assignment().values(), &[Some(0)]);
    director.assert_consistent().unwrap();
}

#[test]
fn invalid_model_move_never_calls_constraint_callbacks() {
    let count = Cell::new(0);
    let model = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("constant", vec![0], |_| {
            count.set(count.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
    let mut director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    for movement in [
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 9,
            value: Some(1),
        },
    ] {
        assert!(director.apply(&movement).is_err());
    }
    assert_eq!(count.get(), 1);
    assert_eq!(director.assignment().values(), &[Some(0)]);
}

#[test]
fn malformed_networks_are_rejected_before_callbacks() {
    let mut network = ConstraintNetwork::<i32, SimpleScore>::new(1);
    assert!(matches!(
        network.add_constraint("bad", vec![1], |_| panic!()),
        Err(ScoringError::InvalidScope { .. })
    ));
    assert_eq!(network.constraint_count(), 0);
    network
        .add_constraint("ok", vec![], |_| SimpleScore::ZERO)
        .unwrap();
    assert!(matches!(
        network.add_constraint("ok", vec![], |_| panic!()),
        Err(ScoringError::DuplicateName(_))
    ));
    let model = FiniteProblem::new(vec![]).unwrap();
    assert!(matches!(
        ScoreDirector::new(&model, &network, model.initial_assignment()),
        Err(ScoringError::VariableCount { .. })
    ));
}

#[test]
fn cache_diagnostic_detects_illegal_fact_mutation_even_when_totals_cancel() {
    let fact = Cell::new(1_i64);
    let model = FiniteProblem::<i32>::new(vec![]).unwrap();
    let mut network = ConstraintNetwork::new(0);
    network
        .add_constraint("positive", vec![], |_| SimpleScore::new(fact.get()))
        .unwrap();
    network
        .add_constraint("negative", vec![], |_| SimpleScore::new(-fact.get()))
        .unwrap();
    let director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    fact.set(2);
    assert_eq!(
        network.calculate(&model, director.assignment()).unwrap(),
        director.score()
    );
    assert!(matches!(
        director.assert_consistent(),
        Err(ScoringError::ScoreCorruption(report)) if report.constraints.len() == 2 && report.constraints[0].constraint == 0
    ));
}

#[test]
fn modular_overflow_and_structural_updates_match_full_calculation() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut network =
        ConstraintNetwork::new(1).with_arithmetic(timetuck::ScoringArithmetic::Wrapping);
    network
        .add_constraint("max", vec![], |_| SimpleScore::new(i64::MAX))
        .unwrap();
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].map_or(0, |&v| i64::from(v)))
        })
        .unwrap();
    let mut director = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    assert_eq!(
        director.score(),
        SimpleScore::new(i64::MAX).with_structural(-1)
    );
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(director.score(), SimpleScore::new(i64::MIN));
    director.assert_consistent().unwrap();
    director
        .apply(&Move::Change {
            variable: 0,
            value: None,
        })
        .unwrap()
        .commit();
    assert_eq!(
        director.score(),
        SimpleScore::new(i64::MAX).with_structural(-1)
    );
}
