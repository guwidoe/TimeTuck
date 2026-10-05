//! Original queue bounds, recovery, ownership and solver integration cases.
use SelectionLifecycleEvent::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Keys {
    values: Vec<usize>,
    opens: usize,
    declared: Option<usize>,
    events: usize,
}
impl Keys {
    fn new(values: Vec<usize>) -> Self {
        Self {
            declared: Some(values.len()),
            values,
            opens: 0,
            events: 0,
        }
    }
}
impl<C> SelectionSource<C, usize> for Keys {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &C) {
        self.events += 1;
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        Box::new(self.values.iter().copied())
    }
}
#[test]
fn keys_are_phase_scoped_while_candidate_pools_use_current_context() {
    let mut q = PlacementQueue::new(
        Keys::new(vec![4, 4]),
        vec![PlacementFn(|c: &usize, k: &usize| vec![c + k])],
        PlacementOrder::Once,
        3,
        1,
    )
    .unwrap();
    q.event(SolveStarted, &0).unwrap();
    q.event(PhaseStarted, &0).unwrap();
    let held = q.next_placement(&10).unwrap().unwrap();
    assert_eq!(*held.key(), 4);
    assert_eq!(held.moves(), [14]);
    assert_eq!(held.child_index(), 0);
    q.key_source_mut().values = vec![9];
    q.key_source_mut().declared = Some(1);
    assert_eq!(q.next_placement(&20).unwrap().unwrap().moves(), [24]);
    assert!(!q.has_next().unwrap());
    assert!(q.next_placement(&0).unwrap().is_none());
    assert_eq!(q.key_source().opens, 1);
    q.event(PhaseEnded, &0).unwrap();
    q.event(PhaseStarted, &0).unwrap();
    let next = q.next_placement(&30).unwrap().unwrap();
    assert_eq!(*next.key(), 9);
    assert_eq!(next.moves(), [39]);
    assert_eq!(held.moves(), [14]);
    assert_eq!(q.key_source().opens, 2);
    q.event(PhaseEnded, &0).unwrap();
    q.event(SolveEnded, &0).unwrap();
    assert!(q.has_next().is_err());
}
#[test]
fn empty_keys_children_and_pools_have_distinct_exhaustion() {
    for order in [PlacementOrder::Once, PlacementOrder::Cycle] {
        let mut q = PlacementQueue::new(
            Keys::new(vec![]),
            vec![PlacementFn(|_: &(), _: &usize| vec![1])],
            order,
            0,
            1,
        )
        .unwrap();
        q.event(SolveStarted, &()).unwrap();
        q.event(PhaseStarted, &()).unwrap();
        assert!(!q.has_next().unwrap());
        assert!(q.next_placement(&()).unwrap().is_none());
        let children: Vec<Box<dyn PlacementSource<(), usize, i32>>> = vec![];
        let mut q = PlacementQueue::new(Keys::new(vec![0]), children, order, 1, 1).unwrap();
        q.event(SolveStarted, &()).unwrap();
        q.event(PhaseStarted, &()).unwrap();
        assert!(!q.has_next().unwrap());
        let mut q = PlacementQueue::new(
            Keys::new(vec![0]),
            vec![PlacementFn(|_: &(), _: &usize| Vec::<i32>::new())],
            order,
            1,
            0,
        )
        .unwrap();
        q.event(SolveStarted, &()).unwrap();
        q.event(PhaseStarted, &()).unwrap();
        assert!(q.next_placement(&()).unwrap().unwrap().is_empty());
        assert_eq!(q.has_next().unwrap(), order == PlacementOrder::Cycle);
    }
}
struct NonClone(usize);
struct NonCloneKeys;
impl SelectionSource<usize, NonClone> for NonCloneKeys {
    fn size(&mut self, _: &usize) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, _: &'a usize) -> Box<dyn Iterator<Item = NonClone> + 'a>
    where
        NonClone: 'a,
    {
        Box::new(std::iter::once(NonClone(7)))
    }
}
#[test]
fn snapshots_share_nonclone_keys_and_moves_without_opening_again() {
    let calls = Cell::new(0);
    let mut q = PlacementQueue::new(
        NonCloneKeys,
        vec![PlacementFn(|c: &usize, k: &NonClone| {
            calls.set(calls.get() + 1);
            vec![NonClone(c + k.0)]
        })],
        PlacementOrder::Cycle,
        1,
        1,
    )
    .unwrap();
    q.event(SolveStarted, &0).unwrap();
    q.event(PhaseStarted, &0).unwrap();
    for _ in 0..10 {
        assert!(q.has_next().unwrap());
    }
    assert_eq!(calls.get(), 0);
    let held = q.next_placement(&1).unwrap().unwrap();
    let shared = held.clone();
    assert!(std::ptr::eq(held.key(), shared.key()));
    assert!(std::ptr::eq(held.moves().as_ptr(), shared.moves().as_ptr()));
    q.event(PhaseEnded, &0).unwrap();
    q.event(PhaseStarted, &0).unwrap();
    assert_eq!(q.next_placement(&2).unwrap().unwrap().moves()[0].0, 9);
    assert_eq!(shared.key().0, 7);
    assert_eq!(held.iter().next().unwrap().0, 8);
    assert_eq!(calls.get(), 2);
}
#[derive(Clone, Copy)]
enum Mode {
    Good,
    DeclaredLimit,
    Mismatch,
    Endless,
    Unbounded,
    Panic,
}
struct Source {
    mode: Mode,
    opens: Rc<Cell<usize>>,
    pulls: Rc<Cell<usize>>,
}
impl PlacementSource<(), usize, usize> for Source {
    fn is_never_ending(&self) -> bool {
        matches!(self.mode, Mode::Unbounded)
    }
    fn size(&mut self, _: &(), _: &usize) -> Option<usize> {
        match self.mode {
            Mode::DeclaredLimit => Some(3),
            Mode::Mismatch => Some(2),
            _ => None,
        }
    }
    fn open<'a>(&'a mut self, _: &'a (), _: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        assert!(!matches!(self.mode, Mode::Panic), "source failed");
        if matches!(self.mode, Mode::Endless) {
            Box::new(std::iter::from_fn(move || {
                self.pulls.set(self.pulls.get() + 1);
                Some(1)
            }))
        } else {
            Box::new(std::iter::once(1))
        }
    }
}
#[test]
fn failed_or_panicking_pools_poison_only_current_phase_and_keep_held_snapshots() {
    for mode in [
        Mode::DeclaredLimit,
        Mode::Mismatch,
        Mode::Endless,
        Mode::Unbounded,
        Mode::Panic,
    ] {
        let opens = Rc::new(Cell::new(0));
        let pulls = Rc::new(Cell::new(0));
        let mut q = PlacementQueue::new(
            Keys::new(vec![0]),
            vec![Source {
                mode: Mode::Good,
                opens: opens.clone(),
                pulls: pulls.clone(),
            }],
            PlacementOrder::Cycle,
            1,
            2,
        )
        .unwrap();
        q.event(SolveStarted, &()).unwrap();
        q.event(PhaseStarted, &()).unwrap();
        let held = q.next_placement(&()).unwrap().unwrap();
        q.children_mut()[0].mode = mode;
        if matches!(mode, Mode::Panic) {
            assert!(catch_unwind(AssertUnwindSafe(|| q.next_placement(&()))).is_err());
        } else {
            let result = q.next_placement(&());
            assert!(match mode {
                Mode::DeclaredLimit | Mode::Endless => matches!(
                    result,
                    Err(PlacementError::CandidateLimit {
                        child: 0,
                        maximum: 2
                    })
                ),
                Mode::Mismatch => matches!(
                    result,
                    Err(PlacementError::SizeMismatch {
                        child: 0,
                        declared: 2,
                        actual: 1
                    })
                ),
                Mode::Unbounded =>
                    matches!(result, Err(PlacementError::UnboundedChild { child: 0 })),
                _ => false,
            });
        }
        assert!(matches!(q.has_next(), Err(PlacementError::Unavailable)));
        assert!(matches!(
            q.next_placement(&()),
            Err(PlacementError::Unavailable)
        ));
        assert_eq!(held.moves(), [1]);
        if matches!(mode, Mode::DeclaredLimit | Mode::Unbounded) {
            assert_eq!(opens.get(), 1);
        }
        if matches!(mode, Mode::Endless) {
            assert_eq!(pulls.get(), 3);
        }
        q.children_mut()[0].mode = Mode::Good;
        q.event(PhaseEnded, &()).unwrap();
        q.event(PhaseStarted, &()).unwrap();
        assert_eq!(q.next_placement(&()).unwrap().unwrap().moves(), [1]);
    }
}
#[test]
fn invalid_events_do_not_poison_and_failed_key_builds_can_recover() {
    let mut q = PlacementQueue::new(
        Keys::new(vec![0]),
        vec![PlacementFn(|_: &(), _: &usize| vec![0])],
        PlacementOrder::Once,
        1,
        1,
    )
    .unwrap();
    assert!(matches!(
        q.event(PhaseStarted, &()),
        Err(PlacementError::Cache(
            SelectionCacheError::InvalidTransition { .. }
        ))
    ));
    assert_eq!(q.key_source().events, 0);
    q.event(SolveStarted, &()).unwrap();
    q.event(PhaseStarted, &()).unwrap();
    assert!(q.event(SolveEnded, &()).is_err());
    assert!(q.has_next().unwrap());
    assert_eq!(q.key_source().events, 2);
    q.event(PhaseEnded, &()).unwrap();
    q.key_source_mut().declared = Some(0);
    assert!(matches!(
        q.event(PhaseStarted, &()),
        Err(PlacementError::Cache(SelectionCacheError::SizeMismatch {
            declared: 0,
            actual: 1
        }))
    ));
    assert!(matches!(q.has_next(), Err(PlacementError::Unavailable)));
    q.event(PhaseEnded, &()).unwrap();
    q.key_source_mut().declared = Some(1);
    q.event(PhaseStarted, &()).unwrap();
    assert!(q.has_next().unwrap());
    q.event(StepStarted, &()).unwrap();
    q.event(StepEnded, &()).unwrap();
    q.event(PhaseEnded, &()).unwrap();
    q.event(SolveEnded, &()).unwrap();
}
#[test]
fn grouped_joint_placements_construct_complete_solutions_through_both_backends() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| Variable::new(vec![2 * i + 1, 2 * i + 2]))
            .collect(),
    )
    .unwrap();
    for stream in [false, true] {
        let child = PlacementFn(|a: &Assignment<i32>, key: &usize| {
            let left = 2 * key;
            let right = left + 1;
            if a.values()[left].is_some() {
                return vec![];
            }
            p.variables()[left]
                .domain()
                .iter()
                .flat_map(|&x| {
                    p.variables()[right].domain().iter().map(move |&y| {
                        Move::Composite(vec![
                            Move::Change {
                                variable: left,
                                value: Some(x),
                            },
                            Move::Change {
                                variable: right,
                                value: Some(y),
                            },
                        ])
                    })
                })
                .collect()
        });
        let mut q = PlacementQueue::new(
            Keys::new(vec![0, 1]),
            vec![child],
            PlacementOrder::Once,
            2,
            4,
        )
        .unwrap();
        let initial = p.initial_assignment();
        q.event(SolveStarted, &initial).unwrap();
        q.event(PhaseStarted, &initial).unwrap();
        let mut seen = vec![];
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, step| {
            seen.push((step, a.clone()));
            // Explicit queue lifecycle surrounds this solve; no hidden Java listener wiring.
            while let Some(placement) = q.next_placement(a).unwrap() {
                if !placement.is_empty() {
                    return placement.moves().to_vec();
                }
            }
            vec![]
        };
        let plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                }),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let r = if stream {
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut provider);
            solve_stream_with_extensions(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = b.source::<usize, (usize, Option<i32>)>();
                    b.score("value", s.port(), |v| {
                        SimpleScore::new(i64::from(v.1.unwrap_or(0)))
                    })?;
                    Ok((b.build(), s))
                },
                plan,
                ext,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(4);
            n.add_constraint("sum", vec![0, 1, 2, 3], |v| {
                SimpleScore::new(v.iter().filter_map(|v| v.copied()).map(i64::from).sum())
            })
            .unwrap();
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut provider);
            solve_with_extensions(&p, &n, plan, ext).unwrap()
        };
        assert!(r.complete);
        assert_eq!(r.assignment.values(), &[Some(2), Some(4), Some(6), Some(8)]);
        assert_eq!(r.score, Some(SimpleScore::new(20)));
        assert_eq!(r.phases[0].work.move_evaluations, 8);
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], (0, initial));
        assert_eq!(seen[1].1.values(), &[Some(2), Some(4), None, None]);
        q.event(PhaseEnded, &r.assignment).unwrap();
        q.event(SolveEnded, &r.assignment).unwrap();
    }
}

#[test]
fn key_build_errors_still_deliver_balanced_child_events() {
    struct Child {
        phase: bool,
        starts: usize,
        ends: usize,
    }
    impl PlacementSource<(), usize, usize> for Child {
        fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
            match e {
                PhaseStarted => {
                    assert!(!self.phase);
                    self.phase = true;
                    self.starts += 1;
                }
                PhaseEnded => {
                    assert!(self.phase);
                    self.phase = false;
                    self.ends += 1;
                }
                _ => {}
            }
        }
        fn size(&mut self, _: &(), _: &usize) -> Option<usize> {
            Some(1)
        }
        fn open<'a>(&'a mut self, _: &'a (), _: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
        where
            usize: 'a,
        {
            assert!(self.phase);
            Box::new(std::iter::once(1))
        }
    }
    let mut keys = Keys::new(vec![0]);
    keys.declared = Some(0);
    let mut q = PlacementQueue::new(
        keys,
        vec![Child {
            phase: false,
            starts: 0,
            ends: 0,
        }],
        PlacementOrder::Once,
        1,
        1,
    )
    .unwrap();
    q.event(SolveStarted, &()).unwrap();
    assert!(q.event(PhaseStarted, &()).is_err());
    assert_eq!(q.children()[0].starts, 1);
    assert!(q.children()[0].phase);
    q.event(PhaseEnded, &()).unwrap();
    q.key_source_mut().declared = Some(1);
    q.event(PhaseStarted, &()).unwrap();
    assert_eq!(q.next_placement(&()).unwrap().unwrap().moves(), [1]);
    q.event(PhaseEnded, &()).unwrap();
    q.event(SolveEnded, &()).unwrap();
    assert_eq!((q.children()[0].starts, q.children()[0].ends), (2, 2));
}
