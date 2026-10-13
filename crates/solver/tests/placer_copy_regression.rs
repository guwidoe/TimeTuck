//! Original fresh provider execution, independent state and factory failure checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Keys {
    count: usize,
    events: Vec<SelectionLifecycleEvent>,
}
impl SelectionSource<(), usize> for Keys {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.count)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(0..self.count)
    }
}
struct Pool {
    values: Vec<i32>,
    events: Vec<SelectionLifecycleEvent>,
}
impl PlacementSource<(), usize, i32> for Pool {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &(), _: &usize) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a (), _: &'a usize) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        Box::new(self.values.iter().copied())
    }
}
#[derive(Clone)]
struct Policy {
    count: usize,
    values: Vec<i32>,
}
type Capture = fn(&FiniteProblem<i32>, &Assignment<i32>, u64);
type Mapper =
    fn(&FiniteProblem<i32>, &Assignment<i32>, u64, &usize, usize, &i32) -> Option<Move<i32>>;
type Provider = PlacementQueueProvider<(), usize, i32, Keys, Pool, Capture, Mapper>;
fn capture(_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64) {}
fn mapper(
    _: &FiniteProblem<i32>,
    a: &Assignment<i32>,
    _: u64,
    &key: &usize,
    _: usize,
    &value: &i32,
) -> Option<Move<i32>> {
    a.values()[key].is_none().then_some(Move::Change {
        variable: key,
        value: Some(value),
    })
}
fn build(c: &Policy) -> Result<Provider, PlacementError> {
    let q = PlacementQueue::new(
        Keys {
            count: c.count,
            events: vec![],
        },
        vec![Pool {
            values: c.values.clone(),
            events: vec![],
        }],
        PlacementOrder::Once,
        c.count,
        c.values.len(),
    )?;
    Ok(PlacementQueueProvider::new(
        q,
        capture as Capture,
        mapper as Mapper,
    ))
}
#[test]
fn copied_provider_runs_real_configured_construction_with_independent_lifecycle() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().flatten().map(|&&v| i64::from(v)).sum())
    })
    .unwrap();
    let mut original = RebuildablePlacer::build(
        Policy {
            count: 3,
            values: vec![1, 2],
        },
        build,
    )
    .unwrap();
    let run = |provider: &mut Provider| {
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind_shared([0], provider).unwrap();
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                })],
                limits: SolverLimits::default(),
            },
            ext,
        )
        .unwrap()
    };
    let first = run(original.session_mut());
    assert_eq!(first.assignment.values(), &[Some(2); 3]);
    assert_eq!(first.work.construction_steps, 3);
    let history = original.session().queue().key_source().events.clone();
    assert_eq!(
        history.first(),
        Some(&SelectionLifecycleEvent::SolveStarted)
    );
    assert_eq!(history.last(), Some(&SelectionLifecycleEvent::SolveEnded));
    let mut copy = original.copy().unwrap();
    assert!(copy.session().queue().key_source().events.is_empty());
    let second = run(copy.session_mut());
    assert_eq!(first.assignment, second.assignment);
    assert_eq!(second.work.construction_steps, 3);
    assert_eq!(copy.session().queue().key_source().events, history);
    assert_eq!(original.session().queue().key_source().events, history);
    assert_eq!(copy.session().queue().state(), SelectionCacheState::Idle);
}
#[test]
fn failed_or_panicking_rebuild_preserves_live_queue_and_retained_policy() {
    let mode = Rc::new(Cell::new(0));
    let flag = mode.clone();
    let mut original = RebuildablePlacer::build(
        Policy {
            count: 2,
            values: vec![1, 2],
        },
        move |c: &Policy| match flag.get() {
            1 => Err(PlacementError::Unavailable),
            2 => panic!("factory"),
            _ => build(c),
        },
    )
    .unwrap();
    // Rebuilding never opens or closes the original provider sources.
    let q = &mut original.session_mut().queue();
    assert_eq!(q.state(), SelectionCacheState::Idle);
    mode.set(1);
    assert!(matches!(
        original.copy(),
        Err(PlacerCopyError::Build(PlacementError::Unavailable))
    ));
    mode.set(2);
    assert!(catch_unwind(AssertUnwindSafe(|| original.copy())).is_err());
    assert!(original.session().queue().key_source().events.is_empty());
    assert_eq!(original.configuration().unwrap().count, 2);
    mode.set(0);
    let mut copy = original.copy().unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(2);
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        copy.session_mut(),
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1); 2]);
    assert!(original.session().queue().key_source().events.is_empty());
}
