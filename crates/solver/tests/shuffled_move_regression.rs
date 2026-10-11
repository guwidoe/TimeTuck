//! Original seeded ordering/search regressions; no upstream coverage credit.
use std::num::NonZeroUsize;
use timetuck::*;
fn shuffled(seed: u64, window: usize) -> MoveOrder {
    MoveOrder::ShuffledMoves {
        seed,
        window: NonZeroUsize::new(window).unwrap(),
    }
}
#[test]
fn basic_scoped_and_stream_search_escape_neutral_cycles_with_repeatable_budgets() {
    let p = FiniteProblem::new(
        (0..3)
            .map(|_| Variable::new(vec![0, 1]).with_initial(0))
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("last", vec![2], |v| {
        SimpleScore::new(i64::from(v[0] == Some(&1)))
    })
    .unwrap();
    for order in [MoveOrder::Original, shuffled(20261006, 16)] {
        let expected = SimpleScore::new(i64::from(order != MoveOrder::Original));
        for budget in [1, 2, 1000] {
            let config = LocalSearchConfig {
                move_order: order,
                neighborhood: Neighborhood::Change,
                acceptance: Acceptance::HillClimbing,
                best_score_limit: Some(SimpleScore::new(1)),
                max_steps: 100,
                max_evaluations: budget,
                assert_scores: true,
                ..Default::default()
            };
            let a = local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
            let again = local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
            assert_eq!(a, again);
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("last", source.port(), |(i, v)| {
                SimpleScore::new(i64::from(*i == 2 && *v == Some(1)))
            })
            .unwrap();
            let other =
                stream_local_search_from(&p, b.build(), source, p.initial_assignment(), config)
                    .unwrap();
            assert_eq!(a.best, other.best);
            assert_eq!(a.evaluations, other.evaluations);
            assert_eq!(a.completed_steps, other.completed_steps);
            assert_eq!(a.termination, other.termination);
            assert!(a.evaluations <= budget);
            if budget == 1000 {
                assert_eq!(a.best.unwrap().score, expected);
            }
        }
    }
}
#[test]
fn shuffled_list_and_joint_mixed_search_escape_neutral_cycles_in_both_backends() {
    fn reward(route: &[i32]) -> SimpleScore {
        SimpleScore::new(i64::from(route == [0, 2, 1]))
    }
    let p = ListProblem::new((0..3).collect(), vec![ListEntity::new((0..3).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("target", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(row) = facts[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for order in [MoveOrder::Original, shuffled(20261006, 64)] {
        let expected = SimpleScore::new(i64::from(order != MoveOrder::Original));
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::TwoOpt { cross_entity: true },
            move_order: order,
            acceptance: Acceptance::HillClimbing,
            best_score_limit: Some(SimpleScore::new(1)),
            max_evaluations: 100_000,
            max_steps: 100,
            selection: MoveSelection::FirstAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let scoped =
            list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("target", s.entities.port(), |(_, row)| reward(row))
            .unwrap();
        let stream =
            list_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config.clone())
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
        assert_eq!(scoped.evaluations, stream.evaluations);
        assert_eq!(scoped.termination, stream.termination);
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            p.clone(),
        );
        let mut n = MixedConstraintNetwork::new(1, 1, 3);
        n.add_constraint(
            "joint",
            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
            |facts| {
                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&facts[0], &facts[1])
                else {
                    unreachable!()
                };
                if v.copied() == Some(1) {
                    reward(row)
                } else {
                    SimpleScore::new(0)
                }
            },
        )
        .unwrap();
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: true,
        };
        let scoped =
            mixed_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        let joined = b
            .join_equal(
                s.basic.port(),
                s.lists.entities.port(),
                |(i, _)| *i,
                |(i, _)| *i,
            )
            .unwrap();
        b.score("joint", &joined, |((_, v), (_, row))| {
            if *v == Some(1) {
                reward(row)
            } else {
                SimpleScore::new(0)
            }
        })
        .unwrap();
        let stream =
            mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config)
                .unwrap();
        assert_eq!(scoped.best.as_ref().unwrap().score, expected);
        assert_eq!(scoped.best, stream.best);
        assert_eq!(scoped.completed_steps, stream.completed_steps);
        assert_eq!(scoped.evaluations, stream.evaluations);
        assert_eq!(scoped.termination, stream.termination);
    }
}

#[test]
fn one_slot_window_preserves_original_search_and_large_windows_respect_tiny_budgets() {
    let p = ListProblem::new((0..8).collect(), vec![ListEntity::new((0..8).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 8);
    n.add_constraint("position", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        SimpleScore::new(
            row.iter()
                .enumerate()
                .map(|(i, v)| i as i64 * i64::from(*v))
                .sum(),
        )
    })
    .unwrap();
    for budget in [0, 1, 2, 3, 37] {
        let config = LocalSearchConfig {
            acceptance: Acceptance::HillClimbing,
            max_evaluations: budget,
            max_steps: 10,
            ..Default::default()
        };
        let original =
            list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        let one = list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                move_order: shuffled(99, 1),
                ..config.clone()
            },
        )
        .unwrap();
        assert_eq!(original, one);
        let large = list_local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                move_order: shuffled(99, usize::MAX),
                ..config.clone()
            },
        )
        .unwrap();
        assert!(large.evaluations <= budget);
        assert_eq!(
            large,
            list_local_search_from(
                &p,
                &n,
                p.initial_assignment(),
                LocalSearchConfig {
                    move_order: shuffled(99, usize::MAX),
                    ..config.clone()
                }
            )
            .unwrap()
        );
    }
}
