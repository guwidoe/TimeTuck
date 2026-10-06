// Adapted from pinned Timefold Community DefaultSolverTest and test fixtures.
// Apache-2.0; source identities and native substitutions: compat/provenance.json.
use timetuck::*;

#[derive(Clone, Copy)]
enum Objective {
    Assigned(i64),
    Empty(i64),
}
impl Objective {
    fn score(self, list: &[usize]) -> SimpleScore {
        SimpleScore::new(match self {
            Self::Assigned(weight) => weight * list.len() as i64,
            Self::Empty(weight) => weight * i64::from(list.is_empty()),
        })
    }
}
fn solve(
    p: &ListProblem<usize>,
    objective: Objective,
    construction: ConstructionConfig,
    search: bool,
    target: Option<SimpleScore>,
    stream: bool,
) -> SolverResult<ListAssignment<usize>, SimpleScore> {
    let mut events = 0;
    let mut phases = vec![SolverPhase::Construction(construction)];
    if search {
        phases.push(SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 10,
            neighborhood: Neighborhood::ChangeAndSwap,
            assert_scores: true,
            ..Default::default()
        }));
    }
    let plan = SolverSequence {
        phases,
        limits: SolverLimits {
            best_score_limit: target,
            ..Default::default()
        },
    };
    let r = if stream {
        solve_list_stream_with_observer(
            p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("objective", s.entities.port(), move |(_, list)| {
                    objective.score(list)
                })?;
                Ok((b.build(), s))
            },
            plan,
            |_| events += 1,
        )
        .unwrap()
    } else {
        let mut n = ListConstraintNetwork::new(p.entities().len(), p.values().len());
        for i in 0..p.entities().len() {
            n.add_constraint(
                format!("objective{i}"),
                vec![ListScope::Entity(i)],
                move |f| {
                    let ListFact::Entity(list) = &f[0] else {
                        panic!()
                    };
                    objective.score(list)
                },
            )
            .unwrap();
        }
        solve_list_with_observer(p, &n, plan, |_| events += 1).unwrap()
    };
    assert!(events > 0);
    assert!(r.complete && r.solution().is_some());
    p.validate(&r.assignment).unwrap();
    r
}
fn construction() -> ConstructionConfig<'static> {
    ConstructionConfig {
        optional: OptionalConstruction::EvaluateUnassigned,
        interruption: ConstructionInterruption::DiscardIncompleteStep,
        assert_scores: true,
        ..Default::default()
    }
}
fn optional() -> ListProblem<usize> {
    ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2])])
        .unwrap()
        .allowing_unassigned()
}
#[test]
fn solve_with_allows_unassigned_values_list_variable() {
    for stream in [false, true] {
        let r = solve(
            &optional(),
            Objective::Assigned(-1),
            construction(),
            true,
            Some(SimpleScore::ZERO),
            stream,
        );
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert!(r.assignment.lists()[0].is_empty());
    }
}
#[test]
fn solve_with_allows_unassigned_values_list_variable_and_only_down() {
    for stream in [false, true] {
        // Native early-pick policy replaces Java ONLY_DOWN trend inference.
        let r = solve(
            &optional(),
            Objective::Assigned(1),
            ConstructionConfig {
                pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
                ..construction()
            },
            false,
            None,
            stream,
        );
        assert_eq!(r.score, Some(SimpleScore::new(4)));
        assert_eq!(r.assignment.lists()[0].len(), 4);
        assert_eq!(r.phases[0].work.move_evaluations, 2);
    }
}
#[test]
fn solve_with_ch_allows_unassigned_values_list_variable_and_terminate_in_step() {
    for stream in [false, true] {
        let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![])])
            .unwrap()
            .allowing_unassigned();
        let r = solve(
            &p,
            Objective::Empty(1),
            ConstructionConfig {
                max_move_evaluations: Some(1),
                ..construction()
            },
            false,
            None,
            stream,
        );
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert!(r.assignment.lists()[0].is_empty());
        assert_eq!(r.phases[0].work.move_evaluations, 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Construction(ConstructionTermination::MoveEvaluationLimit)
        );
    }
}
fn pinned(
    prefix: bool,
    fair: bool,
    stream: bool,
) -> SolverResult<ListAssignment<usize>, SimpleScore> {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            if prefix {
                ListEntity::new(vec![1, 2]).with_pinned_prefix(1)
            } else {
                ListEntity::new(vec![])
            },
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    // Native model-derived owner/index views and initial scoring replace the
    // SolutionManager update; incremental/full score assertions replace TRACKED_FULL_ASSERT.
    let r = solve(
        &p,
        Objective::Empty(if fair { -1 } else { 1 }),
        construction(),
        true,
        None,
        stream,
    );
    assert_eq!(r.score, Some(SimpleScore::new(if fair { 0 } else { 1 })));
    assert_eq!(r.assignment.lists()[0], vec![0]);
    assert_eq!(r.assignment.lists().iter().map(Vec::len).sum::<usize>(), 4);
    if prefix {
        assert_eq!(r.assignment.lists()[1][0], 1);
    }
    r
}
#[test]
fn solve_with_planning_list_variable_entity_pin_fair() {
    for stream in [false, true] {
        pinned(false, true, stream);
    }
}
#[test]
fn solve_with_planning_list_variable_entity_pin_unfair() {
    for stream in [false, true] {
        pinned(false, false, stream);
    }
}
#[test]
fn solve_with_planning_list_variable_pin_index_fair() {
    for stream in [false, true] {
        pinned(true, true, stream);
    }
}
#[test]
fn solve_with_planning_list_variable_pin_index_unfair() {
    for stream in [false, true] {
        let r = pinned(true, false, stream);
        let mut values = r.assignment.lists()[1].clone();
        values.sort();
        assert_eq!(values, vec![1, 2, 3]);
        assert!(r.assignment.lists()[2].is_empty());
    }
}
