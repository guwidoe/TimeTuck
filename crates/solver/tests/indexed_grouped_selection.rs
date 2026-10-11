//! Original hash-grouping work bounds, ownership, failure and provider regressions.
use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;

struct Source {
    ids: Vec<usize>,
    opens: usize,
    events: usize,
}
impl Source {
    fn new(ids: Vec<usize>) -> Self {
        Self {
            ids,
            opens: 0,
            events: 0,
        }
    }
}
impl<C> SelectionSource<C, usize> for Source {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &C) {
        self.events += 1;
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.ids.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        Box::new(self.ids.iter().copied())
    }
}
fn rows(groups: &GroupedSelection<usize>) -> Vec<Vec<usize>> {
    groups
        .iter()
        .map(|g| g.iter().map(|i| **i).collect())
        .collect()
}
#[test]
fn indexed_comparison_work_is_linear_for_distinct_keys_and_duplicates() {
    let n = 2048;
    let ids = (0..n).chain((0..n).rev()).collect::<Vec<_>>();
    let eq = Cell::new(0usize);
    let hashes = Cell::new(0usize);
    let mut source = GroupedSelectionSource::new(
        Source::new(ids.clone()),
        ids.len(),
        |_: &(), a: &usize, b: &usize| {
            eq.set(eq.get() + 1);
            a == b
        },
    )
    .with_hash(|_: &(), id: &usize| {
        hashes.set(hashes.get() + 1);
        *id as u64
    });
    let first = source.try_groups(&()).unwrap();
    assert_eq!(eq.get(), n);
    assert_eq!(hashes.get(), 2 * n);
    assert_eq!(rows(&first), (0..n).map(|i| vec![i, i]).collect::<Vec<_>>());
    for group in first.groups() {
        assert!(!Arc::ptr_eq(&group[0], &group[1]));
    }
    let again = source.try_groups(&()).unwrap();
    assert!(Arc::ptr_eq(&first.groups()[0], &again.groups()[0]));
    assert_eq!(
        (eq.get(), hashes.get(), source.source().opens),
        (n, 2 * n, 1)
    );
    let plain_eq = Cell::new(0usize);
    let mut plain =
        GroupedSelectionSource::new(Source::new(ids), 2 * n, |_: &(), a: &usize, b: &usize| {
            plain_eq.set(plain_eq.get() + 1);
            a == b
        });
    assert_eq!(rows(&plain.try_groups(&()).unwrap()), rows(&first));
    assert_eq!(plain_eq.get(), n * n);
}
#[test]
fn collision_order_matches_plain_comparator_trace_and_context_edits_rebuild() {
    use std::cell::RefCell;
    let plain_trace = RefCell::new(vec![]);
    let hash_trace = RefCell::new(vec![]);
    let mut plain = GroupedSelectionSource::new(
        Source::new(vec![3, 1, 3, 0, 2, 4, 1]),
        7,
        |ctx: &Vec<usize>, a: &usize, b: &usize| {
            plain_trace.borrow_mut().push((*a, *b));
            ctx[*a] == ctx[*b]
        },
    );
    let mut indexed = GroupedSelectionSource::new(
        Source::new(vec![3, 1, 3, 0, 2, 4, 1]),
        7,
        |ctx: &Vec<usize>, a: &usize, b: &usize| {
            hash_trace.borrow_mut().push((*a, *b));
            ctx[*a] == ctx[*b]
        },
    )
    .with_hash(|_: &Vec<usize>, _: &usize| 0);
    for ctx in [vec![0, 1, 2, 3, 4], vec![0, 1, 0, 1, 0], vec![0; 5]] {
        plain.event(SelectionLifecycleEvent::StepStarted, &ctx);
        indexed.event(SelectionLifecycleEvent::StepStarted, &ctx);
        plain_trace.borrow_mut().clear();
        hash_trace.borrow_mut().clear();
        assert_eq!(
            rows(&plain.try_groups(&ctx).unwrap()),
            rows(&indexed.try_groups(&ctx).unwrap())
        );
        assert_eq!(*plain_trace.borrow(), *hash_trace.borrow());
    }
    assert_eq!(indexed.source().events, 3);
    let old = indexed.try_groups(&vec![0; 5]).unwrap();
    indexed.source_mut().ids = vec![4, 0];
    assert_eq!(
        rows(&indexed.try_groups(&vec![0, 1, 2, 3, 4]).unwrap()),
        [vec![4], vec![0]]
    );
    assert_eq!(rows(&old), [vec![3, 1, 3, 0, 2, 4, 1]]);
    indexed.equivalent_mut();
    indexed.hash_mut();
    assert_eq!(
        rows(&indexed.try_groups(&vec![0; 5]).unwrap()),
        [vec![4, 0]]
    );
}
struct Item {
    id: usize,
    dropped: Rc<Cell<usize>>,
}
impl Drop for Item {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
struct Items {
    dropped: Rc<Cell<usize>>,
    opens: usize,
}
impl SelectionSource<(), Item> for Items {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(4)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Item> + 'a>
    where
        Item: 'a,
    {
        self.opens += 1;
        Box::new((0..4).map(|id| Item {
            id,
            dropped: self.dropped.clone(),
        }))
    }
}
#[test]
fn callback_panics_drop_unpublished_nonclone_payloads_and_retry_from_source() {
    let dropped = Rc::new(Cell::new(0));
    let hash_panic = Cell::new(false);
    let eq_panic = Cell::new(false);
    let mut source = GroupedSelectionSource::new(
        Items {
            dropped: dropped.clone(),
            opens: 0,
        },
        4,
        |_: &(), a: &Item, b: &Item| {
            assert!(!eq_panic.get());
            a.id % 2 == b.id % 2
        },
    )
    .with_hash(|_: &(), i: &Item| {
        assert!(!hash_panic.get() || i.id < 2);
        0
    });
    let first = source.try_groups(&()).unwrap();
    assert_eq!(first.len(), 2);
    hash_panic.set(true);
    source.hash_mut();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.try_groups(&()))).is_err()
    );
    assert_eq!(dropped.get(), 4);
    assert_eq!(first.groups()[0][1].id, 2);
    hash_panic.set(false);
    eq_panic.set(true);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.try_groups(&()))).is_err()
    );
    assert_eq!(dropped.get(), 8);
    eq_panic.set(false);
    let next = source.try_groups(&()).unwrap();
    assert_eq!(next.len(), 2);
    assert_eq!(source.source().opens, 4);
    assert!(!Arc::ptr_eq(&first.groups()[0], &next.groups()[0]));
    drop(source);
    drop(next);
    assert_eq!(dropped.get(), 12);
    drop(first);
    assert_eq!(dropped.get(), 16);
}
#[test]
fn cached_move_provider_regroups_after_each_actual_assignment_change() {
    use std::cell::RefCell;
    let problem = FiniteProblem::new(
        [0, 0, 1, 2]
            .map(|v| Variable::new(vec![0, 1, 2, 3]).with_initial(v))
            .to_vec(),
    )
    .unwrap();
    let mut network = ConstraintNetwork::new(4);
    network
        .add_constraint("sum", vec![0, 1, 2, 3], |values| {
            SimpleScore::new(values.iter().map(|v| i64::from(v.copied().unwrap())).sum())
        })
        .unwrap();
    let groups = RefCell::new(vec![]);
    let hashes = Cell::new(0);
    let source = GroupedSelectionSource::new(
        Source::new(vec![0, 1, 2, 3]),
        4,
        |a: &Assignment<i32>, l: &usize, r: &usize| a.values()[*l] == a.values()[*r],
    )
    .with_hash(|a: &Assignment<i32>, id: &usize| {
        hashes.set(hashes.get() + 1);
        a.values()[*id].unwrap() as u64
    });
    let mut provider = CachedMoveProvider::uniform(
        source,
        CachedProviderConfig::default(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.clone(),
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64, group: &SelectionGroup<usize>| {
            let ids = group.iter().map(|i| **i).collect::<Vec<_>>();
            groups.borrow_mut().push((step, ids.clone()));
            let value = a.values()[ids[0]].unwrap() + 1;
            (value <= 3).then(|| {
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
    let result = local_search_from_with_provider(
        &problem,
        &network,
        problem.initial_assignment(),
        LocalSearchConfig {
            max_steps: 10,
            selection: MoveSelection::BestAccepted,
            acceptance: Acceptance::HillClimbing,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(result.best.unwrap().assignment.values(), &[Some(3); 4]);
    assert_eq!(provider.cache().source().source().opens, 4);
    assert_eq!(hashes.get(), 16);
    assert_eq!(
        *groups.borrow(),
        [
            (0, vec![0, 1]),
            (0, vec![2]),
            (0, vec![3]),
            (1, vec![0, 1, 2]),
            (1, vec![3]),
            (2, vec![0, 1, 2, 3]),
            (3, vec![0, 1, 2, 3])
        ]
    );
}
