// Original native grouping and solver integration regressions.
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
struct Recorded {
    id: i32,
    log: Rc<RefCell<Vec<(i32, MoveProviderEvent)>>>,
    calls: Rc<RefCell<Vec<(i32, u64)>>>,
}
impl MoveProvider<(), (), i32> for Recorded {
    fn event(&mut self, event: MoveProviderEvent) {
        self.log.borrow_mut().push((self.id, event));
    }
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        _: &'a (),
        step: u64,
    ) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.calls.borrow_mut().push((self.id, step));
        Box::new([self.id, self.id].into_iter())
    }
}
#[test]
fn groups_compile_without_opening_leaves_and_preserve_order_events_and_snapshots() {
    let log = Rc::new(RefCell::new(vec![]));
    let calls = Rc::new(RefCell::new(vec![]));
    let make = |id| Recorded {
        id,
        log: log.clone(),
        calls: calls.clone(),
    };
    let (mut a, mut b, mut c) = (make(1), make(2), make(3));
    let group = MoveProviderGroup::group([
        MoveProviderGroup::group([]),
        MoveProviderGroup::leaf(&mut a),
        MoveProviderGroup::group([
            MoveProviderGroup::leaf(&mut b),
            MoveProviderGroup::group([MoveProviderGroup::leaf(&mut c)]),
        ]),
    ]);
    assert_eq!(group.leaf_count(), 3);
    let mut union = group.into_union(UnionOrder::Sequential).unwrap();
    assert!(calls.borrow().is_empty());
    assert!(log.borrow().is_empty());
    union.event(MoveProviderEvent::PhaseStarted);
    assert_eq!(
        *log.borrow(),
        vec![
            (1, MoveProviderEvent::PhaseStarted),
            (2, MoveProviderEvent::PhaseStarted),
            (3, MoveProviderEvent::PhaseStarted)
        ]
    );
    {
        let mut iter = union.moves(&(), &(), 7);
        assert!(calls.borrow().is_empty());
        assert_eq!(iter.next(), Some(1));
        assert_eq!(*calls.borrow(), vec![(1, 7)]);
        assert_eq!(iter.collect::<Vec<_>>(), [1, 2, 2, 3, 3]);
    }
    assert_eq!(*calls.borrow(), vec![(1, 7), (2, 7), (3, 7)]);
    assert_eq!(
        union.moves(&(), &(), 8).collect::<Vec<_>>(),
        [1, 1, 2, 2, 3, 3]
    );
    assert_eq!(&calls.borrow()[3..], &[(1, 8), (2, 8), (3, 8)]);
}
#[test]
fn empty_and_deep_groups_compile_without_recursive_traversal() {
    let empty = MoveProviderGroup::<(), (), i32>::group([MoveProviderGroup::group([])]);
    assert_eq!(empty.leaf_count(), 0);
    assert!(empty.into_union(UnionOrder::Sequential).is_err());
    let mut leaf = |_: &(), _: &(), _| vec![7];
    let mut group = MoveProviderGroup::leaf(&mut leaf);
    for _ in 0..10_000 {
        group = MoveProviderGroup::group([group]);
    }
    assert_eq!(group.leaf_count(), 1);
    let mut union = group.into_union(UnionOrder::Random { seed: 7 }).unwrap();
    assert_eq!(union.moves(&(), &(), 0).collect::<Vec<_>>(), [7]);
}
#[test]
fn regrouping_preserves_random_trace_and_exhaustion_reweights_live_leaves() {
    fn run(nested: bool) -> Vec<i32> {
        let mut a = |_: &(), _: &(), _| vec![0];
        let mut b = |_: &(), _: &(), _| vec![1; 5];
        let mut c = |_: &(), _: &(), _| vec![2; 11];
        let (a, b, c) = (
            MoveProviderGroup::leaf(&mut a),
            MoveProviderGroup::leaf(&mut b),
            MoveProviderGroup::leaf(&mut c),
        );
        let group = if nested {
            MoveProviderGroup::group([a, MoveProviderGroup::group([b, c])])
        } else {
            MoveProviderGroup::group([a, b, c])
        };
        let mut union = group.into_union(UnionOrder::Random { seed: 93 }).unwrap();
        union.moves(&(), &(), 0).collect()
    }
    let expected = run(false);
    assert_eq!(run(true), expected);
    assert_eq!(expected.len(), 17);
    assert_eq!(expected.iter().filter(|v| **v == 0).count(), 1);
    assert_eq!(expected.iter().filter(|v| **v == 1).count(), 5);
    assert_eq!(expected.iter().filter(|v| **v == 2).count(), 11);
}
#[test]
fn grouped_providers_drive_both_solver_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
    let mut network = ConstraintNetwork::new(2);
    for i in 0..2 {
        network
            .add_constraint(format!("value-{i}"), vec![i], |v| {
                SimpleScore::new(i64::from(*v[0].unwrap()))
            })
            .unwrap();
    }
    for stream in [false, true] {
        let mut a = |_: &FiniteProblem<i32>, state: &Assignment<i32>, _| {
            if state.values()[0].unwrap() < 2 {
                vec![Move::Change {
                    variable: 0,
                    value: Some(state.values()[0].unwrap() + 1),
                }]
            } else {
                vec![]
            }
        };
        let mut b = |_: &FiniteProblem<i32>, state: &Assignment<i32>, _| {
            if state.values()[1].unwrap() < 2 {
                vec![Move::Change {
                    variable: 1,
                    value: Some(state.values()[1].unwrap() + 1),
                }]
            } else {
                vec![]
            }
        };
        let group = MoveProviderGroup::group([
            MoveProviderGroup::leaf(&mut a),
            MoveProviderGroup::group([MoveProviderGroup::leaf(&mut b)]),
        ]);
        let mut provider = group.into_union(UnionOrder::Random { seed: 7 }).unwrap();
        let config = LocalSearchConfig {
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut builder = StreamGraphBuilder::new();
            let source = builder.source::<usize, (usize, Option<i32>)>();
            builder
                .score("value", source.port(), |(_, v)| {
                    SimpleScore::new(i64::from(v.unwrap()))
                })
                .unwrap();
            stream_local_search_from_with_provider(
                &p,
                builder.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(4));
        assert_eq!(best.assignment.values(), [Some(2), Some(2)]);
        assert_eq!(result.completed_steps, 4);
    }
}
