//! Original structural routing/rescoring across fresh and retained native routes.
use std::{fmt::Debug, marker::PhantomData};
use timetuck::*;
trait State: Clone + Debug + PartialEq {
    type Move;
    fn score(&self) -> SimpleScore;
    fn movement(&self, flawed: bool) -> Self::Move;
}
impl State for Assignment<i64> {
    type Move = Move<i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.values().iter().flatten().sum())
            .with_structural(-(self.values().iter().filter(|v| v.is_none()).count() as i64))
    }
    fn movement(&self, flawed: bool) -> Self::Move {
        Move::Change {
            variable: 0,
            value: if flawed { None } else { Some(1) },
        }
    }
}
impl State for ListAssignment<i64> {
    type Move = ListMove<i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.location(&7).map_or(0, |p| p.entity as i64))
            .with_structural(-([7, 8].iter().filter(|v| self.location(v).is_none()).count() as i64))
    }
    fn movement(&self, flawed: bool) -> Self::Move {
        if let Some(source) = self.location(&7) {
            ListMove::Change {
                source,
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            }
        } else {
            assert!(flawed);
            ListMove::Assign {
                value: 7,
                destination: ListPosition {
                    entity: 1,
                    index: 0,
                },
            }
        }
    }
}
impl State for MixedAssignment<i64, i64> {
    type Move = MixedMove<i64, i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.basic().score().score() + self.lists().score().score())
            .with_structural(self.basic().score().structural() + self.lists().score().structural())
    }
    fn movement(&self, flawed: bool) -> Self::Move {
        MixedMove {
            basic: self.basic().movement(flawed),
            lists: self.lists().movement(flawed),
        }
    }
}
fn script<A: State, E>(
    c: &mut CustomPhaseContext<'_, A, A::Move, SimpleScore, E>,
    flawed: bool,
) -> Result<(), CustomPhaseError<E>> {
    let before = c.assignment().clone();
    let score = c.score();
    let movement = before.movement(flawed);
    let mut valid = 0;
    let mut bad = 0;
    assert_eq!(
        c.evaluate_structured(
            &movement,
            |a| {
                valid += 1;
                assert_ne!(a, &before);
                assert!(a.score().structural() >= 0);
                false
            },
            |a| {
                bad += 1;
                assert_ne!(a, &before);
                assert!(a.score().structural() < 0);
                true
            }
        )?,
        Some(flawed)
    );
    assert_eq!((valid, bad), (usize::from(!flawed), usize::from(flawed)));
    assert_eq!(c.assignment(), &before);
    assert_eq!(c.score(), score);
    let mut full = 0;
    assert_eq!(
        c.evaluate_structured_with_score(
            &movement,
            |a| {
                full += 1;
                Ok((a.score(), 1))
            },
            |a, s| {
                valid += 1;
                assert_eq!(s, a.score());
                assert!(s.structural() >= 0);
                false
            },
            |a, s| {
                bad += 1;
                assert_eq!(s, a.score());
                assert!(s.structural() < 0);
                true
            }
        )?,
        Some(flawed)
    );
    assert_eq!(full, 1);
    assert_eq!(
        (valid, bad),
        (2 * usize::from(!flawed), 2 * usize::from(flawed))
    );
    assert_eq!(c.assignment(), &before);
    assert_eq!(c.score(), score);
    assert_eq!(c.best_assignment(), &before);
    assert_eq!(c.best_score(), score);
    assert_eq!(c.work().evaluations, 4);
    assert_eq!(c.work().move_evaluations, 2);
    assert_eq!(c.work().committed_moves, 0);
    Ok(())
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Custom(Default::default())],
        ..Default::default()
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::Custom(Default::default())],
        ..Default::default()
    }
}
fn check<A: State>(r: SolverResult<A, SimpleScore>, expected: &A) {
    assert_eq!(&r.assignment, expected);
    assert_eq!(r.score, Some(expected.score()));
    assert_eq!(r.complete, expected.score().structural() == 0);
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.work.completed_steps, 0); // Preserve the existing custom summary contract.
    assert_eq!(
        r.termination,
        if r.complete {
            SolverTermination::PlanCompleted
        } else {
            SolverTermination::IncompleteConstruction
        }
    );
}
#[derive(Clone)]
struct Cache<C> {
    score: SimpleScore,
    kind: PhantomData<C>,
}
fn cache<C>() -> Cache<C> {
    Cache {
        score: SimpleScore::ZERO,
        kind: PhantomData,
    }
}
impl<A: State, C> IncrementalCalculator<A> for Cache<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.score().with_structural(0);
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        assert_eq!(self.score, a.score().with_structural(0));
        self.score = SimpleScore::ZERO;
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.score().with_structural(0);
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.score
    }
}
fn basic_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i64>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    b.score("value", s.port(), |(_, v)| SimpleScore::new(v.unwrap_or(0)))
        .unwrap();
    (b.build(), s)
}
fn list_graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i64>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<i64>::new(&mut b);
    b.score("owner", s.values.port(), |v| {
        SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
    })
    .unwrap();
    (b.build(), s)
}
fn mixed_graph() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i64, i64>,
) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<i64, i64>::new(&mut b);
    b.score("value", s.basic.port(), |(_, v)| {
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    b.score("owner", s.lists.values.port(), |v| {
        SimpleScore::new(v.position.map_or(0, |p| p.entity as i64))
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn fresh_and_retained_routes_dispatch_valid_and_flawed_trials_without_replay() {
    for flawed in [false, true] {
        let b = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap();
        let l = ListProblem::new(
            vec![7_i64, 8],
            vec![
                ListEntity::new(if flawed { vec![] } else { vec![7, 8] }),
                ListEntity::new(vec![]),
                ListEntity::new(vec![]),
            ],
        )
        .unwrap();
        let m = MixedProblem::new(b.clone(), l.clone());
        let eb = b.initial_assignment();
        let el = l.initial_assignment();
        let em = MixedAssignment::new(eb.clone(), el.clone());
        let mut bn = ConstraintNetwork::new(1);
        bn.add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
        let mut ln = ListConstraintNetwork::new(3, 2);
        ln.add_constraint("owner", vec![ListScope::Value(0)], |v| {
            let [ListFact::Value { element, .. }] = v else {
                unreachable!()
            };
            SimpleScore::new(element.as_ref().map_or(0, |v| v.position.entity as i64))
        })
        .unwrap();
        let mut mn = MixedConstraintNetwork::new(1, 3, 2);
        mn.add_constraint(
            "sum",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                let [MixedFact::Basic(a), MixedFact::ListValue { element, .. }] = v else {
                    unreachable!()
                };
                SimpleScore::new(
                    a.copied().unwrap_or(0)
                        + element.as_ref().map_or(0, |v| v.position.entity as i64),
                )
            },
        )
        .unwrap();
        macro_rules! fresh {
            ($solve:ident,$p:expr,$n:expr,$plan:expr,$expected:expr) => {{
                let mut command =
                    |_: usize, c: &mut CustomPhaseContext<'_, _, _, _, _>| script(c, flawed);
                let r = $solve(
                    $p,
                    $n,
                    $plan,
                    SolverExtensions {
                        command: Some(&mut command),
                        ..Default::default()
                    },
                )
                .unwrap();
                check(r, $expected);
            }};
        }
        fresh!(solve_with_extensions, &b, &bn, plan(), &eb);
        fresh!(solve_list_with_extensions, &l, &ln, plan(), &el);
        fresh!(solve_mixed_with_extensions, &m, &mn, mixed_plan(), &em);
        fresh!(
            solve_stream_with_extensions,
            &b,
            || Ok(basic_graph()),
            plan(),
            &eb
        );
        fresh!(
            solve_list_stream_with_extensions,
            &l,
            || Ok(list_graph()),
            plan(),
            &el
        );
        fresh!(
            solve_mixed_stream_with_extensions,
            &m,
            || Ok(mixed_graph()),
            mixed_plan(),
            &em
        );
        fresh!(
            solve_calculator_with_extensions,
            &b,
            &cache::<Vec<usize>>,
            plan(),
            &eb
        );
        fresh!(
            solve_list_calculator_with_extensions,
            &l,
            &cache::<Vec<usize>>,
            plan(),
            &el
        );
        fresh!(
            solve_mixed_calculator_with_extensions,
            &m,
            &cache::<MixedCalculatorChange>,
            mixed_plan(),
            &em
        );
        macro_rules! retained {
            ($d:expr,$plan:expr,$expected:expr) => {{
                let mut d = $d;
                let mut command =
                    |_: usize, c: &mut CustomPhaseContext<'_, _, _, _, _>| script(c, flawed);
                let r = d
                    .run_solver_with_extensions(
                        $plan,
                        SolverExtensions {
                            command: Some(&mut command),
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(d.assignment(), &r.assignment);
                d.assert_consistent().unwrap();
                check(r, $expected);
            }};
        }
        retained!(
            ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap(),
            plan(),
            &eb
        );
        retained!(
            ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
            plan(),
            &el
        );
        retained!(
            MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
            mixed_plan(),
            &em
        );
        let (g, s) = basic_graph();
        retained!(
            StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap(),
            plan(),
            &eb
        );
        let (g, s) = list_graph();
        retained!(
            ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap(),
            plan(),
            &el
        );
        let (g, s) = mixed_graph();
        retained!(
            MixedStreamScoreDirector::new(&m, g, s, m.initial_assignment()).unwrap(),
            mixed_plan(),
            &em
        );
        retained!(
            CalculatorScoreDirector::new(&b, b.initial_assignment(), cache::<Vec<usize>>, false)
                .unwrap(),
            plan(),
            &eb
        );
        retained!(
            ListCalculatorScoreDirector::new(
                &l,
                l.initial_assignment(),
                cache::<Vec<usize>>,
                false
            )
            .unwrap(),
            plan(),
            &el
        );
        retained!(
            MixedCalculatorScoreDirector::new(
                &m,
                m.initial_assignment(),
                cache::<MixedCalculatorChange>,
                false
            )
            .unwrap(),
            mixed_plan(),
            &em
        );
    }
}
