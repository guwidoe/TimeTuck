//! Original varied field/domain/root policies, genuine identity and cache errors.
use std::cell::Cell;
use timetuck::*;
#[test]
fn varied_fields_domains_owners_and_roots_match_independent_optimum_with_valid_bound() {
    let p = MixedProblem::new(
        FiniteProblem::new((0..6).map(|i| Variable::new(vec![i + 1, i + 8])).collect()).unwrap(),
        ListProblem::new(vec![40, 50], vec![ListEntity::new(vec![]); 3]).unwrap(),
    );
    let upper = p
        .basic()
        .variables()
        .iter()
        .map(|v| i64::from(*v.domain().iter().max().unwrap()))
        .sum::<i64>()
        + 6;
    let mut n = MixedConstraintNetwork::new(6, 3, 2);
    n.add_constraint(
        "sum and owner",
        (0..6)
            .map(MixedScope::Basic)
            .chain((0..3).map(MixedScope::ListEntity))
            .collect(),
        |facts| {
            SimpleScore::new(
                facts
                    .iter()
                    .enumerate()
                    .map(|(i, f)| match f {
                        MixedFact::Basic(v) => v.copied().map_or(0, i64::from),
                        MixedFact::ListEntity(v) => (i as i64 - 5) * v.len() as i64,
                        _ => 0,
                    })
                    .sum(),
            )
        },
    )
    .unwrap();
    for roots in 1..=3 {
        for kind in [ExactPhaseKind::BruteForce, ExactPhaseKind::BranchAndBound] {
            let plan = ExactMixedPhasePlan {
                basic_groups: vec![vec![0], vec![1, 2], vec![3, 4, 5]],
                entity_order: vec![0, 1, 2],
                value_order: vec![1, 0],
                list_roots: roots,
            };
            let result = run_exact_mixed_phase(
                &p,
                &plan,
                ExactMovePhaseConfig {
                    kind,
                    exploration: None,
                    max_steps: 200,
                },
                |a| n.calculate(&p, a),
                |_, _| Some(SimpleScore::new(upper)),
            )
            .unwrap();
            assert_eq!(result.best.score, SimpleScore::new(upper));
            assert!(p.is_complete(&result.best.assignment));
            assert_eq!(result.best.assignment.lists().lists()[2], vec![50, 40]);
            assert_eq!(
                result.best.assignment.basic().values(),
                &(8..14).map(Some).collect::<Vec<_>>()
            );
            assert_eq!(
                result.working_score,
                n.calculate(&p, &result.working).unwrap()
            );
            assert!(result.basic_steps + result.list_steps <= 200);
            assert_eq!(p.initial_assignment().lists().lists().len(), 3);
            assert!(
                p.initial_assignment()
                    .lists()
                    .lists()
                    .iter()
                    .all(Vec::is_empty)
            );
        }
    }
}
struct Tiny {
    start: Assignment<i32>,
    score_calls: usize,
    visits: Vec<(usize, usize)>,
    rootscores: Vec<i32>,
}
impl ExactPhaseModel for Tiny {
    type Assignment = Assignment<i32>;
    type Move = Move<i32>;
    type Score = SimpleScore;
    type Error = ();
    fn initial(&mut self) -> Result<Self::Assignment, ()> {
        Ok(self.start.clone())
    }
    fn max_depth(&self) -> usize {
        2
    }
    fn moves(&mut self, depth: usize, a: &Self::Assignment) -> Result<Vec<Self::Move>, ()> {
        if depth == 0 {
            Ok(vec![
                Move::Change {
                    variable: 0,
                    value: Some(8),
                },
                Move::Change {
                    variable: 0,
                    value: Some(9),
                },
            ])
        } else {
            assert!(a.values()[0].is_some());
            Ok(vec![Move::Change {
                variable: 1,
                value: Some(10),
            }])
        }
    }
    fn apply(&mut self, a: &mut Self::Assignment, m: &Self::Move) -> Result<(), ()> {
        let Move::Change { variable, value } = m else {
            panic!()
        };
        let mut values = a.values().to_vec();
        values[*variable] = *value;
        *a = Assignment::new(values);
        Ok(())
    }
    fn inverse(&self, before: &Self::Assignment, after: &Self::Assignment) -> Self::Move {
        let i = before
            .values()
            .iter()
            .zip(after.values())
            .position(|(a, b)| a != b)
            .unwrap();
        Move::Change {
            variable: i,
            value: before.values()[i],
        }
    }
    fn score(&mut self, a: &Self::Assignment) -> Result<SimpleScore, ()> {
        self.score_calls += 1;
        Ok(
            SimpleScore::new(a.values().iter().flatten().map(|&v| i64::from(v)).sum())
                .with_structural(-(a.values().iter().filter(|v| v.is_none()).count() as i64)),
        )
    }
    fn expanded(
        &mut self,
        root: usize,
        node: usize,
        _: usize,
        a: &Self::Assignment,
        _: &Self::Score,
    ) -> Result<(), ()> {
        self.visits.push((root, node));
        if node == 0 {
            assert_eq!(a, &self.start);
            self.rootscores.push(root as i32);
        }
        Ok(())
    }
}
#[test]
fn roots_have_distinct_real_arenas_and_cached_score_preimages_are_checked() {
    for roots in 1..=3 {
        for value in [None, Some(7)] {
            let mut m = Tiny {
                start: Assignment::new(vec![None, value]),
                score_calls: 0,
                visits: vec![],
                rootscores: vec![],
            };
            let initial = m.start.clone();
            let score = m.score(&initial).unwrap();
            m.score_calls = 0;
            let result = run_seeded_exact_phase(
                &mut m,
                ExactMovePhaseConfig {
                    kind: ExactPhaseKind::BruteForce,
                    exploration: None,
                    max_steps: 100,
                },
                Some(ExactPhaseScoreCache {
                    assignment: initial,
                    score,
                }),
                roots,
                |_, _| {},
            )
            .unwrap();
            assert_eq!(m.rootscores, (0..roots as i32).collect::<Vec<_>>());
            assert!(m.visits.iter().any(|&(r, n)| r == roots - 1 && n > 0));
            assert_eq!(m.score_calls, 4 * roots);
            assert_eq!(result.steps, 3 * roots);
            assert_eq!(result.best.assignment.values(), &[Some(9), Some(10)]);
        }
    }
    let mut m = Tiny {
        start: Assignment::new(vec![None, None]),
        score_calls: 0,
        visits: vec![],
        rootscores: vec![],
    };
    assert!(matches!(
        run_seeded_exact_phase(&mut m, ExactMovePhaseConfig::default(), None, 0, |_, _| {}),
        Err(ExactPhaseError::InvalidRootCount)
    ));
    assert_eq!(m.score_calls, 0);
    let stale = ExactPhaseScoreCache {
        assignment: Assignment::new(vec![Some(1), None]),
        score: SimpleScore::new(1),
    };
    assert!(matches!(
        run_seeded_exact_phase(
            &mut m,
            ExactMovePhaseConfig::default(),
            Some(stale),
            1,
            |_, _| {}
        ),
        Err(ExactPhaseError::InitialScoreState)
    ));
    assert_eq!(m.score_calls, 0);
}
#[test]
fn plan_errors_and_small_shared_budgets_do_not_publish_unearned_completion() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap(),
        ListProblem::new(vec![4], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let mut plan = ExactMixedPhasePlan {
        basic_groups: vec![vec![0], vec![1]],
        entity_order: vec![0],
        value_order: vec![0],
        list_roots: 2,
    };
    for budget in [0, 1, 2] {
        let result = run_exact_mixed_phase(
            &p,
            &plan,
            ExactMovePhaseConfig {
                kind: ExactPhaseKind::BruteForce,
                exploration: None,
                max_steps: budget,
            },
            |a| {
                Ok::<_, ()>(
                    SimpleScore::new(
                        a.basic()
                            .values()
                            .iter()
                            .flatten()
                            .map(|&v| i64::from(v))
                            .sum(),
                    )
                    .with_structural(
                        -((a.basic().values().iter().filter(|v| v.is_none()).count()
                            + usize::from(a.lists().lists()[0].is_empty()))
                            as i64),
                    ),
                )
            },
            |_, _| None,
        )
        .unwrap();
        assert_eq!(result.basic_steps + result.list_steps, budget);
        assert!(result.best.score.structural() < 0);
    }
    plan.list_roots = 0;
    assert!(matches!(
        run_exact_mixed_phase(
            &p,
            &plan,
            ExactMovePhaseConfig::default(),
            |_| panic!("invalid plan scorer"),
            |_, _: &SimpleScore| None::<SimpleScore>
        ),
        Err(ExactPhaseError::Model(
            ExactMixedModelError::<()>::InvalidPlan
        ))
    ));
}

#[test]
fn changed_calculator_scores_fail_final_validation_instead_of_publishing() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap(),
        ListProblem::new(vec![4], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let plan = ExactMixedPhasePlan {
        basic_groups: vec![vec![0]],
        entity_order: vec![0],
        value_order: vec![0],
        list_roots: 1,
    };
    let calls = Cell::new(0i64);
    let result = run_exact_mixed_phase(
        &p,
        &plan,
        ExactMovePhaseConfig {
            kind: ExactPhaseKind::BruteForce,
            exploration: None,
            max_steps: 10,
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(SimpleScore::new(calls.get()))
        },
        |_, _| None,
    );
    assert!(matches!(
        result,
        Err(ExactPhaseError::Model(
            ExactMixedModelError::ScoreChangedDuringFinalization
        ))
    ));
}
