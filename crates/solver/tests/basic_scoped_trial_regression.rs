//! Original basic transaction/selection contracts; no upstream method credit.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn single_and_repeated_writes_keep_registration_order_arguments_and_exact_rollback() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]).with_initial(0); 2]).unwrap();
    let calls = RefCell::new(Vec::new());
    let mut network = ConstraintNetwork::new(2);
    for (id, scope) in [vec![0, 0], vec![1], vec![1, 0, 1], vec![], vec![0]]
        .into_iter()
        .enumerate()
    {
        let calls = &calls;
        network
            .add_constraint(format!("constraint-{id}"), scope, move |facts| {
                let values: Vec<_> = facts.iter().map(|v| v.copied()).collect();
                calls.borrow_mut().push((id, values.clone()));
                SimpleScore::new(values.iter().flatten().sum())
            })
            .unwrap();
    }
    network
        .add_weighted_constraint("other-variable", vec![1], SimpleScore::ZERO, |_| {
            panic!("disabled callback")
        })
        .unwrap();
    let mut director = ScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    calls.borrow_mut().clear();
    let initial = director.assignment().clone();
    let initial_cache = director
        .constraint_scores()
        .map(|(name, score)| (name.to_owned(), score))
        .collect::<Vec<_>>();
    let change = Move::Change {
        variable: 0,
        value: Some(2),
    };
    let repeated = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        change.clone(),
    ]);
    let expected = vec![
        (0, vec![Some(2), Some(2)]),
        (2, vec![Some(0), Some(2), Some(0)]),
        (4, vec![Some(2)]),
    ];
    for movement in [&change, &repeated] {
        {
            let trial = director.apply(movement).unwrap();
            assert_eq!(trial.constraint_evaluations(), 3);
            assert_eq!(trial.score(), SimpleScore::new(8));
            assert_eq!(calls.take(), expected);
        }
        assert_eq!(director.assignment(), &initial);
        assert_eq!(director.score(), SimpleScore::ZERO);
        assert_eq!(
            director
                .constraint_scores()
                .map(|(name, score)| (name.to_owned(), score))
                .collect::<Vec<_>>(),
            initial_cache
        );
        assert!(calls.borrow().is_empty(), "rollback does not rescore");
    }
    // A recorded same-value write still runs its ordinary affected callbacks.
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(0),
        })
        .unwrap()
        .commit();
    assert_eq!(
        calls.take(),
        vec![
            (0, vec![Some(0), Some(0)]),
            (2, vec![Some(0), Some(0), Some(0)]),
            (4, vec![Some(0)])
        ]
    );
    director.apply(&change).unwrap().commit();
    assert_eq!(calls.take(), expected);
    assert_eq!(director.assignment().values(), &[Some(2), Some(0)]);
    assert_eq!(director.score(), SimpleScore::new(8));
    director.assert_consistent().unwrap();
}

#[test]
fn single_direct_write_callback_failure_after_an_earlier_contribution_restores_caches() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let calls = RefCell::new(Vec::new());
    let mode = std::cell::Cell::new(0);
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("earlier", vec![0], |v| {
            calls.borrow_mut().push(0);
            SimpleScore::new(v[0].copied().unwrap())
        })
        .unwrap();
    network
        .add_constraint("later", vec![0, 0], |v| {
            calls.borrow_mut().push(1);
            assert_ne!(mode.get(), 2, "intentional callback panic");
            SimpleScore::new(v[0].copied().unwrap()).with_structural(if mode.get() == 1 {
                -1
            } else {
                0
            })
        })
        .unwrap();
    let mut director = ScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    let movement = Move::Change {
        variable: 0,
        value: Some(1),
    };
    for injected in [1, 2] {
        calls.borrow_mut().clear();
        mode.set(injected);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            director.apply(&movement).map(|trial| trial.score())
        }));
        if injected == 1 {
            assert!(matches!(
                outcome.unwrap(),
                Err(ScoringError::StructuralContribution { constraint: 1 })
            ));
        } else {
            assert!(outcome.is_err());
        }
        assert_eq!(calls.take(), vec![0, 1]);
        assert_eq!(director.assignment().values(), &[Some(0)]);
        assert_eq!(director.score(), SimpleScore::ZERO);
        assert_eq!(
            director
                .constraint_scores()
                .map(|(_, score)| score)
                .collect::<Vec<_>>(),
            vec![SimpleScore::ZERO; 2]
        );
    }
    mode.set(0);
    director.apply(&movement).unwrap().commit();
    assert_eq!(calls.take(), vec![0, 1]);
    assert_eq!(director.score(), SimpleScore::new(2));
    director.assert_consistent().unwrap();
}

#[test]
fn actual_best_selection_and_random_ties_match_the_configurable_weight_route() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap();
    for tie_breaking in [
        TieBreaking::First,
        TieBreaking::Random { seed: 7 },
        TieBreaking::Random { seed: 23 },
    ] {
        let mut direct = ConstraintNetwork::new(1);
        direct
            .add_constraint("equal improvements", vec![0, 0], |v| {
                assert_eq!(v[0], v[1]);
                SimpleScore::new(i64::from(v[0].copied().unwrap() > 0))
            })
            .unwrap();
        let mut weighted = ConstraintNetwork::new(1);
        weighted
            .add_weighted_constraint("equal improvements", vec![0, 0], SimpleScore::new(1), |v| {
                assert_eq!(v[0], v[1]);
                i64::from(v[0].copied().unwrap() > 0)
            })
            .unwrap();
        let config = LocalSearchConfig {
            max_steps: 1,
            max_evaluations: 10,
            selection: MoveSelection::BestAccepted,
            tie_breaking,
            assert_scores: true,
            ..Default::default()
        };
        let results = [&direct, &weighted].map(|network| {
            local_search_from(&p, network, p.initial_assignment(), config.clone()).unwrap()
        });
        assert_eq!(results[0], results[1]);
        assert_eq!(results[0].best.as_ref().unwrap().score, SimpleScore::new(1));
        assert_eq!(results[0].completed_steps, 1);
        if tie_breaking == TieBreaking::First {
            assert_eq!(
                results[0].best.as_ref().unwrap().assignment.values(),
                &[Some(1)]
            );
        }
    }
}
