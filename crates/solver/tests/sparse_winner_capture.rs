//! Native retained-winner state and payload regressions; no source-method credit.
use std::{cell::Cell, fmt, rc::Rc};
use timetuck::*;

#[derive(Clone)]
struct Control {
    armed: Rc<Cell<bool>>,
    remaining: Rc<Cell<Option<usize>>>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            armed: Rc::new(Cell::new(false)),
            remaining: Rc::new(Cell::new(None)),
        }
    }
}
struct Value {
    id: usize,
    payload: i64,
    control: Control,
}
impl Value {
    fn new(id: usize, payload: i64, c: &Control) -> Self {
        Self {
            id,
            payload,
            control: c.clone(),
        }
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        if let Some(left) = self.control.remaining.get() {
            assert_ne!(left, 0, "payload clone panic");
            self.control.remaining.set(Some(left - 1));
        }
        Self::new(self.id, self.payload, &self.control)
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
macro_rules! verify {
    ($director:expr,$plan:expr,$config:expr,$moves:expr,$p:ty,$a:ty) => {{
        let mut d = $director;
        let moves = $moves;
        let mut reference = MoveForager::new(ForagerConfig {
            current_score: d.score(),
            best_score: d.score(),
            tie_breaking: $config.tie_breaking,
            ..Default::default()
        });
        for movement in &moves {
            let trial = d.apply(movement).unwrap();
            reference.add(
                trial.score(),
                true,
                (format!("{:?}", trial.assignment()), trial.score()),
            );
        }
        let expected = reference.pick().unwrap();
        let mut provider = |_: &$p, _: &$a, _: u64| moves.clone();
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut provider);
        let result = d.run_solver_with_extensions($plan, ext).unwrap();
        assert_eq!(result.work.completed_steps, 1);
        assert_eq!(format!("{:?}", d.assignment()), expected.0);
        assert_eq!(d.score(), expected.1);
        d.assert_consistent().unwrap();
    }};
}
#[test]
fn repeated_and_noncanonical_payloads_match_full_snapshot_reference() {
    for n in [8, 64, 257] {
        for ties in [TieBreaking::First, TieBreaking::Random { seed: 18 }] {
            let c = Control::default();
            let p = basic(n, &c);
            let lp = lists(n, &c);
            let mp = MixedProblem::new(p.clone(), lp.clone());
            let mut bn = ConstraintNetwork::<Value, SimpleScore>::new(n);
            bn.add_constraint("payload", vec![0], |f| {
                SimpleScore::new(f[0].unwrap().payload)
            })
            .unwrap();
            let mut ln = ListConstraintNetwork::<Value, SimpleScore>::new(n, n);
            ln.add_constraint("payload", vec![ListScope::Entity(0)], |f| {
                let ListFact::Entity(row) = &f[0] else {
                    unreachable!()
                };
                SimpleScore::new(row[0].payload)
            })
            .unwrap();
            let mut mn = MixedConstraintNetwork::<Value, Value, SimpleScore>::new(n, n, n);
            mn.add_constraint("payload", vec![MixedScope::Basic(0)], |f| {
                let MixedFact::Basic(v) = &f[0] else {
                    unreachable!()
                };
                SimpleScore::new(v.unwrap().payload)
            })
            .unwrap();
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
                ScoreDirector::new(&p, &bn, p.initial_assignment()).unwrap(),
                plan(config(ties)),
                config(ties),
                bm,
                FiniteProblem<Value>,
                Assignment<Value>
            );
            verify!(
                ListScoreDirector::new(&lp, &ln, lp.initial_assignment()).unwrap(),
                plan(config(ties)),
                config(ties),
                lm,
                ListProblem<Value>,
                ListAssignment<Value>
            );
            verify!(MixedScoreDirector::new(&mp,&mn,mp.initial_assignment()).unwrap(),mixed_plan(config(ties)),config(ties),mm,MixedProblem<Value,Value>,MixedAssignment<Value,Value>);
        }
    }
}

#[test]
fn clone_panic_in_sparse_capture_restores_trial_and_cached_scores() {
    let c = Control::default();
    let p = basic(128, &c);
    let lp = lists(128, &c);
    let mp = MixedProblem::new(p.clone(), lp.clone());
    let mut bn = ConstraintNetwork::<Value, SimpleScore>::new(128);
    bn.add_constraint("payload", vec![0], |f| {
        if c.armed.get() {
            c.remaining.set(Some(3));
        }
        SimpleScore::new(f[0].unwrap().payload)
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::<Value, SimpleScore>::new(128, 128);
    ln.add_constraint("rows", vec![ListScope::Entity(0)], |f| {
        if c.armed.get() {
            c.remaining.set(Some(1));
        }
        let ListFact::Entity(row) = &f[0] else {
            unreachable!()
        };
        SimpleScore::new(-(row.len() as i64))
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::<Value, Value, SimpleScore>::new(128, 128, 128);
    mn.add_constraint("payload", vec![MixedScope::Basic(0)], |f| {
        if c.armed.get() {
            c.remaining.set(Some(3));
        }
        let MixedFact::Basic(v) = &f[0] else {
            unreachable!()
        };
        SimpleScore::new(v.unwrap().payload)
    })
    .unwrap();
    macro_rules! panic_case {
        ($director:expr,$p:ty,$a:ty,$plan:expr,$movement:expr) => {{
            let mut d = $director;
            let original = format!("{:?}", d.assignment());
            let score = d.score();
            let movement = $movement;
            let mut provider = |_: &$p, _: &$a, _: u64| {
                let moves = vec![movement.clone()];
                c.armed.set(true);
                moves
            };
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut provider);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                d.run_solver_with_extensions($plan, ext)
            }));
            assert!(result.is_err());
            c.armed.set(false);
            c.remaining.set(None);
            assert_eq!(format!("{:?}", d.assignment()), original);
            assert_eq!(d.score(), score);
            d.assert_consistent().unwrap();
        }};
    }
    let mut cfg = config(TieBreaking::First);
    cfg.assert_scores = false;
    panic_case!(
        ScoreDirector::new(&p, &bn, p.initial_assignment()).unwrap(),
        FiniteProblem<Value>,
        Assignment<Value>,
        plan(cfg.clone()),
        Move::Composite(vec![
            change(0, Value::new(1, 500, &c)),
            change(1, Value::new(1, 501, &c))
        ])
    );
    panic_case!(
        ListScoreDirector::new(&lp, &ln, lp.initial_assignment()).unwrap(),
        ListProblem<Value>,
        ListAssignment<Value>,
        plan(cfg.clone()),
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(1, 1)
        }
    );
    panic_case!(MixedScoreDirector::new(&mp,&mn,mp.initial_assignment()).unwrap(),MixedProblem<Value,Value>,MixedAssignment<Value,Value>,mixed_plan(cfg),MixedMove {basic:Move::Composite(vec![change(0,Value::new(1,500,&c)),change(1,Value::new(1,501,&c))]),lists:ListMove::Change {source:pos(0,0),destination:pos(1,1)}});
}

#[test]
fn dense_and_empty_components_match_full_copy_reference() {
    let c = Control::default();
    let p = basic(128, &c);
    let lp = lists(128, &c);
    let mp = MixedProblem::new(p.clone(), lp.clone());
    let mut n = MixedConstraintNetwork::<Value, Value, SimpleScore>::new(128, 128, 128);
    n.add_constraint(
        "visible",
        vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
        |facts| {
            let (MixedFact::Basic(b), MixedFact::ListEntity(row)) = (&facts[0], &facts[1]) else {
                unreachable!()
            };
            SimpleScore::new(b.unwrap().payload + row[0].payload)
        },
    )
    .unwrap();
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
        verify!(MixedScoreDirector::new(&mp,&n,mp.initial_assignment()).unwrap(),mixed_plan(cfg),cfg,vec![movement.clone(),movement],MixedProblem<Value,Value>,MixedAssignment<Value,Value>);
    }
}
