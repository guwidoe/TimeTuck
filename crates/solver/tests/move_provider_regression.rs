//! Original all-backend provider lifecycle, limits, errors and snapshot checks.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
}
fn change(source: usize, destination: usize) -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: source,
            index: 0,
        },
        destination: ListPosition {
            entity: destination,
            index: 0,
        },
    }
}
#[test]
fn providers_observe_committed_snapshots_and_replace_builtins_in_all_six_paths() {
    let p = basic();
    let l = lists();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| s(i64::from(*v[0].unwrap())))
        .unwrap();
    let mut ln = ListConstraintNetwork::new(3, 1);
    ln.add_constraint("owner", vec![ListScope::Value(0)], |f| {
        let ListFact::Value { element, .. } = &f[0] else {
            unreachable!()
        };
        s(element.as_ref().unwrap().position.entity as i64)
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 3, 1);
    mn.add_constraint(
        "both",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let (MixedFact::Basic(v), MixedFact::ListValue { element, .. }) = (&f[0], &f[1]) else {
                unreachable!()
            };
            s(i64::from(*v.unwrap()) + element.as_ref().unwrap().position.entity as i64)
        },
    )
    .unwrap();
    let config = LocalSearchConfig {
        neighborhood: Neighborhood::MassUnassign(MassAssignmentSize::all()),
        assert_scores: true,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut snapshots = vec![];
        let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, step| {
            let v = a.values()[0].unwrap();
            snapshots.push((step, v));
            if v < 2 {
                vec![Move::Change {
                    variable: 0,
                    value: Some(v + 1),
                }]
            } else {
                vec![]
            }
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |(_, v)| s(i64::from(v.unwrap())))
                .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &p,
                &bn,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(snapshots, vec![(0, 0), (1, 1), (2, 2)]);
        assert_eq!(r.best.unwrap().score, s(2));
        assert_eq!(r.completed_steps, 2);
        let mut snapshots = vec![];
        let mut provider = |_: &ListProblem<i32>, a: &ListAssignment<i32>, step| {
            let owner = a.location(&7).unwrap().entity;
            snapshots.push((step, owner));
            if owner < 2 {
                vec![change(owner, owner + 1)]
            } else {
                vec![]
            }
        };
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::MassListUnassign(ListSampleSize::all()),
            ..config
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = ListStreamSources::new(&mut b);
            b.score("owner", source.values.port(), |v| {
                s(v.position.unwrap().entity as i64)
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &l,
                b.build(),
                source,
                l.initial_assignment(),
                config.clone(),
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &l,
                &ln,
                l.initial_assignment(),
                config.clone(),
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(snapshots, vec![(0, 0), (1, 1), (2, 2)]);
        assert_eq!(r.best.unwrap().score, s(2));
        assert_eq!(r.completed_steps, 2);
        let mut snapshots = vec![];
        let mut provider = |_: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, step| {
            let v = a.basic().values()[0].unwrap();
            let owner = a.lists().location(&7).unwrap().entity;
            snapshots.push((step, v, owner));
            if v < 2 {
                vec![MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(v + 1),
                    },
                    lists: change(owner, owner + 1),
                }]
            } else {
                vec![]
            }
        };
        // A custom provider supplies joint moves even when built-in joint generation is off.
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: false,
        };
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            b.score("value", source.basic.port(), |(_, v)| {
                s(i64::from(v.unwrap()))
            })
            .unwrap();
            b.score("owner", source.lists.values.port(), |v| {
                s(v.position.unwrap().entity as i64)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &m,
                b.build(),
                source,
                m.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            mixed_local_search_from_with_provider(
                &m,
                &mn,
                m.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(snapshots, vec![(0, 0, 0), (1, 1, 1), (2, 2, 2)]);
        assert_eq!(r.best.unwrap().score, s(4));
        assert_eq!(r.completed_steps, 2);
    }
}
struct Repeating<M> {
    movement: M,
    pulls: Rc<Cell<usize>>,
    calls: usize,
}
impl<M> Repeating<M> {
    fn new(movement: M) -> Self {
        Self {
            movement,
            pulls: Rc::new(Cell::new(0)),
            calls: 0,
        }
    }
}
impl<P, A, M: Clone> MoveProvider<P, A, M> for Repeating<M> {
    fn moves<'a>(&'a mut self, _: &'a P, _: &'a A, _: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.calls += 1;
        let pulls = self.pulls.clone();
        Box::new(
            std::iter::repeat(self.movement.clone()).inspect(move |_| pulls.set(pulls.get() + 1)),
        )
    }
}
#[test]
fn infinite_providers_obey_budgets_prefetch_and_prestart_cancellation_on_all_paths() {
    let p = basic();
    let l = lists();
    let m = MixedProblem::new(p.clone(), l.clone());
    for mode in 0..4 {
        let token = CancellationToken::new();
        if mode == 2 {
            token.cancel();
        }
        let c = LocalSearchConfig {
            cancellation: Some(&token),
            max_evaluations: if mode == 0 { 0 } else { 100 },
            max_move_evaluations: Some(7),
            move_order: if mode == 3 {
                MoveOrder::ShuffledMoves {
                    seed: 3,
                    window: 4.try_into().unwrap(),
                }
            } else {
                MoveOrder::Original
            },
            ..Default::default()
        };
        macro_rules! check {
            ($result:expr,$provider:expr) => {{
                let r = $result.unwrap();
                assert_eq!(r.evaluations, if mode == 0 || mode == 2 { 0 } else { 8 });
                assert_eq!(r.completed_steps, 0);
                assert_eq!(
                    r.termination,
                    match mode {
                        0 => LocalSearchTermination::EvaluationLimit,
                        2 => LocalSearchTermination::Cancelled,
                        _ => LocalSearchTermination::MoveEvaluationLimit,
                    }
                );
                assert_eq!($provider.calls, usize::from(mode == 1 || mode == 3));
                assert_eq!(
                    $provider.pulls.get(),
                    match mode {
                        0 | 2 => 0,
                        1 => 8,
                        _ => 11,
                    }
                );
            }};
        }
        let mut provider = Repeating::new(Move::Composite(vec![]));
        check!(
            local_search_from_with_provider(
                &p,
                &ConstraintNetwork::<_, SimpleScore>::new(1),
                p.initial_assignment(),
                c.clone(),
                &mut provider
            ),
            provider
        );
        let mut provider = Repeating::new(Move::Composite(vec![]));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source();
        check!(
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                c.clone(),
                &mut provider
            ),
            provider
        );
        let mut provider = Repeating::new(ListMove::Composite(vec![]));
        check!(
            list_local_search_from_with_provider(
                &l,
                &ListConstraintNetwork::<_, SimpleScore>::new(3, 1),
                l.initial_assignment(),
                c.clone(),
                &mut provider
            ),
            provider
        );
        let mut provider = Repeating::new(ListMove::Composite(vec![]));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = ListStreamSources::new(&mut b);
        check!(
            list_stream_local_search_from_with_provider(
                &l,
                b.build(),
                source,
                l.initial_assignment(),
                c.clone(),
                &mut provider
            ),
            provider
        );
        let c = MixedLocalSearchConfig {
            search: c,
            joint_moves: true,
        };
        let mut provider = Repeating::new(MixedMove {
            basic: Move::Composite(vec![]),
            lists: ListMove::Composite(vec![]),
        });
        check!(
            mixed_local_search_from_with_provider(
                &m,
                &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 3, 1),
                m.initial_assignment(),
                c.clone(),
                &mut provider
            ),
            provider
        );
        let mut provider = Repeating::new(MixedMove {
            basic: Move::Composite(vec![]),
            lists: ListMove::Composite(vec![]),
        });
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = MixedStreamSources::new(&mut b);
        check!(
            mixed_stream_local_search_from_with_provider(
                &m,
                b.build(),
                source,
                m.initial_assignment(),
                c,
                &mut provider
            ),
            provider
        );
    }
}
