//! Original multi-step phase-best assignment regressions; no source-test credit.
use timetuck::*;
fn config(steps: u64) -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: steps,
        selection: MoveSelection::BestAccepted,
        acceptance: Acceptance::StepCountingHillClimbing {
            period: 100,
            mode: StepCountingMode::Step,
        },
        assert_scores: true,
        ..Default::default()
    }
}
fn plan(steps: u64) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::LocalSearch(config(steps))],
        ..Default::default()
    }
}
fn basic_move(n: usize, step: usize, dense: bool) -> Move<usize> {
    let v = [5, 2, 6, 1, 7, 3][step];
    let mut moves = vec![Move::Change {
        variable: 0,
        value: Some(v),
    }];
    for slot in 1..if dense { n } else { 2 } {
        moves.push(Move::Change {
            variable: slot,
            value: Some(step % 8),
        });
    }
    // Repeated writes include a net-zero intermediate change.
    moves.push(Move::Change {
        variable: 0,
        value: Some(0),
    });
    moves.push(Move::Change {
        variable: 0,
        value: Some(v),
    });
    Move::Composite(moves)
}
fn list_move(a: &ListAssignment<usize>, step: usize, dense: bool) -> ListMove<usize> {
    let target = [5, 2, 6, 1, 7, 3][step];
    let at = a.element(&target).unwrap().position;
    let mut moves = vec![ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: at,
    }];
    for i in (2..if dense { a.lists().len() } else { 4 }).step_by(2) {
        if i + 1 < a.lists().len() {
            moves.push(ListMove::Swap {
                left: ListPosition {
                    entity: i,
                    index: 0,
                },
                right: ListPosition {
                    entity: i + 1,
                    index: 0,
                },
            });
        }
    }
    ListMove::Composite(moves)
}
#[test]
fn sparse_and_dense_best_snapshots_match_full_reference_across_worsening_steps() {
    for n in [8, 64, 257] {
        for dense in [false, true] {
            for steps in [2, 6] {
                let p =
                    FiniteProblem::new(vec![Variable::new((0..8).collect()).with_initial(0); n])
                        .unwrap();
                let lp = ListProblem::new_indexed(
                    (0..n).collect(),
                    (0..n).map(|i| ListEntity::new(vec![i])).collect(),
                )
                .unwrap();
                let mut net = ConstraintNetwork::new(n);
                net.add_constraint(
                    "first",
                    vec![0],
                    |f| SimpleScore::new(*f[0].unwrap() as i64),
                )
                .unwrap();
                let mut ln = ListConstraintNetwork::new(n, n);
                ln.add_constraint("first", vec![ListScope::Entity(0)], |f| {
                    let ListFact::Entity(r) = f[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(r[0] as i64)
                })
                .unwrap();
                let mp = MixedProblem::new(p.clone(), lp.clone());
                let mut mn = MixedConstraintNetwork::new(n, n, n);
                mn.add_constraint(
                    "first",
                    vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
                    |f| {
                        let (MixedFact::Basic(b), MixedFact::ListEntity(r)) = (&f[0], &f[1]) else {
                            unreachable!()
                        };
                        SimpleScore::new(*b.unwrap() as i64 + r[0] as i64)
                    },
                )
                .unwrap();
                let mut ba = p.initial_assignment();
                let mut la = lp.initial_assignment();
                let mut best_b = ba.clone();
                let mut best_l = la.clone();
                let mut best = 0;
                for step in 0..steps {
                    p.apply(&mut ba, &basic_move(n, step, dense))
                        .unwrap()
                        .commit();
                    let m = list_move(&la, step, dense);
                    lp.apply(&mut la, &m).unwrap().commit();
                    let score = ba.values()[0].unwrap();
                    assert_eq!(score, la.lists()[0][0]);
                    if score > best {
                        best = score;
                        best_b = ba.clone();
                        best_l = la.clone();
                    }
                }
                let mut d = ScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
                let mut provider = |_: &FiniteProblem<usize>, _: &Assignment<usize>, step: u64| {
                    vec![basic_move(n, step as usize, dense)]
                };
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let r = d
                    .run_solver_with_extensions(plan(steps as u64), ext)
                    .unwrap();
                assert_eq!(r.work.completed_steps, steps as u64);
                assert_eq!(d.assignment(), &best_b);
                assert_eq!(d.score(), SimpleScore::new(best as i64));
                d.assert_consistent().unwrap();
                let mut d = ListScoreDirector::new(&lp, &ln, lp.initial_assignment()).unwrap();
                let mut provider =
                    |_: &ListProblem<usize>, a: &ListAssignment<usize>, step: u64| {
                        vec![list_move(a, step as usize, dense)]
                    };
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let r = d
                    .run_solver_with_extensions(plan(steps as u64), ext)
                    .unwrap();
                assert_eq!(r.work.completed_steps, steps as u64);
                assert_eq!(d.assignment(), &best_l);
                assert_eq!(d.score(), SimpleScore::new(best as i64));
                d.assert_consistent().unwrap();
                let mut d = MixedScoreDirector::new(&mp, &mn, mp.initial_assignment()).unwrap();
                let mut provider = |_: &MixedProblem<usize, usize>,
                                    a: &MixedAssignment<usize, usize>,
                                    step: u64| {
                    vec![MixedMove {
                        basic: basic_move(n, step as usize, dense),
                        lists: list_move(a.lists(), step as usize, dense),
                    }]
                };
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let c = MixedSolverSequence {
                    phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search: config(steps as u64),
                        ..Default::default()
                    })],
                    ..Default::default()
                };
                let r = d.run_solver_with_extensions(c, ext).unwrap();
                assert_eq!(r.work.completed_steps, steps as u64);
                assert_eq!(d.assignment().basic(), &best_b);
                assert_eq!(d.assignment().lists(), &best_l);
                assert_eq!(d.score(), SimpleScore::new(2 * best as i64));
                d.assert_consistent().unwrap();
            }
        }
    }
}
