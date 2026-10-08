//! Native grouped-selection ownership, failure and solver regressions.
use SelectionLifecycleEvent as E;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Source {
    ids: Vec<usize>,
    opens: usize,
    declared: Option<usize>,
    infinite: bool,
}
impl Source {
    fn new(ids: Vec<usize>) -> Self {
        Self {
            ids,
            opens: 0,
            declared: None,
            infinite: false,
        }
    }
}
impl<C> SelectionSource<C, usize> for Source {
    fn is_never_ending(&self) -> bool {
        self.infinite
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        Box::new(self.ids.iter().copied())
    }
}
#[test]
fn stable_groups_reuse_occurrence_handles_until_invalidated() {
    let mut s = GroupedSelectionSource::new(
        Source::new(vec![3, 1, 3, 2]),
        4,
        |_: &(), a: &usize, b: &usize| a % 2 == b % 2,
    );
    let first = s.try_groups(&()).unwrap();
    assert_eq!(first.len(), 2);
    assert!(!first.is_empty());
    assert!(first.get(2).is_none());
    assert_eq!(first.iter().len(), 2);
    assert_eq!(
        first
            .get(0)
            .unwrap()
            .iter()
            .map(|v| **v)
            .collect::<Vec<_>>(),
        [3, 1, 3]
    );
    assert!(!Arc::ptr_eq(&first.groups()[0][0], &first.groups()[0][2]));
    assert_eq!(s.size(&()), Some(2));
    let reopened = s.open(&()).collect::<Vec<_>>();
    assert!(Arc::ptr_eq(&first.groups()[0], &reopened[0]));
    assert_eq!(s.source().opens, 1);
    s.event(E::StepStarted, &());
    let next = s.try_groups(&()).unwrap();
    assert!(!Arc::ptr_eq(&first.groups()[0], &next.groups()[0]));
    assert_eq!(s.source().opens, 2);
    s.source_mut().ids.clear();
    assert!(s.try_groups(&()).unwrap().is_empty());
    assert_eq!(first.groups()[0].len(), 3);
}

#[test]
fn input_failures_and_comparator_panics_publish_no_partial_snapshot() {
    let panic_now = Cell::new(true);
    let mut s = GroupedSelectionSource::new(
        Source::new(vec![1, 2, 3]),
        3,
        |_: &(), a: &usize, b: &usize| {
            assert!(!panic_now.get(), "comparator");
            a == b
        },
    );
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.try_groups(&()))).is_err());
    panic_now.set(false);
    assert_eq!(s.try_groups(&()).unwrap().len(), 3);
    assert_eq!(s.source().opens, 2);
    s.source_mut().declared = Some(2);
    assert!(matches!(
        s.try_groups(&()),
        Err(SelectionProductError::SizeMismatch {
            child: 0,
            declared: 2,
            actual: 3
        })
    ));
    s.source_mut().declared = Some(4);
    assert!(matches!(
        s.try_groups(&()),
        Err(SelectionProductError::CandidateLimit {
            child: 0,
            maximum: 3
        })
    ));
    s.source_mut().declared = None;
    s.source_mut().ids.push(4);
    assert!(matches!(
        s.try_groups(&()),
        Err(SelectionProductError::CandidateLimit { .. })
    ));
    s.source_mut().infinite = true;
    assert!(matches!(
        s.try_groups(&()),
        Err(SelectionProductError::UnboundedChild { child: 0 })
    ));
}

struct Value {
    id: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for Value {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct NonClone {
    drops: Rc<Cell<usize>>,
}
impl SelectionSource<(), Value> for NonClone {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(3)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Value> + 'a>
    where
        Value: 'a,
    {
        let drops = self.drops.clone();
        Box::new((0..3).map(move |id| Value {
            id,
            drops: drops.clone(),
        }))
    }
}
#[test]
fn held_groups_own_nonclone_payloads_after_phase_cleanup() {
    let drops = Rc::new(Cell::new(0));
    let mut s = GroupedSelectionSource::new(
        NonClone {
            drops: drops.clone(),
        },
        3,
        |_: &(), a: &Value, b: &Value| a.id / 2 == b.id / 2,
    );
    let snapshot = s.try_groups(&()).unwrap();
    let first = snapshot.groups()[0].clone();
    drop(snapshot);
    s.event(E::PhaseEnded, &());
    assert_eq!(drops.get(), 1);
    drop(s);
    assert_eq!(first.iter().map(|v| v.id).collect::<Vec<_>>(), [0, 1]);
    drop(first);
    assert_eq!(drops.get(), 3);
}

#[test]
fn cached_group_source_drives_real_pillar_changes_and_regroups_each_step() {
    let p = FiniteProblem::new(
        [0, 0, 1]
            .into_iter()
            .map(|v| Variable::new(vec![0, 1, 2]).with_initial(v))
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("sum", vec![0, 1, 2], |v| {
        SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap())).sum())
    })
    .unwrap();
    let groups = RefCell::new(vec![]);
    let source = GroupedSelectionSource::new(
        Source::new(vec![0, 1, 2]),
        3,
        |a: &Assignment<i32>, &l: &usize, &r: &usize| a.values()[l] == a.values()[r],
    );
    let mut provider = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.clone(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64, group: &SelectionGroup<usize>| {
            let ids = group.iter().map(|v| **v).collect::<Vec<_>>();
            groups.borrow_mut().push((step, ids.clone()));
            let value = a.values()[ids[0]].unwrap() + 1;
            (value <= 2).then(|| {
                Move::Composite(
                    ids.into_iter()
                        .map(|variable| Move::Change {
                            variable,
                            value: Some(value),
                        })
                        .collect(),
                )
            })
        },
    )
    .unwrap();
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::HillClimbing,
                assert_scores: true,
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(2); 3]);
    assert_eq!(r.score, Some(SimpleScore::new(6)));
    assert_eq!(
        *groups.borrow(),
        [
            (0, vec![0, 1]),
            (0, vec![2]),
            (1, vec![0, 1, 2]),
            (2, vec![0, 1, 2])
        ]
    );
    assert_eq!(provider.cache().source().source().opens, 3);
}
