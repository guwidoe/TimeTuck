//! Original sampled gather search integration tests.
use timetuck::*;
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [1, 3, 0, 2]))
}
fn problem() -> ListProblem<i32> {
    ListProblem::new((0..4).collect(), vec![ListEntity::new((0..4).collect())]).unwrap()
}
#[test]
fn list_search_gathers_nonadjacent_values_on_both_backends() {
    let p = problem();
    let mut network = ListConstraintNetwork::new(1, 4);
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
    for stream in [false, true] {
        let mut provider =
            GatherSamplingProvider::new(GatherSamplingConfig::new(2, 2, 300, 31).unwrap());
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
        assert_eq!(r.best.unwrap().assignment.lists(), &[vec![1, 3, 0, 2]]);
    }
}
#[test]
fn mixed_search_gathers_and_preserves_basic_assignments_on_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7, 8]).with_initial(7)]).unwrap(),
        problem(),
    );
    let mut network = MixedConstraintNetwork::new(1, 1, 4);
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
        let mut provider =
            GatherSamplingProvider::new(GatherSamplingConfig::new(2, 2, 300, 31).unwrap());
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
        assert_eq!(best.assignment.lists().lists(), &[vec![1, 3, 0, 2]]);
        assert_eq!(best.assignment.basic().values(), &[Some(7)]);
    }
}
