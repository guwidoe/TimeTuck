use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
struct Source<T>(Vec<T>);
impl<C, T: Clone> SelectionSource<C, T> for Source<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.0.iter().cloned())
    }
}
#[test]
fn small_products_match_independent_lazy_enumeration_for_both_empty_policies() {
    for dimensions in 0..=4 {
        for encoded in 0..4usize.pow(dimensions) {
            let mut code = encoded;
            let choices = (0..dimensions)
                .map(|_| {
                    let len = code % 4;
                    code /= 4;
                    (0..len).collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            for ignore in [false, true] {
                let reference =
                    MoveProduct::new(choices.len(), ignore, |i| choices[i].iter().copied())
                        .collect::<Vec<_>>();
                let mut product = SelectionProduct::<(), _, _>::new(
                    choices.iter().cloned().map(Source).collect(),
                    ignore,
                    3,
                );
                let snapshot = product.snapshot(&()).unwrap();
                assert_eq!(snapshot.cardinality(), reference.len() as u128);
                let mut iter = snapshot.iter();
                assert_eq!(iter.size_hint(), (reference.len(), Some(reference.len())));
                for (i, expected) in reference.iter().enumerate() {
                    let row = iter.next().unwrap();
                    assert_eq!(row.iter().map(|v| **v).collect::<Vec<_>>(), *expected);
                    assert_eq!(
                        snapshot
                            .get(i as u128)
                            .unwrap()
                            .iter()
                            .map(|v| **v)
                            .collect::<Vec<_>>(),
                        *expected
                    );
                }
                assert_eq!(iter.next(), None);
                assert_eq!(iter.next(), None);
                assert_eq!(snapshot.get(reference.len() as u128), None);
            }
        }
    }
}
#[test]
fn huge_cardinality_is_indexed_without_materializing_rows_and_overflow_is_explicit() {
    let mut product =
        SelectionProduct::<(), _, _>::new((0..100).map(|_| Source(vec![0, 1])).collect(), false, 2);
    let snapshot = product.snapshot(&()).unwrap();
    assert_eq!(snapshot.cardinality(), 1u128 << 100);
    assert!(snapshot.get(0).unwrap().iter().all(|v| **v == 0));
    assert!(
        snapshot
            .get((1u128 << 100) - 1)
            .unwrap()
            .iter()
            .all(|v| **v == 1)
    );
    assert_eq!(snapshot.iter().size_hint(), (usize::MAX, None));
    let mut overflow =
        SelectionProduct::<(), _, _>::new((0..128).map(|_| Source(vec![0, 1])).collect(), false, 2);
    assert!(matches!(
        overflow.snapshot(&()),
        Err(SelectionProductError::CardinalityOverflow)
    ));
}
struct Bounded {
    declared: Option<usize>,
    repeating: bool,
    actual: usize,
    opens: Rc<Cell<usize>>,
    reads: Rc<Cell<usize>>,
}
impl SelectionSource<(), usize> for Bounded {
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        Box::new((0..self.actual).inspect(|_| self.reads.set(self.reads.get() + 1)))
    }
}
#[test]
fn child_limits_metadata_errors_and_unbounded_sources_are_checked() {
    for (declared, repeating, actual, opens, reads, error) in [
        (
            None,
            false,
            usize::MAX,
            1,
            4,
            SelectionProductError::CandidateLimit {
                child: 0,
                maximum: 3,
            },
        ),
        (
            Some(10),
            false,
            10,
            0,
            0,
            SelectionProductError::CandidateLimit {
                child: 0,
                maximum: 3,
            },
        ),
        (
            Some(2),
            false,
            3,
            1,
            3,
            SelectionProductError::SizeMismatch {
                child: 0,
                declared: 2,
                actual: 3,
            },
        ),
        (
            Some(1),
            true,
            usize::MAX,
            0,
            0,
            SelectionProductError::UnboundedChild { child: 0 },
        ),
    ] {
        let open_count = Rc::new(Cell::new(0));
        let read_count = Rc::new(Cell::new(0));
        let source = Bounded {
            declared,
            repeating,
            actual,
            opens: open_count.clone(),
            reads: read_count.clone(),
        };
        let mut product = SelectionProduct::new(vec![source], false, 3);
        assert!(matches!(product.snapshot(&()),Err(e)if e==error));
        assert_eq!(open_count.get(), opens);
        assert_eq!(read_count.get(), reads);
    }
}
struct Item {
    value: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for Item {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Once(Option<Vec<Item>>);
impl SelectionSource<(), Item> for Once {
    fn size(&mut self, _: &()) -> Option<usize> {
        self.0.as_ref().map(Vec::len)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Item> + 'a>
    where
        Item: 'a,
    {
        Box::new(self.0.take().unwrap().into_iter())
    }
}
#[test]
fn snapshots_share_nonclone_occurrences_and_survive_the_source() {
    let drops = Rc::new(Cell::new(0));
    let item = |value| Item {
        value,
        drops: drops.clone(),
    };
    let mut product = SelectionProduct::new(
        vec![
            Once(Some(vec![item(1), item(1)])),
            Once(Some(vec![item(9)])),
        ],
        false,
        2,
    );
    let snapshot = product.snapshot(&()).unwrap();
    drop(product);
    let copy = snapshot.clone();
    let a = snapshot.get(0).unwrap();
    let b = snapshot.get(1).unwrap();
    assert_eq!(a[0].value, b[0].value);
    assert!(!Arc::ptr_eq(&a[0], &b[0]));
    assert!(Arc::ptr_eq(&a[1], &b[1]));
    drop(snapshot);
    drop(copy);
    assert_eq!(drops.get(), 0);
    drop(a);
    assert_eq!(drops.get(), 1);
    drop(b);
    assert_eq!(drops.get(), 3);
}
#[test]
fn sampling_preserves_child_indices_and_seed_reproducibility_without_touching_empty_products() {
    let mut product = SelectionProduct::<(), _, _>::new(
        vec![
            Source(vec![]),
            Source(vec![1, 2]),
            Source(vec![]),
            Source(vec![8, 9, 10]),
        ],
        true,
        3,
    );
    let snapshot = product.snapshot(&()).unwrap();
    let mut calls = vec![];
    let row = snapshot
        .try_sample_with(|child, bound| {
            calls.push((child, bound));
            bound - 1
        })
        .unwrap()
        .unwrap();
    assert_eq!(calls, [(1, 2), (3, 3)]);
    assert_eq!(row.iter().map(|v| **v).collect::<Vec<_>>(), [2, 10]);
    assert!(matches!(
        snapshot.try_sample_with(|_, n| n),
        Err(SelectionProductError::InvalidDraw {
            child: 1,
            index: 2,
            bound: 2
        })
    ));
    let draws = |seed| {
        snapshot
            .samples(seed)
            .take(30)
            .map(|row| row.iter().map(|v| **v).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    assert_eq!(draws(7), draws(7));
    assert_ne!(draws(7), draws(8));
    let mut empty = SelectionProduct::<(), usize, Source<usize>>::new(vec![], true, 0);
    let snapshot = empty.snapshot(&()).unwrap();
    let mut iter = snapshot.samples_with(|_, _| panic!("empty draw"));
    assert!(iter.is_never_ending());
    assert!(!iter.has_next());
    assert_eq!(iter.next(), None);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}
#[test]
fn original_and_random_products_generate_composite_moves_for_both_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2, 3]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(0),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap() + *v[1].unwrap()))
    })
    .unwrap();
    for random in [false, true] {
        for stream in [false, true] {
            let children = vec![
                Source(
                    (1..=3)
                        .map(|v| Move::Change {
                            variable: 0,
                            value: Some(v),
                        })
                        .collect(),
                ),
                Source(
                    (1..=2)
                        .map(|v| Move::Change {
                            variable: 1,
                            value: Some(v),
                        })
                        .collect(),
                ),
            ];
            let mut product = SelectionProduct::<(), _, _>::new(children, false, 3);
            let mut provider = move |_: &FiniteProblem<i32>, _: &Assignment<i32>, step: u64| {
                product.event(SelectionLifecycleEvent::StepStarted, &());
                let snapshot = product.snapshot(&()).unwrap();
                let rows = if random {
                    snapshot.samples(step + 5).take(100).collect::<Vec<_>>()
                } else {
                    snapshot.iter().collect()
                };
                let moves = rows
                    .into_iter()
                    .map(|row| Move::Composite(row.into_iter().map(|m| (*m).clone()).collect()))
                    .collect();
                product.event(SelectionLifecycleEvent::StepEnded, &());
                moves
            };
            let config = LocalSearchConfig {
                max_steps: 1,
                max_move_evaluations: Some(120),
                selection: MoveSelection::BestAccepted,
                assert_scores: true,
                ..Default::default()
            };
            let result = if stream {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                b.score("sum", source.port(), |r| {
                    SimpleScore::new(i64::from(r.1.unwrap()))
                })
                .unwrap();
                stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            } else {
                local_search_from_with_provider(
                    &p,
                    &n,
                    p.initial_assignment(),
                    config,
                    &mut provider,
                )
                .unwrap()
            };
            assert_eq!(result.best.unwrap().score, SimpleScore::new(5));
        }
    }
}
