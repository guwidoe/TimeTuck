//! Jointly choose vehicle depots (basic variables) and ordered customer routes.
use timetuck::{
    Assignment, BranchAndBoundConfig, BranchEvaluation, ConstructionConfig, FiniteProblem,
    HardSoftScore, ListAssignment, ListEntity, ListProblem, LocalSearchConfig, MixedAssignment,
    MixedConstraintNetwork, MixedConstructionConfig, MixedConstructionOrder, MixedFact,
    MixedLocalSearchConfig, MixedProblem, MixedScope, MixedStreamSources, MoveSelection,
    NodeExploration, StreamGraphBuilder, Variable, branch_and_bound_mixed, exhaustive_mixed_search,
    greedy_mixed_construction, greedy_mixed_stream_construction, mixed_local_search_from,
    mixed_stream_local_search_from,
};

fn score(state: &MixedAssignment<i64, usize>) -> HardSoftScore {
    let coordinates = [1_i64, 9, 10];
    let mut overload = 0;
    let mut distance = 0;
    for (vehicle, route) in state.lists().lists().iter().enumerate() {
        let depot = state.basic().values()[vehicle].expect("complete depot choice");
        overload += route.len().saturating_sub(2) as i64;
        let mut previous = depot;
        for &customer in route {
            distance += (coordinates[customer] - previous).abs();
            previous = coordinates[customer];
        }
        distance += (previous - depot).abs();
    }
    HardSoftScore::new(-overload, -distance)
}

fn route_contribution(depot: Option<i64>, route: &[usize]) -> HardSoftScore {
    let depot = depot.unwrap_or(0);
    let points = [1_i64, 9, 10];
    let tour: Vec<_> = std::iter::once(depot)
        .chain(route.iter().map(|&v| points[v]))
        .chain(std::iter::once(depot))
        .collect();
    let distance: i64 = tour.windows(2).map(|edge| (edge[1] - edge[0]).abs()).sum();
    HardSoftScore::new(-(route.len().saturating_sub(2) as i64), -distance)
}
// Inserting a customer cannot reduce a metric tour's distance or overload.
// Undecided depots use their best permitted choice separately for each route.
// This remains optimistic for the combined objective without guessing an optimum.
fn route_bound(state: &MixedAssignment<i64, usize>, undecided_depots: &[usize]) -> HardSoftScore {
    state
        .lists()
        .lists()
        .iter()
        .enumerate()
        .map(|(vehicle, route)| {
            if undecided_depots.contains(&vehicle) {
                [0, 10]
                    .into_iter()
                    .map(|depot| route_contribution(Some(depot), route))
                    .max()
                    .unwrap()
            } else {
                route_contribution(state.basic().values()[vehicle], route)
            }
        })
        .fold(HardSoftScore::new(0, 0), |total, s| total + s)
}
fn route_graph() -> (
    timetuck::StreamGraph<'static, HardSoftScore>,
    MixedStreamSources<i64, usize>,
) {
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    let joined = b
        .join_equal(
            sources.basic.port(),
            sources.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    b.score("routes", &joined, |((_, depot), (_, route))| {
        route_contribution(*depot, route)
    })
    .unwrap();
    (b.build(), sources)
}

fn main() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 10]), Variable::new(vec![0, 10])]).unwrap(),
        ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let result = exhaustive_mixed_search(&problem, score, 96);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 96);
    let best = result.best.unwrap();
    assert!(problem.is_complete(&best.assignment));
    assert_eq!(best.score, score(&best.assignment));
    // Customers 0 and 1 are each one unit from their nearest available depot.
    // In separate routes they each cost at least 2; in one route their span of
    // 8 costs at least 16. Therefore every solution has total distance >= 4.
    // Routes [customer 0] from depot 0 and [1, 2] from depot 10 attain this bound.
    assert_eq!(best.score, HardSoftScore::new(0, -4));
    println!(
        "{} after {} complete mixed assignments",
        best.score, result.evaluations
    );
    println!(
        "depots: {:?}; routes: {:?}",
        best.assignment.basic().values(),
        best.assignment.lists().lists()
    );
    let mut network = MixedConstraintNetwork::<i64, usize, HardSoftScore>::new(2, 2, 3);
    for vehicle in 0..2 {
        network
            .add_constraint(
                format!("route {vehicle}"),
                vec![MixedScope::Basic(vehicle), MixedScope::ListEntity(vehicle)],
                |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    route_contribution(depot.copied(), route)
                },
            )
            .unwrap();
    }
    for exploration in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
    ] {
        let bounded = branch_and_bound_mixed(
            &problem,
            |state, basic, _| {
                BranchEvaluation::bounded(
                    network.calculate(&problem, state).unwrap(),
                    route_bound(state, basic),
                )
            },
            BranchAndBoundConfig {
                exploration,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bounded.is_optimal());
        let solution = bounded.best.unwrap();
        assert_eq!(solution.score, best.score);
        assert_eq!(solution.score, score(&solution.assignment));
        assert!(bounded.complete_evaluations < result.evaluations);
        println!(
            "Bounded {exploration:?}: {} after {} nodes / {} complete assignments / {} pruned branches",
            solution.score,
            bounded.evaluations,
            bounded.complete_evaluations,
            bounded.pruned_branches
        );
    }
    let initial = MixedAssignment::new(
        Assignment::new(vec![Some(0), Some(0)]),
        ListAssignment::new(vec![vec![0, 1], vec![2]]),
    );
    let local = mixed_local_search_from(
        &problem,
        &network,
        initial,
        MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_evaluations: 1000,
                max_steps: 20,
                selection: MoveSelection::BestAccepted,
                assert_scores: true,
                ..Default::default()
            },
            joint_moves: true,
        },
    )
    .unwrap();
    let solution = local.best.unwrap();
    assert!(problem.is_complete(&solution.assignment));
    assert_eq!(solution.score, score(&solution.assignment));
    assert_eq!(solution.score, best.score);
    println!(
        "Local search: {} after {} evaluations / {} callbacks / {} steps",
        solution.score, local.evaluations, local.constraint_evaluations, local.completed_steps
    );
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let construction = greedy_mixed_construction(
            &problem,
            &network,
            MixedConstructionConfig {
                construction: ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                order,
            },
        )
        .unwrap();
        let constructed = construction.solution().expect("complete construction");
        assert_eq!(constructed.score, score(&constructed.assignment));
        let improved = mixed_local_search_from(
            &problem,
            &network,
            constructed.assignment,
            MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 1000,
                    max_steps: 20,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: true,
            },
        )
        .unwrap();
        let solution = improved.best.unwrap();
        assert!(problem.is_complete(&solution.assignment));
        assert_eq!(solution.score, score(&solution.assignment));
        assert_eq!(solution.score, best.score);
        println!(
            "{order:?}: {} constructed, {} improved; {} combined evaluations / {} callbacks",
            constructed.score,
            solution.score,
            construction.evaluations + improved.evaluations,
            construction.constraint_evaluations + improved.constraint_evaluations,
        );
        let (graph, sources) = route_graph();
        let construction = greedy_mixed_stream_construction(
            &problem,
            graph,
            sources,
            MixedConstructionConfig {
                construction: ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                order,
            },
        )
        .unwrap();
        let constructed = construction
            .solution()
            .expect("complete stream construction");
        assert_eq!(constructed.score, score(&constructed.assignment));
        let (graph, sources) = route_graph();
        let improved = mixed_stream_local_search_from(
            &problem,
            graph,
            sources,
            constructed.assignment,
            MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 1000,
                    max_steps: 20,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: true,
            },
        )
        .unwrap();
        let solution = improved.best.unwrap();
        assert!(problem.is_complete(&solution.assignment));
        assert_eq!(solution.score, score(&solution.assignment));
        assert_eq!(solution.score, best.score);
        println!(
            "Stream {order:?}: {} constructed, {} improved; {} combined evaluations / {} terminal calls",
            constructed.score,
            solution.score,
            construction.evaluations + improved.evaluations,
            construction.constraint_evaluations + improved.constraint_evaluations
        );
        let plan = timetuck::MixedSolverConfig {
            construction: MixedConstructionConfig {
                construction: ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                order,
            },
            local_search: vec![MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 1000,
                    max_steps: 20,
                    selection: MoveSelection::BestAccepted,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: true,
            }],
            ..Default::default()
        };
        let automatic = timetuck::solve_mixed(&problem, &network, plan.clone()).unwrap();
        let automatic_stream =
            timetuck::solve_mixed_stream(&problem, || Ok(route_graph()), plan).unwrap();
        for r in [&automatic, &automatic_stream] {
            assert!(r.complete);
            assert_eq!(r.score, Some(best.score));
            assert_eq!(r.score, Some(score(&r.assignment)));
            assert_eq!(r.phases.len(), 2);
            assert_eq!(r.termination, timetuck::SolverTermination::PlanCompleted);
        }
        assert_eq!(
            automatic_stream.work.evaluations,
            construction.evaluations + improved.evaluations
        );
        assert_eq!(
            automatic_stream.work.constraint_evaluations,
            construction.constraint_evaluations + improved.constraint_evaluations
        );
        println!(
            "Orchestrated {order:?}: {} / {} evaluations, both at {}",
            automatic.work.evaluations, automatic_stream.work.evaluations, best.score
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stream_bounded_search_matches_route_scoring_at_every_partial_state() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 10]); 2]).unwrap(),
            ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap(),
        );
        for exploration in [
            NodeExploration::DepthFirst,
            NodeExploration::BreadthFirst,
            NodeExploration::ScoreFirst,
            NodeExploration::OptimisticBoundFirst,
        ] {
            let r = branch_and_bound_mixed(
                &p,
                |a, basic, lists| {
                    let (graph, sources) = route_graph();
                    let director =
                        timetuck::MixedStreamScoreDirector::new(&p, graph, sources, a.clone())
                            .unwrap();
                    let expected = a
                        .lists()
                        .lists()
                        .iter()
                        .enumerate()
                        .map(|(i, row)| route_contribution(a.basic().values()[i], row))
                        .fold(HardSoftScore::new(0, 0), |sum, s| sum + s)
                        .with_structural(-((basic.len() + lists.len()) as i64));
                    assert_eq!(director.score(), expected);
                    if basic.is_empty() && lists.is_empty() {
                        assert_eq!(expected, score(a));
                    }
                    BranchEvaluation::bounded(director.score(), route_bound(a, basic))
                },
                BranchAndBoundConfig {
                    exploration,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(r.is_optimal());
            assert_eq!(r.best.unwrap().score, HardSoftScore::new(0, -4));
        }
    }
    // Enumerate independently scored complete states, then check every unknown-
    // bound tree prefix against all compatible completions. Later insertions can
    // shift positions, so remove unresolved customers before comparing routes.
    #[test]
    fn metric_bound_dominates_every_compatible_completion() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 10]); 2]).unwrap(),
            ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap(),
        );
        let mut full = Vec::new();
        exhaustive_mixed_search(
            &p,
            |a| {
                let s = score(a);
                full.push((a.clone(), s));
                s
            },
            u64::MAX,
        );
        let r = branch_and_bound_mixed(
            &p,
            |a, basic, lists| {
                let bound = route_bound(a, basic);
                let mut matches = 0;
                for (complete, s) in &full {
                    let depots_match = (0..2).all(|i| {
                        basic.contains(&i) || complete.basic().values()[i] == a.basic().values()[i]
                    });
                    let rows: Vec<Vec<_>> = complete
                        .lists()
                        .lists()
                        .iter()
                        .map(|row| row.iter().filter(|v| !lists.contains(v)).copied().collect())
                        .collect();
                    if depots_match && rows == a.lists().lists() {
                        matches += 1;
                        assert!(*s <= bound);
                        if basic.is_empty() && lists.is_empty() {
                            assert_eq!(*s, bound);
                        }
                    }
                }
                assert!(matches > 0);
                BranchEvaluation::unbounded(bound)
            },
            BranchAndBoundConfig::default(),
        )
        .unwrap();
        assert_eq!(r.complete_evaluations, 96);
        assert_eq!(r.best.unwrap().score, HardSoftScore::new(0, -4));
    }
}
