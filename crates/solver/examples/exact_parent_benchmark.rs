//! Same-host actual bounded-search workloads. Model/network setup is excluded;
//! each timed solve includes root preparation, retained director setup, scoring,
//! best snapshots, restoration and dropping results/directors.
use std::{cell::Cell, fmt, hint::black_box, marker::PhantomData, rc::Rc, time::Instant};
use timetuck::*;
#[derive(Eq)]
struct V {
    id: i64,
    bytes: Vec<u8>,
    clones: Rc<Cell<u64>>,
}
impl PartialEq for V {
    fn eq(&self, b: &Self) -> bool {
        self.id == b.id
    }
}
impl Clone for V {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            bytes: self.bytes.clone(),
            clones: self.clones.clone(),
        }
    }
}
impl fmt::Debug for V {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.id.fmt(f)
    }
}
trait Objective {
    fn score(&self) -> SimpleScore;
}
impl Objective for Assignment<V> {
    fn score(&self) -> SimpleScore {
        SimpleScore::new(-self.values().iter().flatten().map(|v| v.id).sum::<i64>())
    }
}
impl Objective for ListAssignment<V> {
    fn score(&self) -> SimpleScore {
        SimpleScore::new(
            -self
                .lists()
                .iter()
                .enumerate()
                .map(|(e, l)| {
                    l.iter()
                        .enumerate()
                        .map(|(i, v)| (e + i + 1) as i64 * v.id)
                        .sum::<i64>()
                })
                .sum::<i64>(),
        )
    }
}
impl Objective for MixedAssignment<V, V> {
    fn score(&self) -> SimpleScore {
        SimpleScore::new(self.basic().score().score() + self.lists().score().score())
    }
}
#[derive(Clone)]
struct Cache<C> {
    score: SimpleScore,
    marker: PhantomData<C>,
}
fn cache<C>() -> Cache<C> {
    Cache {
        score: SimpleScore::ZERO,
        marker: PhantomData,
    }
}
impl<A: Objective, C> IncrementalCalculator<A> for Cache<C> {
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
    fn after_change(
        &mut self,
        a: &A,
        _: &C,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.score
    }
}
fn phase(cap: u64, bounded: bool) -> ExactPhaseConfig<'static> {
    ExactPhaseConfig {
        max_evaluations: cap,
        strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
            bound: if bounded {
                PartialScoreBound::NonIncreasing
            } else {
                PartialScoreBound::Unknown
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn plan(c: ExactPhaseConfig<'_>) -> SolverSequence<'_> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn mixed(c: ExactPhaseConfig<'_>) -> MixedSolverSequence<'_> {
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn evaluation(score: SimpleScore, bounded: bool) -> BranchEvaluation<SimpleScore> {
    if bounded {
        BranchEvaluation::bounded(score, score)
    } else {
        BranchEvaluation::unbounded(score)
    }
}
fn samples(
    mut f: impl FnMut() -> (u64, i64),
    clones: &Cell<u64>,
    warmup: usize,
    count: usize,
) -> serde_json::Value {
    for _ in 0..warmup {
        black_box(f());
    }
    let mut ns = vec![];
    let mut clone_counts = vec![];
    let mut outcome = None;
    for _ in 0..count {
        clones.set(0);
        let now = Instant::now();
        let next = black_box(f());
        ns.push(now.elapsed().as_nanos());
        clone_counts.push(clones.get());
        if let Some(previous) = outcome {
            assert_eq!(previous, next);
        }
        outcome = Some(next);
    }
    serde_json::json!({"ns":ns,"value_clones":clone_counts,"result":outcome})
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|a| a == "reference") {
        reference();
        return;
    }
    let warmup = args.get(1).map_or(1, |s| s.parse().unwrap());
    let count = args.get(2).map_or(5, |s| s.parse().unwrap());
    for (name, bytes, cap, bounded) in [
        ("small", 0, 10_000, false),
        ("owned", 4096, 10_000, false),
        ("pruned", 4096, 10_000, true),
        ("short", 4096, 2, false),
        ("zero", 4096, 0, false),
    ] {
        let clones = Rc::new(Cell::new(0));
        let value = |id| V {
            id,
            bytes: vec![id as u8; bytes],
            clones: clones.clone(),
        };
        let b = FiniteProblem::new(vec![
            Variable::new(vec![value(0), value(1)]);
            if name == "small" { 3 } else { 8 }
        ])
        .unwrap();
        let l = ListProblem::new(
            (1..=if name == "small" { 3 } else { 5 })
                .map(value)
                .collect(),
            vec![ListEntity::new(vec![]); 2],
        )
        .unwrap();
        let m = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![value(0), value(1)]); 3]).unwrap(),
            l.clone(),
        );
        let mut bn = ConstraintNetwork::<V, SimpleScore>::new(b.variables().len());
        bn.add_constraint("sum", (0..b.variables().len()).collect(), |v| {
            SimpleScore::new(-v.iter().flatten().map(|v| v.id).sum::<i64>())
        })
        .unwrap();
        let mut ln = ListConstraintNetwork::<V, SimpleScore>::new(2, l.values().len());
        ln.add_constraint(
            "list",
            vec![ListScope::Entity(0), ListScope::Entity(1)],
            |f| {
                let [ListFact::Entity(a), ListFact::Entity(b)] = f else {
                    unreachable!()
                };
                SimpleScore::new(
                    -a.iter()
                        .enumerate()
                        .map(|(i, v)| (i + 1) as i64 * v.id)
                        .chain(b.iter().enumerate().map(|(i, v)| (i + 2) as i64 * v.id))
                        .sum::<i64>(),
                )
            },
        )
        .unwrap();
        let mut mn = MixedConstraintNetwork::<V, V, SimpleScore>::new(3, 2, l.values().len());
        mn.add_constraint(
            "joint",
            vec![
                MixedScope::Basic(0),
                MixedScope::Basic(1),
                MixedScope::Basic(2),
                MixedScope::ListEntity(0),
                MixedScope::ListEntity(1),
            ],
            |f| {
                let [
                    MixedFact::Basic(x),
                    MixedFact::Basic(y),
                    MixedFact::Basic(z),
                    MixedFact::ListEntity(a),
                    MixedFact::ListEntity(b),
                ] = f
                else {
                    unreachable!()
                };
                SimpleScore::new(
                    -x.iter()
                        .chain(y)
                        .chain(z)
                        .map(|v| v.id)
                        .chain(a.iter().enumerate().map(|(i, v)| (i + 1) as i64 * v.id))
                        .chain(b.iter().enumerate().map(|(i, v)| (i + 2) as i64 * v.id))
                        .sum::<i64>(),
                )
            },
        )
        .unwrap();
        for mode in [
            "standalone",
            "fresh",
            "retained-scoped",
            "retained-calculator",
        ] {
            for model in ["basic", "list", "mixed"] {
                let c = phase(cap, bounded);
                let bc = BranchAndBoundConfig {
                    max_evaluations: cap,
                    ..Default::default()
                };
                let row = samples(
                    || match (mode, model) {
                        ("standalone", "basic") => {
                            let r = branch_and_bound(&b, |a, _| evaluation(a.score(), bounded), bc)
                                .unwrap();
                            (r.evaluations, r.best.map_or(i64::MIN, |r| r.score.score()))
                        }
                        ("standalone", "list") => {
                            let r = branch_and_bound_lists(
                                &l,
                                |a, _| evaluation(a.score(), bounded),
                                bc,
                            )
                            .unwrap();
                            (r.evaluations, r.best.map_or(i64::MIN, |r| r.score.score()))
                        }
                        ("standalone", "mixed") => {
                            let r = branch_and_bound_mixed(
                                &m,
                                |a, _, _| evaluation(a.score(), bounded),
                                bc,
                            )
                            .unwrap();
                            (r.evaluations, r.best.map_or(i64::MIN, |r| r.score.score()))
                        }
                        ("fresh", "basic") => {
                            let r = solve(&b, &bn, plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("fresh", "list") => {
                            let r = solve_list(&l, &ln, plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("fresh", "mixed") => {
                            let r = solve_mixed(&m, &mn, mixed(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-scoped", "basic") => {
                            let mut d =
                                ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap();
                            let r = d.run_solver(plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-scoped", "list") => {
                            let mut d =
                                ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap();
                            let r = d.run_solver(plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-scoped", "mixed") => {
                            let mut d =
                                MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap();
                            let r = d.run_solver(mixed(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-calculator", "basic") => {
                            let mut d = CalculatorScoreDirector::new(
                                &b,
                                b.initial_assignment(),
                                cache::<Vec<usize>>,
                                false,
                            )
                            .unwrap();
                            let r = d.run_solver(plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-calculator", "list") => {
                            let mut d = ListCalculatorScoreDirector::new(
                                &l,
                                l.initial_assignment(),
                                cache::<Vec<usize>>,
                                false,
                            )
                            .unwrap();
                            let r = d.run_solver(plan(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        ("retained-calculator", "mixed") => {
                            let mut d = MixedCalculatorScoreDirector::new(
                                &m,
                                m.initial_assignment(),
                                cache::<MixedCalculatorChange>,
                                false,
                            )
                            .unwrap();
                            let r = d.run_solver(mixed(c.clone())).unwrap();
                            (r.work.evaluations, r.score.map_or(i64::MIN, |s| s.score()))
                        }
                        _ => unreachable!(),
                    },
                    &clones,
                    warmup,
                    count,
                );
                println!(
                    "{}",
                    serde_json::json!({"model":model,"mode":mode,"case":name,"bytes":bytes,"data":row})
                );
            }
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 8]).unwrap();
    let row = samples(
        || {
            let r = branch_and_bound(
                &p,
                |a, _| {
                    BranchEvaluation::unbounded(SimpleScore::new(
                        -a.values().iter().flatten().sum::<i64>(),
                    ))
                },
                BranchAndBoundConfig::default(),
            )
            .unwrap();
            (r.evaluations, r.best.unwrap().score.score())
        },
        &Cell::new(0),
        warmup,
        count,
    );
    println!(
        "{}",
        serde_json::json!({"model":"basic","mode":"standalone","case":"scalar","bytes":0,"data":row})
    );
}
fn reference() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]); 3]).unwrap();
    let l = ListProblem::new(vec![1i64, 2, 3], vec![ListEntity::new(vec![]); 2])
        .unwrap()
        .allowing_unassigned();
    let m = MixedProblem::new(b.clone(), l.clone());
    let mut rows = vec![];
    for order in [
        NodeExploration::DepthFirst,
        NodeExploration::BreadthFirst,
        NodeExploration::ScoreFirst,
        NodeExploration::OptimisticBoundFirst,
    ] {
        for cap in [0, 1, 2, 3, 5, 9, 40, 2000] {
            for frontier in [0, 1, 3, 1000] {
                for bound in [false, true] {
                    for cancel in [false, true] {
                        let token = CancellationToken::new();
                        let calls = Cell::new(0);
                        let trace = std::cell::RefCell::new(vec![]);
                        let best = std::cell::RefCell::new(vec![]);
                        let config = BranchAndBoundConfig {
                            exploration: order,
                            max_evaluations: cap,
                            max_frontier: frontier,
                            cancellation: Some(&token),
                            ..Default::default()
                        };
                        macro_rules! record {($run:expr,$model:expr)=>{{let r=$run;rows.push(serde_json::json!({"model":$model,"order":format!("{order:?}"),"cap":cap,"frontier":frontier,"bound":bound,"cancel":cancel,"trace":trace.take(),"best":best.take(),"result":format!("{r:?}")}));}};}
                        record!(
                            branch_and_bound_with_observer(
                                &b,
                                |a, remaining| {
                                    calls.set(calls.get() + 1);
                                    trace.borrow_mut().push(format!("{a:?}:{remaining:?}"));
                                    if cancel && calls.get() == 5 {
                                        token.cancel();
                                    }
                                    evaluation(
                                        SimpleScore::new(-a.values().iter().flatten().sum::<i64>()),
                                        bound,
                                    )
                                },
                                config,
                                |r| best.borrow_mut().push(format!("{r:?}"))
                            ),
                            "basic"
                        );
                        let token = CancellationToken::new();
                        calls.set(0);
                        let config = BranchAndBoundConfig {
                            cancellation: Some(&token),
                            ..config
                        };
                        record!(
                            branch_and_bound_lists_with_observer(
                                &l,
                                |a, remaining| {
                                    calls.set(calls.get() + 1);
                                    trace.borrow_mut().push(format!("{a:?}:{remaining:?}"));
                                    if cancel && calls.get() == 5 {
                                        token.cancel();
                                    }
                                    evaluation(
                                        SimpleScore::new(-a.lists().iter().flatten().sum::<i64>()),
                                        bound,
                                    )
                                },
                                config,
                                |r| best.borrow_mut().push(format!("{r:?}"))
                            ),
                            "list"
                        );
                        let token = CancellationToken::new();
                        calls.set(0);
                        let config = BranchAndBoundConfig {
                            cancellation: Some(&token),
                            ..config
                        };
                        record!(
                            branch_and_bound_mixed_with_observer(
                                &m,
                                |a, b, l| {
                                    calls.set(calls.get() + 1);
                                    trace.borrow_mut().push(format!("{a:?}:{b:?}:{l:?}"));
                                    if cancel && calls.get() == 5 {
                                        token.cancel();
                                    }
                                    evaluation(
                                        SimpleScore::new(
                                            -a.basic().values().iter().flatten().sum::<i64>()
                                                - a.lists().lists().iter().flatten().sum::<i64>(),
                                        ),
                                        bound,
                                    )
                                },
                                config,
                                |r| best.borrow_mut().push(format!("{r:?}"))
                            ),
                            "mixed"
                        );
                    }
                }
            }
        }
    }
    println!("{}", serde_json::to_string(&rows).unwrap());
}
