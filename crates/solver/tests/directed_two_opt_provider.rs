//! Original directed-neighborhood and solver integration tests.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [1, 5, 3, 4, 2]))
}
fn problem() -> ListProblem<i32> {
    ListProblem::new((1..=5).collect(), vec![ListEntity::new((1..=5).collect())]).unwrap()
}
#[test]
fn list_search_finds_a_wrapped_improvement_on_both_backends() {
    let p = problem();
    let mut network = ListConstraintNetwork::new(1, 5);
    network
        .add_constraint("target", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            reward(row)
        })
        .unwrap();
    let config = LocalSearchConfig {
        max_steps: 2,
        max_evaluations: 1000,
        assert_scores: true,
        neighborhood: Neighborhood::TwoOpt {
            cross_entity: false,
        },
        ..Default::default()
    };
    let ordinary = list_local_search_from(&p, &network, p.initial_assignment(), config).unwrap();
    assert_eq!(ordinary.best.unwrap().score, SimpleScore::ZERO);
    for stream in [false, true] {
        let mut provider = DirectedTwoOptProvider::new(false);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("target", sources.entities.port(), |(_, row)| reward(row))
                .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            list_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        }
        .unwrap();
        assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(1));
        assert_eq!(r.best.unwrap().assignment.lists(), &[vec![1, 5, 3, 4, 2]]);
    }
}
#[test]
fn mixed_search_uses_wrapped_moves_and_preserves_basic_assignments_on_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7, 8]).with_initial(7)]).unwrap(),
        problem(),
    );
    let mut network = MixedConstraintNetwork::new(1, 1, 5);
    network
        .add_constraint("target", vec![MixedScope::ListEntity(0)], |facts| {
            let MixedFact::ListEntity(row) = facts[0] else {
                unreachable!()
            };
            reward(row)
        })
        .unwrap();
    let config = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_steps: 2,
            max_evaluations: 1000,
            assert_scores: true,
            ..Default::default()
        },
        joint_moves: true,
    };
    for stream in [false, true] {
        let mut provider = DirectedTwoOptProvider::new(false);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut b);
            b.score("target", sources.lists.entities.port(), |(_, row)| {
                reward(row)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            mixed_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        }
        .unwrap();
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(1));
        assert_eq!(best.assignment.lists().lists(), &[vec![1, 5, 3, 4, 2]]);
        assert_eq!(best.assignment.basic().values(), &[Some(7)]);
    }
}
#[test]
fn provider_includes_empty_receivers_but_omits_invalid_ranges_full_pins_and_noops() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1, 2])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![2]),
            ListEntity::new(vec![3]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let successors = |cross| {
        list_directed_two_opt_moves(&p, &a, cross)
            .unwrap()
            .map(|m| {
                let mut next = a.clone();
                p.apply(&mut next, &m).unwrap().commit();
                assert_ne!(next, a);
                assert_eq!(next.lists()[0][0], 0);
                assert_eq!(next.lists()[2], vec![3]);
                next
            })
            .collect::<Vec<_>>()
    };
    let local = successors(false);
    assert!(local.iter().all(|a| a.lists()[1].is_empty()));
    let cross = successors(true);
    assert!(
        cross
            .iter()
            .any(|a| a.lists() == [vec![0, 1], vec![2], vec![3]])
    );
    for a in cross {
        assert!(a.lists()[1].iter().all(|&v| v == 2));
    }
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    assert_eq!(
        list_directed_two_opt_moves(&p, &p.initial_assignment(), true)
            .unwrap()
            .count(),
        0
    );
    assert!(list_directed_two_opt_moves(&p, &ListAssignment::new(vec![vec![99]]), true).is_err());
}
#[derive(Debug)]
struct Counted {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Counted {}
#[test]
fn opening_is_lazy_and_search_limits_bound_scored_candidates() {
    let count = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..30)
        .map(|id| Counted {
            id,
            clones: count.clone(),
        })
        .collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut moves = list_directed_two_opt_moves(&p, &a, true).unwrap();
    assert_eq!(count.get(), 0);
    assert!(moves.next().is_some());
    let first = count.get();
    assert!(first > 0);
    drop(moves);
    assert_eq!(count.get(), first);
    let p = problem();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 5);
    let mut provider = DirectedTwoOptProvider::new(true);
    let r = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_evaluations: 2,
            max_steps: 10,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert!(r.evaluations <= 2);
}
