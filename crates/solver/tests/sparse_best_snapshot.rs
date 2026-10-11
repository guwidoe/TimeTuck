//! Original sparse best-cache regressions, without upstream method credit.
#[path = "support/owned_score.rs"]
mod owned;
use owned::{OwnedScore, fail_clone_at};
use timetuck::*;
fn strategies() -> Vec<ExactSearchStrategy> {
    let mut out = vec![
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
    ];
    out.extend(
        [
            NodeExploration::DepthFirst,
            NodeExploration::BreadthFirst,
            NodeExploration::ScoreFirst,
            NodeExploration::OptimisticBoundFirst,
        ]
        .map(|exploration| {
            ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                exploration,
                ..Default::default()
            })
        }),
    );
    out
}
fn config(strategy: ExactSearchStrategy, cap: u64) -> SolverSequence<'static, OwnedScore> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            ..Default::default()
        })],
        limits: SolverLimits {
            max_evaluations: cap,
            ..Default::default()
        },
    }
}
#[test]
fn basic_best_cache_includes_changes_across_nonimproving_candidates() {
    fail_clone_at(0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]),
        Variable::new(vec![0, -100, 10]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    // Keep each transition sparse relative to the complete contribution cache.
    for i in 0..8 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![i], |v| {
            OwnedScore::new(0, v[0].copied().unwrap_or(0))
        })
        .unwrap();
    }
    for strategy in strategies() {
        for cap in [0, 1, 2, 4, 5, 6, 100] {
            let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let r = d.run_solver(config(strategy.clone(), cap)).unwrap();
            let expected = solve(&p, &n, config(strategy.clone(), cap)).unwrap();
            assert_eq!(r.assignment, expected.assignment);
            assert_eq!(r.score, expected.score);
            assert_eq!(r.termination, expected.termination);
            assert_eq!(r.work.evaluations, expected.work.evaluations);
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn list_and_mixed_best_caches_match_every_interrupted_prefix() {
    let p = ListProblem::new(vec![1i64, 10], vec![ListEntity::new(vec![]); 2]).unwrap();
    let mut n = ListConstraintNetwork::new(2, 2);
    // Keep each transition sparse relative to the complete contribution cache.
    for i in 0..8 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![ListScope::Entity(i)], move |f| {
            let ListFact::Entity(v) = f[0] else {
                unreachable!()
            };
            OwnedScore::new(
                0,
                v.iter()
                    .enumerate()
                    .map(|(j, v)| (i as i64 + 1) * (j as i64 + 1) * v)
                    .sum(),
            )
        })
        .unwrap();
    }
    let mp = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0i64, -100, 1])]).unwrap(),
        p.clone(),
    );
    let mut mn = MixedConstraintNetwork::new(1, 2, 2);
    // Keep each transition sparse relative to the complete contribution cache.
    for i in 0..8 {
        mn.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    mn.add_constraint("basic", vec![MixedScope::Basic(0)], |f| {
        let MixedFact::Basic(v) = f[0] else {
            unreachable!()
        };
        OwnedScore::new(0, v.copied().unwrap_or(0))
    })
    .unwrap();
    for i in 0..2 {
        mn.add_constraint(i.to_string(), vec![MixedScope::ListEntity(i)], move |f| {
            let MixedFact::ListEntity(v) = f[0] else {
                unreachable!()
            };
            OwnedScore::new(
                0,
                v.iter()
                    .enumerate()
                    .map(|(j, v)| (i as i64 + 1) * (j as i64 + 1) * v)
                    .sum(),
            )
        })
        .unwrap();
    }
    for strategy in strategies() {
        for cap in 0..=28 {
            let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let r = d.run_solver(config(strategy.clone(), cap)).unwrap();
            let expected = solve_list(&p, &n, config(strategy.clone(), cap)).unwrap();
            assert_eq!(
                (r.assignment, r.score, r.termination),
                (expected.assignment, expected.score, expected.termination)
            );
            d.assert_consistent().unwrap();
            let c = MixedSolverSequence {
                limits: config(strategy.clone(), cap).limits,
                phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig {
                    strategy: strategy.clone(),
                    ..Default::default()
                })],
            };
            let expected = solve_mixed(&mp, &mn, c.clone()).unwrap();
            let mut d = MixedScoreDirector::new(&mp, &mn, mp.initial_assignment()).unwrap();
            let r = d.run_solver(c).unwrap();
            assert_eq!(
                (r.assignment, r.score, r.termination),
                (expected.assignment, expected.score, expected.termination)
            );
            d.assert_consistent().unwrap();
        }
    }
}

#[test]
fn a_prior_phase_best_survives_exact_prefixes_without_improvement() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(1),
        Variable::new(vec![0, -100, 10]).with_initial(10),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    // Keep each transition sparse relative to the complete contribution cache.
    for i in 0..8 {
        n.add_constraint(format!("constant{i}"), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    for i in 0..2 {
        n.add_constraint(i.to_string(), vec![i], |v| {
            OwnedScore::new(0, v[0].copied().unwrap_or(0))
        })
        .unwrap();
    }
    for strategy in strategies() {
        for cap in 1..=8 {
            let mut c = config(strategy.clone(), cap);
            c.phases.insert(
                0,
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }),
            );
            let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let r = d.run_solver(c).unwrap();
            assert_eq!(r.score, Some(OwnedScore::new(0, 11)));
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), OwnedScore::new(0, 11));
            d.assert_consistent().unwrap();
        }
    }
}

#[path = "support/snapshot_score.rs"]
mod counted;
#[test]
fn owned_score_cloning_does_not_scale_as_cache_size_times_best_updates() {
    use counted::OwnedScore;
    let p = FiniteProblem::new(vec![Variable::new((0..128i64).collect())]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("objective", vec![0], |v| {
        OwnedScore::new(0, v[0].copied().unwrap_or(0))
    })
    .unwrap();
    for i in 0..512 {
        n.add_constraint(i.to_string(), vec![], |_| OwnedScore::new(0, 0))
            .unwrap();
    }
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    counted::reset_clones();
    let r = d
        .run_solver(SolverSequence {
            phases: vec![SolverPhase::Exact(Default::default())],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(r.score, Some(OwnedScore::new(0, 127)));
    assert_eq!(r.work.evaluations, 128);
    // A full contribution snapshot at each improvement alone clones 65,664
    // scores. This bound includes every score clone in the entire retained solve.
    assert!(
        counted::clones() < 16_000,
        "{} score clones",
        counted::clones()
    );
    d.assert_consistent().unwrap();
}
