//! Original mixed neighborhood and search regressions.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
fn problem() -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    )
}
fn parts(facts: &[MixedFact<'_, i32, i32>]) -> (i32, usize) {
    let MixedFact::Basic(value) = facts[0] else {
        panic!()
    };
    let MixedFact::ListValue { ref element, .. } = facts[1] else {
        panic!()
    };
    (*value.unwrap(), element.as_ref().unwrap().position.entity)
}
fn trap(basic: i32, owner: usize) -> i64 {
    match (basic, owner) {
        (0, 0) => 0,
        (1, 1) => 10,
        _ => -5,
    }
}
fn network() -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut net = MixedConstraintNetwork::new(1, 2, 1);
    net.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let (b, o) = parts(f);
            SimpleScore::new(trap(b, o))
        },
    )
    .unwrap();
    net
}
fn config() -> MixedLocalSearchConfig {
    MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_evaluations: 1000,
            max_steps: 10,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        },
        joint_moves: true,
    }
}

#[test]
fn joint_neighborhood_escapes_a_coupled_trap_and_matches_exact_optimum() {
    let p = problem();
    let n = network();
    let mut c = config();
    c.joint_moves = false;
    let separate = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(separate.best.unwrap().score, SimpleScore::ZERO);
    assert_eq!(separate.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(separate.evaluations, 3);
    c.joint_moves = true;
    let joint = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    let exact = exhaustive_mixed_search(
        &p,
        |s| {
            SimpleScore::new(trap(
                s.basic().values()[0].unwrap(),
                s.lists().location(&7).unwrap().entity,
            ))
        },
        u64::MAX,
    );
    assert_eq!(
        joint.best.as_ref().unwrap().score,
        exact.best.unwrap().score
    );
    assert_eq!(joint.best.unwrap().score, SimpleScore::new(10));
    assert_eq!(joint.evaluations, 7);
    assert_eq!(joint.completed_steps, 1);
}

#[test]
fn neighborhood_successors_match_independent_complete_state_product() {
    for optional in [false, true] {
        let mut lists = ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap();
        if optional {
            lists = lists.allowing_unassigned();
        }
        let p = MixedProblem::new(
            FiniteProblem::new(vec![
                Variable::new(vec![0, 1])
                    .with_initial(0)
                    .allowing_unassigned(),
            ])
            .unwrap(),
            lists,
        );
        let mut state = p.initial_assignment();
        let original = state.clone();
        let mut actual = BTreeSet::new();
        for movement in mixed_moves(&p, &original, Neighborhood::ChangeAndSwap, true).unwrap() {
            let trial = p.apply(&mut state, &movement).unwrap();
            assert!(p.is_complete(&trial));
            actual.insert((
                trial.basic().values()[0],
                trial.lists().location(&7).map(|p| p.entity),
            ));
        }
        let mut expected = BTreeSet::new();
        for b in [Some(0), Some(1), None] {
            for o in [Some(0), Some(1), None] {
                if (!optional && o.is_none()) || (b == Some(0) && o == Some(0)) {
                    continue;
                }
                expected.insert((b, o));
            }
        }
        assert_eq!(actual, expected);
        assert_eq!(state, original);
    }
}

#[test]
fn budgeted_best_prefix_installs_saved_winner_without_callback_replay() {
    let p = problem();
    let calls = Cell::new(0);
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "linear",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            calls.set(calls.get() + 1);
            let (b, o) = parts(f);
            SimpleScore::new(i64::from(b) + 2 * o as i64)
        },
    )
    .unwrap();
    let mut c = config();
    c.search.assert_scores = false;
    c.search.max_evaluations = 3;
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.evaluations, 3);
    assert_eq!(calls.get(), 3);
    assert_eq!(r.constraint_evaluations, 3);
    assert_eq!(r.completed_steps, 1);
    assert_eq!(r.accepted_moves, 1);
    assert_eq!(r.termination, LocalSearchTermination::EvaluationLimit);
    let best = r.best.unwrap();
    assert_eq!(best.score, SimpleScore::new(2));
    assert_eq!(best.assignment.basic().values(), &[Some(0)]);
    assert_eq!(best.assignment.lists().location(&7).unwrap().entity, 1);
    calls.set(0);
    c.search.max_evaluations = 4;
    c.search.max_steps = 1;
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.best.unwrap().score, SimpleScore::new(3));
    assert_eq!(calls.get(), 4);
    assert_eq!(r.termination, LocalSearchTermination::StepLimit);
}

#[test]
fn tabu_keeps_basic_and_list_keys_distinct_and_preserves_earliest_global_best() {
    let p = problem();
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "linear",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let (b, o) = parts(f);
            SimpleScore::new(i64::from(b) + o as i64)
        },
    )
    .unwrap();
    let mut c = config();
    c.joint_moves = false;
    c.search.selection = MoveSelection::FirstAccepted;
    c.search.max_steps = 2;
    c.search.acceptance = Acceptance::VariableTabu {
        tenure: 5,
        aspiration: false,
    };
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.best.unwrap().score, SimpleScore::new(2));
    assert_eq!(r.accepted_moves, 2);
    assert_eq!(r.completed_steps, 2);
    let p = MixedProblem::new(
        p.basic().clone(),
        ListProblem::new(vec![7], vec![ListEntity::new(vec![7]).with_pinned(true)]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(1, 1, 1);
    n.add_constraint("worsen", vec![MixedScope::Basic(0)], |f| {
        let MixedFact::Basic(v) = f[0] else { panic!() };
        SimpleScore::new(if v == Some(&0) { 10 } else { 0 })
    })
    .unwrap();
    c.search.rejected_moves = RejectedMovePolicy::BestScore;
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.best.unwrap().assignment, p.initial_assignment());
    assert_eq!(r.accepted_moves, 1);
    assert_eq!(r.completed_steps, 2);
    assert_eq!(r.termination, LocalSearchTermination::StepLimit);
}

#[test]
fn policies_and_shuffled_orders_are_seeded_and_retain_exact_quality() {
    let p = problem();
    let n = network();
    let policies = [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: SimpleScore::new(20),
            cooling_steps: 10,
            seed: 71,
        },
    ];
    for acceptance in policies {
        for seed in [0, 1, 20261105] {
            let mut c = config();
            c.search.acceptance = acceptance;
            c.search.move_order = MoveOrder::ShuffledVariables { seed };
            let first = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
            let second = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
            assert_eq!(first, second);
            let best = first.best.unwrap();
            assert!(p.is_complete(&best.assignment));
            assert_eq!(best.score, SimpleScore::new(10));
            assert_eq!(n.calculate(&p, &best.assignment).unwrap(), best.score);
            assert!(first.evaluations <= c.search.max_evaluations);
            assert!(first.completed_steps <= c.search.max_steps);
        }
    }
}

#[test]
fn zero_budgets_validate_without_callbacks_and_empty_neighborhood_exhausts() {
    let p = problem();
    let mut n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(1, 2, 1);
    n.add_constraint("never", vec![], |_| panic!("zero budget scored"))
        .unwrap();
    let mut c = config();
    c.search.max_evaluations = 0;
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.evaluations, 0);
    assert!(r.best.is_none());
    let initial = MixedAssignment::new(Assignment::new(vec![None]), p.lists().initial_assignment());
    assert!(matches!(
        mixed_local_search_from(&p, &n, initial, c),
        Err(MixedLocalSearchError::IncompleteInitialAssignment)
    ));
    let wrong = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(2, 2, 1);
    assert!(matches!(
        mixed_local_search_from(&p, &wrong, p.initial_assignment(), c),
        Err(MixedLocalSearchError::Scoring(
            MixedScoringError::ModelShape { .. }
        ))
    ));
    c.search.acceptance = Acceptance::LateAcceptance {
        history_size: 0,
        hill_climbing: false,
    };
    assert!(matches!(
        mixed_local_search_from(&p, &n, p.initial_assignment(), c),
        Err(MixedLocalSearchError::Acceptance(
            AcceptanceError::EmptyHistory
        ))
    ));
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0]).with_initial(0).with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(vec![7], vec![ListEntity::new(vec![7]).with_pinned(true)]).unwrap(),
    );
    let n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(1, 1, 1);
    c = config();
    c.search.max_evaluations = 1;
    let r = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
    assert_eq!(r.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(r.evaluations, 1);
    c.search.max_steps = 0;
    c.search.max_accepted_moves = 0;
    assert_eq!(
        mixed_local_search_from(&p, &n, p.initial_assignment(), c)
            .unwrap()
            .termination,
        LocalSearchTermination::AcceptedMoveLimit
    );
}

#[test]
fn product_enumeration_is_lazy_and_preserves_pins_and_entity_ranges() {
    #[derive(Debug)]
    struct Counted<'a> {
        id: i32,
        calls: &'a Cell<usize>,
    }
    impl Clone for Counted<'_> {
        fn clone(&self) -> Self {
            self.calls.set(self.calls.get() + 1);
            Self {
                id: self.id,
                calls: self.calls,
            }
        }
    }
    impl PartialEq for Counted<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Counted<'_> {}
    let calls = Cell::new(0);
    let value = |id| Counted { id, calls: &calls };
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new((0..64).map(value).collect()).with_initial(value(0)),
            Variable::new(vec![value(0)])
                .with_initial(value(0))
                .with_pinned(true),
        ])
        .unwrap(),
        ListProblem::new(
            vec![7, 11, 13],
            vec![
                ListEntity::new(vec![7, 11])
                    .with_pinned_prefix(1)
                    .with_value_range(vec![7, 11]),
                ListEntity::new(vec![13]).with_pinned(true),
                ListEntity::new(vec![]).with_value_range(vec![11]),
            ],
        )
        .unwrap(),
    );
    let original = p.initial_assignment();
    let mut state = original.clone();
    calls.set(0);
    let mut moves = mixed_moves(&p, &original, Neighborhood::ChangeAndSwap, true).unwrap();
    assert_eq!(calls.get(), 0);
    let first = moves.next().unwrap();
    assert_eq!(calls.get(), 1);
    let mut count = 0;
    for movement in std::iter::once(first).chain(moves) {
        let trial = p.apply(&mut state, &movement).unwrap();
        assert!(p.is_complete(&trial));
        assert_eq!(trial.basic().values()[1].as_ref().unwrap().id, 0);
        assert_eq!(trial.lists().list(0).unwrap()[0], 7);
        assert_eq!(trial.lists().list(1), Some([13].as_slice()));
        count += 1;
    }
    assert_eq!(count, 127);
    assert_eq!(state, original);
}
