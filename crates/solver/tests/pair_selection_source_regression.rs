//! Original pair source storage, failure and real-solver integration cases.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Source {
    values: Vec<usize>,
    declared: Option<usize>,
    infinite: bool,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
}
impl Source {
    fn new(values: Vec<usize>) -> Self {
        Self {
            values,
            declared: None,
            infinite: false,
            events: Rc::default(),
        }
    }
}
impl SelectionSource<(), usize> for Source {
    fn is_never_ending(&self) -> bool {
        self.infinite
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.borrow_mut().push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        Box::new(self.values.iter().copied())
    }
}
#[test]
fn occurrence_identity_empty_dimensions_and_collection_errors() {
    let mut source = PairSelectionSource::shared(Source::new(vec![1, 1, 2]), 3);
    assert_eq!(source.size(&()), None);
    let pairs = source.try_pairs(&()).unwrap().collect::<Vec<_>>();
    assert_eq!(
        pairs.iter().map(|(a, b)| (**a, **b)).collect::<Vec<_>>(),
        vec![(1, 1), (1, 2), (1, 2)]
    );
    assert!(!Arc::ptr_eq(&pairs[0].0, &pairs[0].1));
    assert!(Arc::ptr_eq(&pairs[0].0, &pairs[1].0));
    assert!(Arc::ptr_eq(&pairs[0].1, &pairs[2].0));
    let mut empty = Source::new(vec![]);
    empty.declared = Some(0);
    let mut product = PairSelectionSource::separate(Source::new(vec![1, 2]), empty, 2);
    assert_eq!(product.size(&()), Some(0));
    assert_eq!(product.try_pairs(&()).unwrap().count(), 0);
    let mut invalid = Source::new(vec![0]);
    invalid.declared = Some(2);
    let mut product = PairSelectionSource::separate(Source::new(vec![0]), invalid, 3);
    assert!(matches!(
        product.try_pairs(&()),
        Err(SelectionProductError::SizeMismatch {
            child: 1,
            declared: 2,
            actual: 1
        })
    ));
    product.sources_mut()[1].declared = None;
    product.sources_mut()[1].values = vec![0, 1, 2, 3];
    assert!(matches!(
        product.try_pairs(&()),
        Err(SelectionProductError::CandidateLimit {
            child: 1,
            maximum: 3
        })
    ));
    product.sources_mut()[1].infinite = true;
    assert!(product.is_never_ending());
    assert!(matches!(
        product.try_pairs(&()),
        Err(SelectionProductError::UnboundedChild { child: 1 })
    ));
    let mut enormous = Source::new(vec![]);
    enormous.declared = Some(usize::MAX);
    assert_eq!(PairSelectionSource::shared(enormous, 3).size(&()), None);
}
struct Key {
    id: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for Key {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Keys {
    n: usize,
    drops: Rc<Cell<usize>>,
}
impl SelectionSource<(), Key> for Keys {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.n)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Key> + 'a>
    where
        Key: 'a,
    {
        let drops = self.drops.clone();
        Box::new((0..self.n).map(move |id| Key {
            id,
            drops: drops.clone(),
        }))
    }
}
#[test]
fn large_products_store_inputs_only_and_pair_handles_own_nonclone_payloads() {
    let drops = Rc::new(Cell::new(0));
    let mut source = PairSelectionSource::separate(
        Keys {
            n: 10_000,
            drops: drops.clone(),
        },
        Keys {
            n: 10_000,
            drops: drops.clone(),
        },
        10_000,
    );
    let mut pairs = source.try_pairs(&()).unwrap();
    assert_eq!(pairs.remaining(), 100_000_000);
    let first = pairs.next().unwrap();
    let second = pairs.next().unwrap();
    assert_eq!((first.0.id, first.1.id), (0, 0));
    assert_eq!((second.0.id, second.1.id), (0, 1));
    assert!(Arc::ptr_eq(&first.0, &second.0));
    drop(source);
    drop(pairs);
    assert_eq!(drops.get(), 19_997);
    drop(first);
    drop(second);
    assert_eq!(drops.get(), 20_000);
}
#[test]
fn shared_source_drives_two_real_solver_phases_with_one_complete_lifecycle() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| Variable::new(vec![0, 1, 2, 3]).with_initial(i))
            .collect(),
    )
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(4);
    let source = Source::new(vec![0, 1, 2, 3]);
    let events = source.events.clone();
    let proposals = RefCell::new(vec![]);
    let mut provider = CachedMoveProvider::uniform(
        PairSelectionSource::shared(source, 4),
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, step: u64, pair: &SelectionPair<usize>| {
            proposals.borrow_mut().push((step, *pair.0, *pair.1));
            Some(Move::Swap {
                left: *pair.0,
                right: *pair.1,
            })
        },
    )
    .unwrap();
    let mut ext = SolverExtensions::default();
    ext.move_providers
        .bind_shared([0, 1], &mut provider)
        .unwrap();
    let plan = SolverSequence {
        phases: [2, 3]
            .into_iter()
            .map(|max_steps| {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps,
                    acceptance: Acceptance::HillClimbing,
                    selection: MoveSelection::BestAccepted,
                    ..Default::default()
                })
            })
            .collect(),
        ..Default::default()
    };
    let r = solve_with_extensions(&p, &n, plan, ext).unwrap();
    assert_eq!(r.work.completed_steps, 5);
    assert_eq!(r.work.move_evaluations, 30);
    let expected: Vec<_> = [0, 1, 0, 1, 2]
        .into_iter()
        .flat_map(|step| {
            (0..4).flat_map(move |left| (left + 1..4).map(move |right| (step, left, right)))
        })
        .collect();
    assert_eq!(*proposals.borrow(), expected);
    for (event, count) in [
        (SelectionLifecycleEvent::SolveStarted, 1),
        (SelectionLifecycleEvent::SolveEnded, 1),
        (SelectionLifecycleEvent::PhaseStarted, 2),
        (SelectionLifecycleEvent::PhaseEnded, 2),
        (SelectionLifecycleEvent::StepStarted, 5),
        (SelectionLifecycleEvent::StepEnded, 5),
    ] {
        assert_eq!(
            events.borrow().iter().filter(|&&e| e == event).count(),
            count
        );
    }
}
