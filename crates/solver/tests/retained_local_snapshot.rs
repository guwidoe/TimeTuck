//! Original retained local/custom snapshot regressions; no upstream-method credit.
#[path = "support/snapshot_score.rs"]
mod owned;
use owned::OwnedScore;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![2])],
    )
    .unwrap()
}
fn bn() -> ConstraintNetwork<'static, i64, OwnedScore> {
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![i], move |v| {
            OwnedScore::new(0, v[0].copied().unwrap_or(0) * (1 + 9 * i as i64))
        })
        .unwrap();
    }
    for i in 0..16 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    n
}
fn list_score(i: usize, v: &[i64]) -> OwnedScore {
    OwnedScore::new(
        0,
        v.iter()
            .enumerate()
            .map(|(j, v)| (i as i64 * 9 + 1) * (j as i64 + 1) * v)
            .sum(),
    )
}
fn ln() -> ListConstraintNetwork<'static, i64, OwnedScore> {
    let mut n = ListConstraintNetwork::new(2, 2);
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![ListScope::Entity(i)], move |f| {
            let ListFact::Entity(v) = f[0] else {
                unreachable!()
            };
            list_score(i, v)
        })
        .unwrap();
    }
    for i in 0..16 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    n
}
fn mn() -> MixedConstraintNetwork<'static, i64, i64, OwnedScore> {
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    for i in 0..2 {
        n.add_constraint(format!("b{i}"), vec![MixedScope::Basic(i)], move |f| {
            let MixedFact::Basic(v) = f[0] else {
                unreachable!()
            };
            OwnedScore::new(0, v.copied().unwrap_or(0) * (1 + 9 * i as i64))
        })
        .unwrap();
        n.add_constraint(format!("l{i}"), vec![MixedScope::ListEntity(i)], move |f| {
            let MixedFact::ListEntity(v) = f[0] else {
                unreachable!()
            };
            list_score(i, v)
        })
        .unwrap();
    }
    for i in 0..16 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    n
}
fn normalize<A, S>(mut r: SolverResult<A, S>) -> SolverResult<A, S> {
    r.work.constraint_evaluations = 0;
    for p in &mut r.phases {
        p.work.constraint_evaluations = 0;
    }
    r
}
fn mixed(c: SolverSequence<'static, OwnedScore>) -> MixedSolverSequence<'static, OwnedScore> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
                _ => unreachable!(),
            })
            .collect(),
    }
}
fn change(i: usize, v: i64) -> Move<i64> {
    Move::Change {
        variable: i,
        value: Some(v),
    }
}
fn relocate(from: usize, index: usize, to: usize, at: usize) -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: from,
            index,
        },
        destination: ListPosition {
            entity: to,
            index: at,
        },
    }
}
fn list_moves() -> Vec<ListMove<i64>> {
    vec![
        relocate(0, 0, 1, 1),
        relocate(1, 0, 0, 0),
        relocate(1, 0, 0, 1),
        relocate(0, 0, 1, 0),
        relocate(0, 0, 1, 0),
    ]
}
#[test]
fn repeated_local_phases_match_free_solves_under_rejection_and_interruption() {
    for acceptance in [
        Acceptance::HillClimbing,
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: OwnedScore::new(100, 100),
            cooling_steps: 8,
            seed: 7,
        },
    ] {
        for cap in [1, 2, 5, 20, 100] {
            let phase = SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5,
                acceptance: acceptance.clone(),
                assert_scores: true,
                move_order: MoveOrder::ShuffledMoves {
                    seed: 17,
                    window: std::num::NonZeroUsize::new(3).unwrap(),
                },
                ..Default::default()
            });
            let c = SolverSequence {
                limits: SolverLimits {
                    max_evaluations: cap,
                    ..Default::default()
                },
                phases: vec![phase.clone(), phase],
            };
            let p = basic();
            let n = bn();
            let expected = solve(&p, &n, c.clone()).unwrap();
            let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let actual = d.run_solver(c.clone()).unwrap();
            assert_eq!(normalize(actual), normalize(expected));
            d.assert_consistent().unwrap();
            let p = lists();
            let n = ln();
            let expected = solve_list(&p, &n, c.clone()).unwrap();
            let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let actual = d.run_solver(c.clone()).unwrap();
            assert_eq!(normalize(actual), normalize(expected));
            d.assert_consistent().unwrap();
            let p = MixedProblem::new(basic(), lists());
            let n = mn();
            let c = mixed(c);
            let expected = solve_mixed(&p, &n, c.clone()).unwrap();
            let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let actual = d.run_solver(c).unwrap();
            assert_eq!(normalize(actual), normalize(expected));
            d.assert_consistent().unwrap();
        }
    }
}
fn basic_command(
    _: usize,
    c: &mut CustomPhaseContext<'_, Assignment<i64>, Move<i64>, OwnedScore, ScoringError>,
) -> Result<(), CustomPhaseError<ScoringError>> {
    c.evaluate(&change(1, 2), |_, s| s)?;
    for m in [change(0, 2), change(0, 1), change(1, 1), change(1, 0)] {
        if c.execute(&m)?.is_none() {
            break;
        }
    }
    Ok(())
}
fn list_command(
    _: usize,
    c: &mut CustomPhaseContext<
        '_,
        ListAssignment<i64>,
        ListMove<i64>,
        OwnedScore,
        ListScoringError,
    >,
) -> Result<(), CustomPhaseError<ListScoringError>> {
    c.evaluate(&relocate(0, 0, 1, 0), |_, s| s)?;
    for m in list_moves() {
        if c.execute(&m)?.is_none() {
            break;
        }
    }
    Ok(())
}
fn mixed_command(
    _: usize,
    c: &mut CustomPhaseContext<
        '_,
        MixedAssignment<i64, i64>,
        MixedMove<i64, i64>,
        OwnedScore,
        MixedScoringError,
    >,
) -> Result<(), CustomPhaseError<MixedScoringError>> {
    let moves = list_moves();
    for (b, l) in [
        change(0, 2),
        change(0, 1),
        change(1, 1),
        change(1, 0),
        change(1, 2),
    ]
    .into_iter()
    .zip(moves)
    {
        if c.execute(&MixedMove { basic: b, lists: l })?.is_none() {
            break;
        }
    }
    Ok(())
}
#[test]
fn custom_trials_nonbest_commits_and_final_restore_match_all_models() {
    for cap in 1..=9 {
        let c = SolverSequence {
            limits: SolverLimits {
                max_evaluations: cap,
                ..Default::default()
            },
            phases: vec![SolverPhase::Custom(Default::default())],
        };
        let p = basic();
        let n = bn();
        let expected = solve_with_extensions(
            &p,
            &n,
            c.clone(),
            SolverExtensions {
                command: Some(&mut basic_command),
                ..Default::default()
            },
        )
        .unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let actual = d
            .run_solver_with_extensions(
                c.clone(),
                SolverExtensions {
                    command: Some(&mut basic_command),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
        let p = lists();
        let n = ln();
        let expected = solve_list_with_extensions(
            &p,
            &n,
            c.clone(),
            SolverExtensions {
                command: Some(&mut list_command),
                ..Default::default()
            },
        )
        .unwrap();
        let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let actual = d
            .run_solver_with_extensions(
                c.clone(),
                SolverExtensions {
                    command: Some(&mut list_command),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
        let p = MixedProblem::new(basic(), lists());
        let n = mn();
        let c = mixed(c);
        let expected = solve_mixed_with_extensions(
            &p,
            &n,
            c.clone(),
            SolverExtensions {
                command: Some(&mut mixed_command),
                ..Default::default()
            },
        )
        .unwrap();
        let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let actual = d
            .run_solver_with_extensions(
                c,
                SolverExtensions {
                    command: Some(&mut mixed_command),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(normalize(actual), normalize(expected));
        d.assert_consistent().unwrap();
    }
}
#[test]
fn custom_error_and_observer_panic_leave_a_reusable_committed_director() {
    let p = basic();
    let n = bn();
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let c = SolverSequence {
        phases: vec![SolverPhase::Custom(Default::default())],
        ..Default::default()
    };
    let mut fail = |_: usize,
                    c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        OwnedScore,
        ScoringError,
    >| {
        c.execute(&change(0, 1))?;
        c.execute(&change(99, 1))?;
        Ok(())
    };
    assert!(
        d.run_solver_with_extensions(
            c.clone(),
            SolverExtensions {
                command: Some(&mut fail),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(d.score(), OwnedScore::new(0, 1));
    d.assert_consistent().unwrap();
    let seen = Cell::new(0);
    let mut observer = |e: SolverEvent<'_, Assignment<i64>, OwnedScore>| {
        if matches!(e, SolverEvent::Best(_)) {
            seen.set(seen.get() + 1);
            if seen.get() == 2 {
                panic!("observer failure");
            }
        }
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| d.run_solver_with_extensions(
            c.clone(),
            SolverExtensions {
                command: Some(&mut basic_command),
                events: Some(&mut observer),
                ..Default::default()
            }
        )))
        .is_err()
    );
    d.assert_consistent().unwrap();
    let r = d
        .run_solver_with_extensions(
            c,
            SolverExtensions {
                command: Some(&mut basic_command),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(OwnedScore::new(0, 11)));
    d.assert_consistent().unwrap();
}
#[test]
fn custom_many_best_updates_avoid_full_cache_clone_work() {
    let p = FiniteProblem::new(vec![Variable::new((0..128i64).collect()).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        OwnedScore::new(0, v[0].copied().unwrap())
    })
    .unwrap();
    for i in 0..512 {
        n.add_constraint(i.to_string(), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    owned::reset_clones();
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        OwnedScore,
        ScoringError,
    >| {
        for v in 1..128 {
            c.execute(&change(0, v))?;
        }
        Ok(())
    };
    let r = d
        .run_solver_with_extensions(
            SolverSequence {
                phases: vec![SolverPhase::Custom(Default::default())],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(OwnedScore::new(0, 127)));
    assert_eq!(r.work.constraint_evaluations, 127);
    assert!(owned::clones() < 16000, "{} score clones", owned::clones());
    d.assert_consistent().unwrap();
}

#[test]
fn provider_local_search_avoids_full_cache_clones_for_each_selected_winner() {
    let p =
        FiniteProblem::new(vec![Variable::new((-1..128i64).collect()).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        OwnedScore::new(0, v[0].copied().unwrap())
    })
    .unwrap();
    for i in 0..512 {
        n.add_constraint(i.to_string(), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    owned::reset_clones();
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
        let v = a.values()[0].unwrap();
        vec![change(0, v + 1), change(0, v - 1)]
    };
    let mut providers = PhaseMoveProviders::default();
    providers.bind(0, &mut provider);
    let r = d
        .run_solver_with_extensions(
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 127,
                    selection: MoveSelection::BestAccepted,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                move_providers: providers,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(OwnedScore::new(0, 127)));
    assert_eq!(r.work.evaluations, 255);
    assert_eq!(r.work.constraint_evaluations, 254);
    assert!(owned::clones() < 32000, "{} score clones", owned::clones());
    d.assert_consistent().unwrap();
}

#[derive(Clone, Debug)]
struct Payload {
    id: usize,
    value: i64,
}
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Payload {}
#[test]
fn identity_equal_commits_restore_payload_and_contribution_snapshots_together() {
    let p = FiniteProblem::new(
        (0..2)
            .map(|id| {
                Variable::new(vec![Payload { id, value: 0 }]).with_initial(Payload { id, value: 0 })
            })
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![i], |v: &[Option<&Payload>]| {
            SimpleScore::new(v[0].unwrap().value)
        })
        .unwrap();
    }
    for i in 0..8 {
        n.add_constraint(format!("constant{i}"), vec![], |_| SimpleScore::ZERO)
            .unwrap();
    }
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<Payload>,
        Move<Payload>,
        SimpleScore,
        ScoringError,
    >| {
        for (id, value) in [(0, 2), (0, 1), (1, 10), (1, 0)] {
            c.execute(&Move::Change {
                variable: id,
                value: Some(Payload { id, value }),
            })?;
        }
        Ok(())
    };
    let r = d
        .run_solver_with_extensions(
            SolverSequence {
                phases: vec![
                    SolverPhase::Custom(Default::default()),
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            },
            SolverExtensions {
                command: Some(&mut command),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(11)));
    assert_eq!(
        d.assignment()
            .values()
            .iter()
            .map(|v| v.as_ref().unwrap().value)
            .collect::<Vec<_>>(),
        [1, 10]
    );
    d.assert_consistent().unwrap();
    d.apply(&Move::Change {
        variable: 0,
        value: Some(Payload { id: 0, value: 2 }),
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), SimpleScore::new(12));
    d.assert_consistent().unwrap();
}
