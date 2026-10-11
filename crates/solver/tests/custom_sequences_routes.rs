//! Original command sequencing and best-state restoration across all 18 routes.
use std::{fmt::Debug, marker::PhantomData};
use timetuck::*;
trait State: Clone + Debug + PartialEq {
    type Move;
    fn score(&self) -> SimpleScore;
    fn movement(&self, value: i64) -> Self::Move;
}
impl State for Assignment<i64> {
    type Move = Move<i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.values().iter().flatten().sum())
    }
    fn movement(&self, value: i64) -> Self::Move {
        Move::Change {
            variable: 0,
            value: Some(value),
        }
    }
}
impl State for ListAssignment<i64> {
    type Move = ListMove<i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.location(&7).map_or(0, |p| p.entity as i64))
    }
    fn movement(&self, value: i64) -> Self::Move {
        ListMove::Change {
            source: self.location(&7).unwrap(),
            destination: ListPosition {
                entity: value as usize,
                index: 0,
            },
        }
    }
}
impl State for MixedAssignment<i64, i64> {
    type Move = MixedMove<i64, i64>;
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.basic().score().score() + self.lists().score().score())
    }
    fn movement(&self, value: i64) -> Self::Move {
        MixedMove {
            basic: self.basic().movement(value),
            lists: self.lists().movement(value),
        }
    }
}
type Context<'a, A, E> = CustomPhaseContext<'a, A, <A as State>::Move, SimpleScore, E>;
type Command<'a, 'c, A, E> =
    dyn FnMut(&mut Context<'c, A, E>) -> Result<(), CustomPhaseError<E>> + 'a;
fn script<A: State, E>(c: &mut Context<'_, A, E>) -> Result<(), CustomPhaseError<E>> {
    let mut first = |c: &mut Context<'_, A, E>| {
        c.execute(&c.assignment().movement(1))?;
        c.execute(&c.assignment().movement(0))?;
        assert_ne!(c.assignment(), c.best_assignment());
        Ok(())
    };
    let mut second = |c: &mut Context<'_, A, E>| {
        assert_eq!(c.assignment(), c.best_assignment());
        assert_eq!(c.score(), c.best_score());
        c.execute_if(&c.assignment().movement(2), |_, _| true)?;
        c.execute(&c.assignment().movement(0))?;
        Ok(())
    };
    let mut third = |c: &mut Context<'_, A, E>| {
        assert_eq!(c.assignment(), c.best_assignment());
        assert_eq!(c.score(), c.best_score());
        Ok(())
    };
    let commands: [&mut Command<'_, '_, A, E>; 3] = [&mut first, &mut second, &mut third];
    assert_eq!(c.run_commands(commands)?, 3);
    assert_eq!(c.work().evaluations, 7);
    assert_eq!(c.work().move_evaluations, 4);
    assert_eq!(c.work().committed_moves, 4);
    assert_eq!(c.work().restorations, 2);
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
    assert!(r.complete);
    assert_eq!(r.work.evaluations, 7);
    assert_eq!(r.work.move_evaluations, 4);
    assert_eq!(r.work.completed_steps, 0); // Preserve the existing custom summary contract.
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
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
        self.score = a.score();
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        assert_eq!(self.score, a.score());
        self.score = SimpleScore::ZERO;
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = a.score();
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
fn all_eighteen_routes_share_a_phase_and_restore_best_between_commands() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![7_i64],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let eb = Assignment::new(vec![Some(2)]);
    let el = ListAssignment::new(vec![vec![], vec![], vec![7]]);
    let em = MixedAssignment::new(eb.clone(), el.clone());
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(3, 1);
    ln.add_constraint("owner", vec![ListScope::Value(0)], |v| {
        let [ListFact::Value { element, .. }] = v else {
            unreachable!()
        };
        SimpleScore::new(element.as_ref().map_or(0, |v| v.position.entity as i64))
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 3, 1);
    mn.add_constraint(
        "sum",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |v| {
            let [MixedFact::Basic(a), MixedFact::ListValue { element, .. }] = v else {
                unreachable!()
            };
            SimpleScore::new(
                a.copied().unwrap_or(0) + element.as_ref().map_or(0, |v| v.position.entity as i64),
            )
        },
    )
    .unwrap();
    macro_rules! fresh {
        ($solve:ident,$p:expr,$n:expr,$plan:expr,$expected:expr) => {{
            let mut command = |_: usize, c: &mut CustomPhaseContext<'_, _, _, _, _>| script(c);
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
            let mut command = |_: usize, c: &mut CustomPhaseContext<'_, _, _, _, _>| script(c);
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
        ListCalculatorScoreDirector::new(&l, l.initial_assignment(), cache::<Vec<usize>>, false)
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
