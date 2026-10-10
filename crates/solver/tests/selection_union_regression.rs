use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[test]
fn finite_interleavings_preserve_occurrences_and_each_child_order() {
    for counts in [[0, 0, 0], [1, 2, 3], [5, 0, 2], [3, 3, 3]] {
        for seed in 0..50 {
            let children = counts
                .iter()
                .enumerate()
                .map(|(child, &count)| {
                    (
                        (0..count)
                            .map(move |i| (child, i))
                            .collect::<Vec<_>>()
                            .into_iter(),
                        [0.5, 2.0, 7.25][child],
                    )
                })
                .collect();
            let mut draw = SampleRandom::new(seed);
            let mut actual = WeightedInterleave::new(children, move || {
                draw.index(std::num::NonZeroUsize::new(1024).unwrap()) as f64 / 1024.0
            })
            .unwrap();
            let rows = actual.by_ref().collect::<Result<Vec<_>, _>>().unwrap();
            assert_eq!(rows.len(), counts.iter().sum::<usize>());
            for (child, &count) in counts.iter().enumerate() {
                assert_eq!(
                    rows.iter()
                        .filter(|(c, _)| *c == child)
                        .map(|(_, i)| *i)
                        .collect::<Vec<_>>(),
                    (0..count).collect::<Vec<_>>()
                );
            }
            assert!(actual.next().is_none());
        }
    }
}
#[test]
fn fractional_boundaries_retire_empty_families_without_extra_draws() {
    let calls = Cell::new(0);
    let mut draws = [0.0, 0.25, 0.999, 0.5].into_iter();
    // Adding a tiny weight after a huge cumulative weight loses precision.
    assert!(matches!(
        WeightedInterleave::new(vec![(0..0, f64::MAX), (0..2, 0.25)], || 0.0),
        Err(WeightedSelectionError::PrecisionLoss { index: 1 })
    ));
    let mut cursor = WeightedInterleave::new(
        vec![
            (vec![1, 2].into_iter(), 0.25),
            (vec![3, 4].into_iter(), 0.75),
            (vec![99].into_iter(), 0.0),
            (vec![].into_iter(), 2.0),
        ],
        || {
            calls.set(calls.get() + 1);
            draws.next().unwrap()
        },
    )
    .unwrap();
    assert_eq!(
        cursor.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        [1, 3, 4, 2]
    );
    assert_eq!(calls.get(), 4);
    assert!(cursor.next().is_none());
}
#[test]
fn nonclone_streams_are_lazy_bounded_and_never_resurrect_after_exhaustion() {
    struct Item(usize);
    let polls = Rc::new(Cell::new(0));
    let p = polls.clone();
    let a = std::iter::from_fn(move || {
        p.set(p.get() + 1);
        match p.get() {
            1 => Some(Item(1)),
            2 => None,
            _ => panic!("resurrected"),
        }
    });
    let disabled = std::iter::from_fn(|| -> Option<Item> { panic!("disabled child polled") });
    let b = std::iter::repeat_with(|| Item(2));
    let children: Vec<(Box<dyn Iterator<Item = Item>>, f64)> = vec![
        (Box::new(a), 1.0),
        (Box::new(disabled), 0.0),
        (Box::new(b), 1.0),
    ];
    let mut cursor = WeightedInterleave::new(children, || 0.0).unwrap();
    assert_eq!(polls.get(), 0);
    assert_eq!(
        cursor
            .by_ref()
            .take(20)
            .map(|v| v.unwrap().0)
            .collect::<Vec<_>>(),
        [vec![1], vec![2; 19]].concat()
    );
    assert_eq!(polls.get(), 2);
}
#[test]
fn invalid_weights_and_draws_fail_explicitly() {
    for weight in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            WeightedInterleave::new(vec![(0..1, weight)], || 0.0),
            Err(WeightedSelectionError::InvalidWeight { index: 0 })
        ));
    }
    for draw in [-0.1, 1.0, f64::NAN, f64::INFINITY] {
        let mut cursor = WeightedInterleave::new(vec![(0..3, 1.0)], move || draw).unwrap();
        assert_eq!(
            cursor.next(),
            Some(Err(WeightedSelectionError::InvalidDraw))
        );
        assert_eq!(cursor.next(), None);
    }
    let mut empty = WeightedInterleave::new(vec![(0..0, 1.0)], || panic!("empty draw")).unwrap();
    assert_eq!(empty.next(), None);
}
struct Source {
    values: Vec<i32>,
    opens: Rc<Cell<usize>>,
    events: usize,
    size: Option<usize>,
    infinite: bool,
}
impl SelectionSource<(), i32> for Source {
    fn event(&mut self, _: SelectionLifecycleEvent, _: &()) {
        self.events += 1;
    }
    fn is_never_ending(&self) -> bool {
        self.infinite
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        self.size
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        Box::new(self.values.iter().copied())
    }
}
fn source(values: Vec<i32>, size: Option<usize>, infinite: bool) -> Source {
    Source {
        values,
        opens: Rc::new(Cell::new(0)),
        events: 0,
        size,
        infinite,
    }
}
#[test]
fn source_lifecycle_metadata_disabled_and_early_drop_contracts() {
    let a = source(vec![1, 2], Some(2), false);
    let ac = a.opens.clone();
    let b = source(vec![3], Some(1), false);
    let bc = b.opens.clone();
    let mut union =
        SelectionUnion::new(vec![(a, 1.0), (b, 1.0), (source(vec![], None, true), 0.0)]).unwrap();
    assert_eq!(union.size(&()), Some(3));
    assert!(!union.is_never_ending());
    union.event(SelectionLifecycleEvent::StepStarted, &());
    {
        let mut cursor = union.ordered(&());
        assert_eq!(ac.get(), 0);
        assert_eq!(cursor.next(), Some(1));
        assert_eq!(bc.get(), 0);
    }
    assert_eq!(union.ordered(&()).collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(ac.get(), 2);
    assert_eq!(bc.get(), 1);
    assert!(union.children().all(|c| c.events == 1));
    let mut overflowing = SelectionUnion::new(vec![
        (source(vec![], Some(usize::MAX), false), 1.0),
        (source(vec![], Some(1), false), 1.0),
    ])
    .unwrap();
    assert_eq!(overflowing.size(&()), None);
    let mut unknown = SelectionUnion::new(vec![(source(vec![], None, true), 1.0)]).unwrap();
    assert_eq!(unknown.size(&()), None);
    assert!(unknown.is_never_ending());
}
#[test]
fn seeded_source_interleaving_repeats_and_integrates_with_both_basic_backends() {
    let mut union = SelectionUnion::new(vec![
        (source(vec![1, 2, 3], Some(3), false), 0.3),
        (source(vec![4, 5], Some(2), false), 0.7),
    ])
    .unwrap();
    let first = union
        .random(&(), 55)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        first,
        union
            .random(&(), 55)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            let children = vec![
                (
                    vec![
                        Move::Change {
                            variable: 0,
                            value: Some(1),
                        },
                        Move::Change {
                            variable: 0,
                            value: Some(3),
                        },
                    ]
                    .into_iter(),
                    0.7,
                ),
                (
                    vec![Move::Change {
                        variable: 0,
                        value: Some(2),
                    }]
                    .into_iter(),
                    0.3,
                ),
            ];
            WeightedInterleave::new(children, || 0.9)
                .unwrap()
                .map(Result::unwrap)
                .filter(|m| matches!(m,Move::Change{value,..} if value!=&a.values()[0]))
                .collect::<Vec<_>>()
        };
        let config = LocalSearchConfig {
            max_steps: 2,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
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
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(3));
    }
}
