//! Original checks for the native step-boundary diagnostic option.
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use timetuck::*;
fn tick(counter: &Cell<i64>) -> SimpleScore {
    let n = counter.get();
    counter.set(n + 1);
    SimpleScore::new(n)
}
struct Tick<C> {
    counter: Rc<Cell<i64>>,
    marker: PhantomData<C>,
}
impl<A, C> IncrementalCalculator<A> for Tick<C> {
    type Score = SimpleScore;
    type Change = C;
    type Justification = ();
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        tick(&self.counter)
    }
}
fn plan(step: bool, phase: u8) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![match phase {
            0 => SolverPhase::Construction(ConstructionConfig {
                assert_step_scores: step,
                ..Default::default()
            }),
            1 => SolverPhase::LocalSearch(LocalSearchConfig {
                assert_step_scores: step,
                max_steps: 0,
                ..Default::default()
            }),
            _ => SolverPhase::RuinRecreate(RuinRecreateConfig {
                assert_step_scores: step,
                max_attempts: 0,
                ..Default::default()
            }),
        }],
        ..Default::default()
    }
}
fn mixed_plan(step: bool, phase: u8) -> MixedSolverSequence<'static> {
    let phase = match plan(step, phase).phases.remove(0) {
        SolverPhase::Construction(construction) => {
            MixedSolverPhase::Construction(MixedConstructionConfig {
                construction,
                ..Default::default()
            })
        }
        SolverPhase::LocalSearch(search) => MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search,
            ..Default::default()
        }),
        SolverPhase::RuinRecreate(search) => {
            MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                search,
                ..Default::default()
            })
        }
        _ => unreachable!(),
    };
    MixedSolverSequence {
        phases: vec![phase],
        ..Default::default()
    }
}
#[test]
fn initialization_checks_reach_all_nine_backends_and_provider_construction() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(vec![1i64], vec![ListEntity::new(vec![1])]).unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let counter = Rc::new(Cell::new(0));
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("drift", vec![0], |_| tick(&counter))
        .unwrap();
    let mut ln = ListConstraintNetwork::new(1, 1);
    ln.add_constraint("drift", vec![ListScope::Entity(0)], |_| tick(&counter))
        .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 1, 1);
    mn.add_constraint("drift", vec![MixedScope::Basic(0)], |_| tick(&counter))
        .unwrap();
    let bg = || {
        let mut g = StreamGraphBuilder::new();
        let source = g.source::<usize, (usize, Option<i64>)>();
        g.score("drift", source.port(), |_| tick(&counter)).unwrap();
        Ok((g.build(), source))
    };
    let lg = || {
        let mut g = StreamGraphBuilder::new();
        let source = ListStreamSources::<i64>::new(&mut g);
        g.score("drift", source.entities.port(), |_| tick(&counter))
            .unwrap();
        Ok((g.build(), source))
    };
    let mg = || {
        let mut g = StreamGraphBuilder::new();
        let source = MixedStreamSources::<i64, i64>::new(&mut g);
        g.score("drift", source.basic.port(), |_| tick(&counter))
            .unwrap();
        Ok((g.build(), source))
    };
    macro_rules! check {
        ($solve:ident,$p:expr,$score:expr,$plan:ident,$pty:ty,$aty:ty) => {{
            for phase in 0..3 {
                for provider in [false, true] {
                    if provider && phase != 0 {
                        continue;
                    }
                    for step in [false, true] {
                        counter.set(0);
                        let mut source = |_: &$pty, _: &$aty, _: u64| vec![];
                        let mut ext = SolverExtensions::default();
                        if provider {
                            ext.move_providers.bind(0, &mut source);
                        }
                        let result = $solve(&$p, $score, $plan(step, phase), ext);
                        if step {
                            assert!(
                                result.is_err(),
                                "{} phase{phase} provider{provider}",
                                stringify!($solve)
                            );
                            assert!(
                                format!("{:?}", result.unwrap_err())
                                    .to_lowercase()
                                    .contains("corrupt")
                            );
                        } else {
                            assert!(
                                result.is_ok(),
                                "{} phase{phase} provider{provider}: {result:?}",
                                stringify!($solve)
                            );
                        }
                    }
                }
            }
        }};
    }
    check!(
        solve_with_extensions,
        b,
        &bn,
        plan,
        FiniteProblem<i64>,
        Assignment<i64>
    );
    check!(
        solve_stream_with_extensions,
        b,
        bg,
        plan,
        FiniteProblem<i64>,
        Assignment<i64>
    );
    check!(
        solve_calculator_with_extensions,
        b,
        &|| Tick::<Vec<usize>> {
            counter: counter.clone(),
            marker: PhantomData
        },
        plan,
        FiniteProblem<i64>,
        Assignment<i64>
    );
    check!(
        solve_list_with_extensions,
        l,
        &ln,
        plan,
        ListProblem<i64>,
        ListAssignment<i64>
    );
    check!(
        solve_list_stream_with_extensions,
        l,
        lg,
        plan,
        ListProblem<i64>,
        ListAssignment<i64>
    );
    check!(
        solve_list_calculator_with_extensions,
        l,
        &|| Tick::<Vec<usize>> {
            counter: counter.clone(),
            marker: PhantomData
        },
        plan,
        ListProblem<i64>,
        ListAssignment<i64>
    );
    check!(solve_mixed_with_extensions,m,&mn,mixed_plan,MixedProblem<i64,i64>,MixedAssignment<i64,i64>);
    check!(solve_mixed_stream_with_extensions,m,mg,mixed_plan,MixedProblem<i64,i64>,MixedAssignment<i64,i64>);
    check!(solve_mixed_calculator_with_extensions,m,&||Tick::<MixedCalculatorChange>{counter:counter.clone(),marker:PhantomData},mixed_plan,MixedProblem<i64,i64>,MixedAssignment<i64,i64>);
}
trait Value {
    type Change;
    fn value(&self) -> i64;
}
impl Value for Assignment<i64> {
    type Change = Vec<usize>;
    fn value(&self) -> i64 {
        self.values().iter().flatten().sum()
    }
}
impl Value for ListAssignment<i64> {
    type Change = Vec<usize>;
    fn value(&self) -> i64 {
        self.lists()
            .iter()
            .enumerate()
            .map(|(i, r)| i as i64 * r.iter().sum::<i64>())
            .sum()
    }
}
impl Value for MixedAssignment<i64, i64> {
    type Change = MixedCalculatorChange;
    fn value(&self) -> i64 {
        self.basic().value() + self.lists().value()
    }
}
struct CommitFault<A> {
    value: i64,
    changes: usize,
    broken: bool,
    marker: PhantomData<A>,
}
impl<A: Value> IncrementalCalculator<A> for CommitFault<A> {
    type Score = SimpleScore;
    type Change = A::Change;
    type Justification = ();
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.value = a.value();
        self.changes = 0;
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &A::Change,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.changes += 1;
        self.value = a.value() + i64::from(self.broken && self.changes == 3);
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.value)
    }
}
fn relocate() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
#[test]
fn boundary_checks_detect_commit_corruption_with_less_diagnostic_work() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![1i64],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    macro_rules! check {
        ($solve:ident,$p:expr,$a:ty,$pty:ty,$movement:expr,$mixed:expr) => {{
            let mut counts = vec![];
            for (step, full) in [(false, false), (true, false), (false, true), (true, true)] {
                for broken in [false, true] {
                    let builds = Cell::new(0);
                    let factory = || {
                        builds.set(builds.get() + 1);
                        CommitFault::<$a> {
                            value: 0,
                            changes: 0,
                            broken,
                            marker: PhantomData,
                        }
                    };
                    let mut provider = |_: &$pty, _: &$a, _: u64| vec![$movement];
                    let config = LocalSearchConfig {
                        assert_step_scores: step,
                        assert_scores: full,
                        max_steps: 1,
                        max_evaluations: 2,
                        ..Default::default()
                    };
                    let result = $solve(
                        &$p,
                        $p.initial_assignment(),
                        factory,
                        $mixed(config),
                        &mut provider,
                    );
                    if broken && (step || full) {
                        assert!(
                            format!("{:?}", result.unwrap_err())
                                .to_lowercase()
                                .contains("corrupt")
                        );
                    } else {
                        let r = result.unwrap();
                        assert_eq!(r.evaluations, 2);
                        assert_eq!(r.constraint_evaluations, 2);
                        assert_eq!(r.accepted_moves, 1);
                        assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
                        if !broken {
                            counts.push(builds.get());
                        }
                    }
                }
            }
            assert_eq!(counts[0], 1);
            assert_eq!(counts[1], 3);
            assert!(counts[2] > counts[1]);
            assert_eq!(counts[3], counts[2]);
        }};
    }
    check!(
        calculator_local_search_with_provider,
        b,
        Assignment<i64>,
        FiniteProblem<i64>,
        Move::Change {
            variable: 0,
            value: Some(1)
        },
        |c| c
    );
    check!(
        list_calculator_local_search_with_provider,
        l,
        ListAssignment<i64>,
        ListProblem<i64>,
        relocate(),
        |c| c
    );
    check!(mixed_calculator_local_search_with_provider,m,MixedAssignment<i64,i64>,MixedProblem<i64,i64>,MixedMove{basic:Move::Composite(vec![]),lists:relocate()},|search|MixedLocalSearchConfig{search,..Default::default()});
}
#[test]
fn construction_checks_completed_placements_and_preserves_provider_settings() {
    let b = FiniteProblem::new(vec![Variable::new(vec![1i64])]).unwrap();
    let l = ListProblem::new(
        vec![1i64],
        vec![
            ListEntity::new(vec![]).with_pinned(true),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mixed_basic = MixedProblem::new(
        b.clone(),
        ListProblem::new(vec![], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let mixed_list = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0i64]).with_initial(0).with_pinned(true),
        ])
        .unwrap(),
        l.clone(),
    );
    macro_rules! check {
        ($solve:ident,$p:expr,$a:ty,$pty:ty,$movement:expr,$plan:expr) => {{
            for provided in [false, true] {
                let mut counts = vec![];
                for (step, full) in [(false, false), (true, false), (false, true), (true, true)] {
                    for broken in [false, true] {
                        let builds = Cell::new(0);
                        let factory = || {
                            builds.set(builds.get() + 1);
                            CommitFault::<$a> {
                                value: 0,
                                changes: 0,
                                broken,
                                marker: PhantomData,
                            }
                        };
                        let mut provider = |_: &$pty, _: &$a, _: u64| vec![$movement];
                        let mut ext = SolverExtensions::default();
                        if provided {
                            ext.move_providers.bind(0, &mut provider);
                        }
                        let c = ConstructionConfig {
                            assert_step_scores: step,
                            assert_scores: full,
                            ..Default::default()
                        };
                        let result = $solve(&$p, &factory, $plan(c), ext);
                        if broken && (step || full) {
                            assert!(
                                format!("{:?}", result.unwrap_err())
                                    .to_lowercase()
                                    .contains("corrupt")
                            );
                        } else {
                            let r = result.unwrap();
                            assert!(r.complete);
                            assert_eq!(r.score, Some(SimpleScore::new(1)));
                            assert_eq!(r.work.evaluations, 2);
                            assert_eq!(r.work.construction_steps, 1);
                            if !broken {
                                counts.push(builds.get());
                            }
                        }
                    }
                }
                assert_eq!(counts[0], 1);
                assert_eq!(counts[1], 3);
                assert!(counts[2] > counts[1]);
                assert_eq!(counts[3], counts[2]);
            }
        }};
    }
    let basic_plan = |c| SolverSequence {
        phases: vec![SolverPhase::Construction(c)],
        ..Default::default()
    };
    let mixed_plan = |construction| MixedSolverSequence {
        phases: vec![MixedSolverPhase::Construction(MixedConstructionConfig {
            construction,
            ..Default::default()
        })],
        ..Default::default()
    };
    let assign = ListMove::Assign {
        value: 1,
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    check!(
        solve_calculator_with_extensions,
        b,
        Assignment<i64>,
        FiniteProblem<i64>,
        Move::Change {
            variable: 0,
            value: Some(1)
        },
        basic_plan
    );
    check!(
        solve_list_calculator_with_extensions,
        l,
        ListAssignment<i64>,
        ListProblem<i64>,
        assign.clone(),
        basic_plan
    );
    check!(solve_mixed_calculator_with_extensions,mixed_basic,MixedAssignment<i64,i64>,MixedProblem<i64,i64>,MixedMove{basic:Move::Change{variable:0,value:Some(1)},lists:ListMove::Composite(vec![])},mixed_plan);
    check!(solve_mixed_calculator_with_extensions,mixed_list,MixedAssignment<i64,i64>,MixedProblem<i64,i64>,MixedMove{basic:Move::Composite(vec![]),lists:assign.clone()},mixed_plan);
}
