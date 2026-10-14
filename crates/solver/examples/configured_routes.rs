//! Original example: owned text configuration drives basic/list joint solving.
use std::time::Duration;
use timetuck::*;

fn route_score(depot: Option<i64>, route: &[usize]) -> HardSoftScore {
    let coordinates = [1_i64, 9, 10];
    let depot = depot.unwrap_or(0);
    let mut previous = depot;
    let mut distance = 0;
    for &customer in route {
        distance += (coordinates[customer] - previous).abs();
        previous = coordinates[customer];
    }
    distance += (previous - depot).abs();
    HardSoftScore::new(-(route.len().saturating_sub(2) as i64), -distance)
}

fn independent_score(state: &MixedAssignment<i64, usize>) -> HardSoftScore {
    state
        .lists()
        .lists()
        .iter()
        .enumerate()
        .map(|(owner, route)| route_score(state.basic().values()[owner], route))
        .fold(HardSoftScore::new(0, 0), |total, score| total + score)
}

fn main() {
    let properties = OwnedPropertySource::new([
        (
            "ai.timefold.platform.termination.spent-limit".into(),
            "10s".into(),
        ),
        (
            "timefold.model.termination.diminished-returns.enabled".into(),
            "true".into(),
        ),
        (
            "timefold.model.termination.diminished-returns.sliding-window-duration".into(),
            "PT30S".into(),
        ),
    ]);
    let mut warnings = Vec::new();
    // This explicit native namespace extends the TEST-visible spent-limit key.
    let settings = RuntimePropertyPlan::resolve(
        &properties,
        &PropertyFallbackRules::community_test_profile(),
        "timefold.model.termination",
        &mut |warning| warnings.push(warning),
    )
    .unwrap();
    let mut plan = MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_evaluations: 2_000,
                    max_steps: 20,
                    selection: MoveSelection::BestAccepted,
                    ..Default::default()
                },
                joint_moves: true,
            }),
        ],
        ..Default::default()
    };
    let binding = settings.bind_mixed(&mut plan).unwrap();
    assert_eq!(plan.limits.max_time, Some(Duration::from_secs(10)));
    assert_eq!(binding.applied_local_phases, 1);
    assert_eq!(warnings.len(), 1);
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 10]); 2]).unwrap(),
        ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap(),
    );
    let mut network = MixedConstraintNetwork::new(2, 2, 3);
    for owner in 0..2 {
        network
            .add_constraint(
                format!("route {owner}"),
                vec![MixedScope::Basic(owner), MixedScope::ListEntity(owner)],
                |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    route_score(depot.copied(), route)
                },
            )
            .unwrap();
    }
    let dense = solve_mixed(&problem, &network, plan.clone()).unwrap();
    let stream = solve_mixed_stream(
        &problem,
        || {
            let mut builder = StreamGraphBuilder::new();
            let sources = MixedStreamSources::<i64, usize>::new(&mut builder);
            let routes = builder.join_equal(
                sources.basic.port(),
                sources.lists.entities.port(),
                |(owner, _)| *owner,
                |(owner, _)| *owner,
            )?;
            builder.score("routes", &routes, |((_, depot), (_, route))| {
                route_score(*depot, route)
            })?;
            Ok((builder.build(), sources))
        },
        plan,
    )
    .unwrap();
    for (name, result) in [("dense", dense), ("stream", stream)] {
        assert!(problem.is_complete(&result.assignment));
        problem.validate(&result.assignment).unwrap();
        assert_eq!(result.score, Some(independent_score(&result.assignment)));
        assert_eq!(result.score, Some(HardSoftScore::new(0, -4)));
        println!(
            "{name}: {} in {} evaluations",
            result.score.unwrap(),
            result.work.evaluations
        );
    }
    println!("{}", warnings[0].message);
}
