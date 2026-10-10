// Adapted from pinned Timefold Community AllowsUnassignedValuesListVariableSolverTest
// and its test-domain easy scorer. Apache-2.0; see compat/provenance.json.
use timetuck::*;

fn configurations() -> [Vec<(Neighborhood, u32)>; 6] {
    let change = (Neighborhood::Change, 1);
    let swap = (Neighborhood::Swap, 1);
    let sub_change = (
        Neighborhood::SubListChange(SubListChangeConfig::default()),
        1,
    );
    let sub_swap = (Neighborhood::SubListSwap(SubListSwapConfig::default()), 1);
    let kopt = (
        Neighborhood::RepartitionKOpt(ListKOptSampling::new(
            ListKOptConfig::new(2, 3).unwrap(),
            0,
            200,
        )),
        1,
    );
    [
        vec![change, swap],
        vec![change, swap, sub_change],
        vec![change, swap, sub_swap],
        vec![kopt],
        vec![change, swap, kopt],
        vec![change, swap, sub_change, sub_swap, kopt],
    ]
}
fn objective(lists: &[Vec<usize>]) -> SimpleScore {
    SimpleScore::new(-(lists.iter().map(Vec::len).sum::<usize>() as i64))
}
fn exercise(initial: Vec<Vec<usize>>, steps: u64) {
    let p = ListProblem::new(
        (0..25).collect(),
        initial.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
    .allowing_unassigned();
    for (index, children) in configurations().iter().enumerate() {
        let union = NeighborhoodUnion {
            children,
            order: UnionOrder::Random { seed: 0 },
        };
        let extra = [
            Acceptance::ValueTabu {
                tenure: 1,
                aspiration: true,
            },
            Acceptance::MoveTabu {
                tenure: 1,
                aspiration: true,
            },
        ];
        for stream in [false, true] {
            let plan = SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig {
                        optional: OptionalConstruction::EvaluateUnassigned,
                        assert_scores: true,
                        ..Default::default()
                    }),
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        neighborhood_union: Some(&union),
                        acceptance: Acceptance::VariableTabu {
                            tenure: 1,
                            aspiration: true,
                        },
                        additional_acceptance: &extra,
                        max_steps: steps,
                        max_evaluations: u64::MAX,
                        max_accepted_moves: u64::MAX,
                        accepted_candidate_limit: std::num::NonZeroU64::new(100),
                        selection: MoveSelection::BestAccepted,
                        rejected_moves: RejectedMovePolicy::BestScore,
                        tie_breaking: TieBreaking::Random { seed: 0 },
                        assert_scores: true,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            };
            let mut updates = 0;
            let mut observe = |u: SolverUpdate<'_, ListAssignment<usize>, SimpleScore>| {
                p.validate(u.assignment).unwrap();
                assert_eq!(u.score, objective(u.assignment.lists()));
                updates += 1;
            };
            let result = if stream {
                solve_list_stream_with_observer(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let sources = ListStreamSources::new(&mut b);
                        b.score("assigned", sources.entities.port(), |(_, list)| {
                            SimpleScore::new(-(list.len() as i64))
                        })?;
                        Ok((b.build(), sources))
                    },
                    plan,
                    &mut observe,
                )
                .unwrap()
            } else {
                let mut n = ListConstraintNetwork::new(5, 25);
                for entity in 0..5 {
                    n.add_constraint(
                        format!("assigned{entity}"),
                        vec![ListScope::Entity(entity)],
                        |facts| {
                            let ListFact::Entity(list) = &facts[0] else {
                                unreachable!()
                            };
                            SimpleScore::new(-(list.len() as i64))
                        },
                    )
                    .unwrap();
                }
                solve_list_with_observer(&p, &n, plan, &mut observe).unwrap()
            };
            // Source assertion: a solution is returned for every configuration.
            assert!(
                result.solution().is_some(),
                "configuration {index}, stream={stream}"
            );
            // Additional native invariants do not claim Java trace equivalence.
            assert!(result.complete);
            assert!(updates > 0);
            assert_eq!(result.phases.len(), 2);
            assert!(result.work.completed_steps <= steps);
            assert_eq!(result.score, Some(objective(result.assignment.lists())));
            p.validate(&result.assignment).unwrap();
        }
    }
}

#[test]
fn run_solver() {
    exercise(vec![vec![]; 5], 1000);
}

#[test]
fn native_partial_lists_exercise_compound_moves_with_three_tabu_policies() {
    // Original additional scenario: include assigned and missing values so all
    // configured move families have work even when construction prefers absence.
    exercise((0..5).map(|e| (e * 4..e * 4 + 4).collect()).collect(), 32);
}
