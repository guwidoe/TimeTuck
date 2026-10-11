//! Native stream retained-winner state and payload regressions; no source-method credit.
use std::fmt;
use timetuck::*;

#[derive(Default)]
struct Control;
#[derive(Clone)]
struct Value {
    id: usize,
    payload: i64,
}
impl Value {
    fn new(id: usize, payload: i64, _: &Control) -> Self {
        Self { id, payload }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.id, self.payload)
    }
}
fn change(slot: usize, v: Value) -> Move<Value> {
    Move::Change {
        variable: slot,
        value: Some(v),
    }
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn config(ties: TieBreaking) -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 1,
        selection: MoveSelection::BestAccepted,
        acceptance: Acceptance::HillClimbing,
        tie_breaking: ties,
        assert_scores: true,
        ..Default::default()
    }
}
fn plan(c: LocalSearchConfig<'static>) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::LocalSearch(c)],
        ..Default::default()
    }
}
fn mixed_plan(c: LocalSearchConfig<'static>) -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: c,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn basic(n: usize, c: &Control) -> FiniteProblem<Value> {
    FiniteProblem::new(
        (0..n)
            .map(|i| {
                Variable::new(vec![Value::new(0, 0, c), Value::new(1, 1, c)])
                    .with_initial(Value::new(0, 100 + i as i64, c))
            })
            .collect(),
    )
    .unwrap()
}
fn lists(n: usize, c: &Control) -> ListProblem<Value> {
    ListProblem::new(
        (0..n).map(|i| Value::new(i, 0, c)).collect(),
        (0..n)
            .map(|i| ListEntity::new(vec![Value::new(i, 100 + i as i64, c)]))
            .collect(),
    )
    .unwrap()
}
fn basic_director(
    p: &FiniteProblem<Value>,
) -> StreamScoreDirector<'_, 'static, Value, SimpleScore> {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<Value>)>();
    b.score("basic", source.port(), |(i, v)| {
        SimpleScore::new(if *i == 0 {
            v.as_ref().unwrap().payload
        } else {
            0
        })
    })
    .unwrap();
    StreamScoreDirector::new(p, b.build(), source, p.initial_assignment()).unwrap()
}
fn list_director(
    p: &ListProblem<Value>,
) -> ListStreamScoreDirector<'_, 'static, Value, SimpleScore> {
    let mut b = StreamGraphBuilder::new();
    let source = ListStreamSources::<Value>::new(&mut b);
    b.score("list", source.entities.port(), |(i, v)| {
        SimpleScore::new(if *i == 0 { v[0].payload } else { 0 })
    })
    .unwrap();
    ListStreamScoreDirector::new(p, b.build(), source, p.initial_assignment()).unwrap()
}
fn mixed_director(
    p: &MixedProblem<Value, Value>,
) -> MixedStreamScoreDirector<'_, 'static, Value, Value, SimpleScore> {
    let mut b = StreamGraphBuilder::new();
    let source = MixedStreamSources::<Value, Value>::new(&mut b);
    b.score("basic", source.basic.port(), |(i, v)| {
        SimpleScore::new(if *i == 0 {
            v.as_ref().unwrap().payload
        } else {
            0
        })
    })
    .unwrap();
    b.score("list", source.lists.entities.port(), |(i, v)| {
        SimpleScore::new(if *i == 0 { v[0].payload } else { 0 })
    })
    .unwrap();
    MixedStreamScoreDirector::new(p, b.build(), source, p.initial_assignment()).unwrap()
}
macro_rules! verify {
    ($director:expr,$plan:expr,$config:expr,$moves:expr,$p:ty,$a:ty) => {{
        let mut d = $director;
        let moves = $moves;
        let mut reference = MoveForager::new(ForagerConfig {
            current_score: d.score(),
            best_score: d.score(),
            tie_breaking: $config.tie_breaking,
            selection: $config.selection,
            ..Default::default()
        });
        let mut evaluated = 0;
        for movement in &moves {
            let trial = d.apply(movement).unwrap();
            evaluated += 1;
            reference.add(
                trial.score(),
                true,
                (format!("{:?}", trial.assignment()), trial.score()),
            );
            if reference.is_quit_early() {
                break;
            }
        }
        let expected = reference.pick().unwrap();
        let mut provider = |_: &$p, _: &$a, _: u64| moves.clone();
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut provider);
        let result = d.run_solver_with_extensions($plan, ext).unwrap();
        assert_eq!(result.work.completed_steps, 1);
        assert_eq!(result.work.evaluations, evaluated + 1);
        assert_eq!(format!("{:?}", d.assignment()), expected.0);
        assert_eq!(d.score(), expected.1);
        d.assert_consistent().unwrap();
    }};
}
#[test]
fn repeated_and_noncanonical_payloads_match_full_snapshot_reference() {
    for n in [8, 64, 257] {
        for cfg in [
            config(TieBreaking::First),
            config(TieBreaking::Random { seed: 18 }),
            LocalSearchConfig {
                selection: MoveSelection::FirstAccepted,
                ..config(TieBreaking::First)
            },
        ] {
            let c = Control;
            let p = basic(n, &c);
            let lp = lists(n, &c);
            let mp = MixedProblem::new(p.clone(), lp.clone());
            let bm: Vec<_> = [200, 250, 250]
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    Move::Composite(vec![
                        change(0, Value::new(1, -3, &c)),
                        change(0, Value::new(1, v, &c)),
                        change(1, Value::new(1, i as i64, &c)),
                    ])
                })
                .collect();
            let lm: Vec<_> = [200, 250, 250]
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    ListMove::Composite(vec![
                        ListMove::Unassign {
                            entity: 0,
                            start: 0,
                            end: 1,
                        },
                        ListMove::Assign {
                            value: Value::new(0, v, &c),
                            destination: pos(0, 0),
                        },
                        ListMove::Change {
                            source: pos(1, 0),
                            destination: pos(i + 2, 1),
                        },
                    ])
                })
                .collect();
            let mm: Vec<_> = bm
                .iter()
                .cloned()
                .zip(lm.iter().cloned())
                .map(|(basic, lists)| MixedMove { basic, lists })
                .collect();
            verify!(
                basic_director(&p),
                plan(cfg.clone()),
                cfg,
                bm,
                FiniteProblem<Value>,
                Assignment<Value>
            );
            verify!(
                list_director(&lp),
                plan(cfg.clone()),
                cfg,
                lm,
                ListProblem<Value>,
                ListAssignment<Value>
            );
            verify!(mixed_director(&mp),mixed_plan(cfg.clone()),cfg,mm,MixedProblem<Value,Value>,MixedAssignment<Value,Value>);
        }
    }
}

#[test]
fn dense_and_empty_components_match_full_copy_reference() {
    let c = Control;
    let p = basic(128, &c);
    let lp = lists(128, &c);
    let mp = MixedProblem::new(p.clone(), lp.clone());
    let dense_basic = Move::Composite(
        (0..128)
            .map(|i| change(i, Value::new(1, 300 + i as i64, &c)))
            .collect(),
    );
    let dense_list = ListMove::Composite(
        (0..128)
            .step_by(2)
            .map(|i| ListMove::Swap {
                left: pos(i, 0),
                right: pos(i + 1, 0),
            })
            .collect(),
    );
    let none_basic = Move::Composite(vec![]);
    let none_list = ListMove::Composite(vec![]);
    for movement in [
        MixedMove {
            basic: dense_basic.clone(),
            lists: none_list.clone(),
        },
        MixedMove {
            basic: none_basic.clone(),
            lists: dense_list.clone(),
        },
        MixedMove {
            basic: dense_basic,
            lists: dense_list,
        },
        MixedMove {
            basic: none_basic,
            lists: none_list,
        },
    ] {
        let cfg = config(TieBreaking::Random { seed: 34 });
        verify!(mixed_director(&mp),mixed_plan(cfg.clone()),cfg,vec![movement.clone(),movement],MixedProblem<Value,Value>,MixedAssignment<Value,Value>);
    }
}
