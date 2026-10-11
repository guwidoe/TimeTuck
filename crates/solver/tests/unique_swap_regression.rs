//! Original regressions for unique pair enumeration and solver integration.
use std::{collections::BTreeSet, marker::PhantomData};
use timetuck::*;

#[test]
fn basic_pairs_retain_first_encounter_orientation_and_apply_legally() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
        Variable::new(vec![0, 1, 2])
            .with_initial(2)
            .with_pinned(true),
        Variable::new(vec![0, 1]).allowing_unassigned(),
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![1, 2]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    for order in [
        vec![0, 1, 2, 3, 4, 5],
        vec![5, 3, 1, 4, 2, 0],
        vec![4, 3, 0],
        vec![],
    ] {
        let collect = |neighborhood| {
            basic_moves_configured(
                &p,
                &a,
                BasicNeighborhood {
                    neighborhood,
                    variables: Some(&order),
                    ..Default::default()
                },
            )
            .unwrap()
            .collect::<Vec<_>>()
        };
        let mut seen = BTreeSet::new();
        let expected: Vec<_> = collect(Neighborhood::Swap)
            .into_iter()
            .filter(|m| {
                let Move::Swap { left, right } = m else {
                    panic!("not a swap")
                };
                seen.insert(((*left).min(*right), (*left).max(*right)))
            })
            .collect();
        let unique = collect(Neighborhood::UniqueSwap);
        assert_eq!(unique, expected);
        for m in unique {
            let mut trial = a.clone();
            p.apply(&mut trial, &m).unwrap().commit();
            p.validate(&trial).unwrap();
            assert_eq!(trial.values()[2], a.values()[2]);
        }
        let combined = collect(Neighborhood::ChangeAndUniqueSwap);
        let mut expected = collect(Neighborhood::Change);
        expected.extend(collect(Neighborhood::UniqueSwap));
        assert_eq!(combined, expected);
    }
    assert_eq!(
        unique_swap_moves(&p, &a).unwrap().count(),
        swap_moves(&p, &a).unwrap().count() / 2
    );
}

#[test]
fn list_pairs_preserve_pins_ranges_and_optional_replacements() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 3, 4, 6]),
            ListEntity::new(vec![2, 3, 4])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2, 3, 4, 7]),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut seen = BTreeSet::new();
    let expected: Vec<_> = list_swap_moves(&p, &a)
        .unwrap()
        .filter(|m| match m {
            ListMove::Swap { left, right } => {
                let (l, r) = ((left.entity, left.index), (right.entity, right.index));
                seen.insert((l.min(r), l.max(r)))
            }
            ListMove::Composite(_) => true,
            _ => panic!("unexpected move"),
        })
        .collect();
    let unique = list_unique_swap_moves(&p, &a).unwrap().collect::<Vec<_>>();
    assert_eq!(unique, expected);
    assert!(unique.iter().any(|m| matches!(m, ListMove::Swap { .. })));
    assert!(unique.iter().any(|m| matches!(m, ListMove::Composite(_))));
    for m in &unique {
        let mut trial = a.clone();
        p.apply(&mut trial, m).unwrap().commit();
        p.validate(&trial).unwrap();
        assert_eq!(trial.lists()[0][0], 0);
        assert_eq!(trial.lists()[1][0], 2);
        assert_eq!(trial.lists()[2], vec![5]);
    }
    assert_eq!(
        list_moves(&p, &a, Neighborhood::UniqueSwap)
            .unwrap()
            .collect::<Vec<_>>(),
        unique
    );
    let mut all = list_moves(&p, &a, Neighborhood::Change)
        .unwrap()
        .collect::<Vec<_>>();
    all.extend(unique);
    assert_eq!(
        list_moves(&p, &a, Neighborhood::ChangeAndUniqueSwap)
            .unwrap()
            .collect::<Vec<_>>(),
        all
    );
}

trait TestAssignment {
    type Change;
    fn score(&self) -> i64;
}
impl TestAssignment for Assignment<usize> {
    type Change = Vec<usize>;
    fn score(&self) -> i64 {
        self.values()
            .iter()
            .enumerate()
            .map(|(i, v)| i64::from(*v == Some(1 - i)))
            .sum()
    }
}
impl TestAssignment for ListAssignment<usize> {
    type Change = Vec<usize>;
    fn score(&self) -> i64 {
        self.lists()
            .iter()
            .enumerate()
            .flat_map(|(i, l)| l.iter().map(move |v| i64::from(i == 1 - v)))
            .sum()
    }
}
impl TestAssignment for MixedAssignment<usize, usize> {
    type Change = MixedCalculatorChange;
    fn score(&self) -> i64 {
        self.basic().score() + self.lists().score()
    }
}
struct Calculator<A>(i64, PhantomData<A>);
impl<A: TestAssignment> IncrementalCalculator<A> for Calculator<A> {
    type Score = SimpleScore;
    type Change = A::Change;
    type Justification = ();
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = a.score();
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &A::Change,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
fn s(v: i64) -> SimpleScore {
    SimpleScore::new(v)
}
fn list_score(f: &ListFact<'_, usize>) -> i64 {
    match f {
        ListFact::Value { value, element } => {
            i64::from(element.as_ref().unwrap().position.entity == 1 - **value)
        }
        _ => unreachable!(),
    }
}
fn assert_result(
    score: SimpleScore,
    steps: u64,
    evaluations: u64,
    termination: LocalSearchTermination,
    want_score: i64,
    candidates: u64,
) {
    assert_eq!(score, s(want_score));
    assert_eq!(steps, 3);
    assert_eq!(evaluations, 1 + 3 * candidates);
    assert_eq!(termination, LocalSearchTermination::StepLimit);
}
#[test]
fn unique_swaps_reduce_evaluations_across_all_nine_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![1])],
    )
    .unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(2);
    bn.add_constraint("target", vec![0, 1], |v| {
        s(i64::from(v[0] == Some(&1)) + i64::from(v[1] == Some(&0)))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(2, 2);
    ln.add_constraint(
        "target",
        vec![ListScope::Value(0), ListScope::Value(1)],
        |f| s(f.iter().map(list_score).sum()),
    )
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(2, 2, 2);
    mn.add_constraint(
        "target",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListValue(0),
            MixedScope::ListValue(1),
        ],
        |f| {
            s(f.iter()
                .enumerate()
                .map(|(i, f)| match f {
                    MixedFact::Basic(v) => i64::from(*v == Some(&(1 - i))),
                    MixedFact::ListValue { value, element } => {
                        i64::from(element.as_ref().unwrap().position.entity == 1 - **value)
                    }
                    _ => unreachable!(),
                })
                .sum())
        },
    )
    .unwrap();
    for (neighborhood, candidates, union) in [
        (Neighborhood::Swap, 2, false),
        (Neighborhood::UniqueSwap, 1, false),
        (Neighborhood::UniqueSwap, 1, true),
    ] {
        let children = [(Neighborhood::Change, 0), (neighborhood.clone(), 1)];
        let families = NeighborhoodUnion {
            children: &children,
            order: UnionOrder::Sequential,
        };
        let config = LocalSearchConfig {
            neighborhood_union: union.then_some(&families),
            neighborhood,
            acceptance: Acceptance::HillClimbing,
            rejected_moves: RejectedMovePolicy::BestScore,
            max_steps: 3,
            selection: MoveSelection::BestAccepted,
            assert_scores: true,
            ..Default::default()
        };
        let b = local_search_from(&p, &bn, p.initial_assignment(), config.clone()).unwrap();
        let bc = calculator_local_search_from(
            &p,
            p.initial_assignment(),
            || Calculator::<Assignment<usize>>(0, PhantomData),
            config.clone(),
        )
        .unwrap();
        let mut g = StreamGraphBuilder::new();
        let source = g.source::<usize, (usize, Option<usize>)>();
        g.score("target", source.port(), |(i, v)| {
            s(i64::from(*v == Some(1 - i)))
        })
        .unwrap();
        let bs = stream_local_search_from(
            &p,
            g.build(),
            source,
            p.initial_assignment(),
            config.clone(),
        )
        .unwrap();
        for r in [b, bc, bs] {
            assert_result(
                r.best.unwrap().score,
                r.completed_steps,
                r.evaluations,
                r.termination,
                2,
                candidates,
            );
        }
        let lr = list_local_search_from(&l, &ln, l.initial_assignment(), config.clone()).unwrap();
        let lc = list_calculator_local_search_from(
            &l,
            l.initial_assignment(),
            || Calculator::<ListAssignment<usize>>(0, PhantomData),
            config.clone(),
        )
        .unwrap();
        let mut g = StreamGraphBuilder::new();
        let source = ListStreamSources::<usize>::new(&mut g);
        g.score("target", source.values.port(), |v| {
            s(i64::from(v.position.unwrap().entity == 1 - v.value))
        })
        .unwrap();
        let ls = list_stream_local_search_from(
            &l,
            g.build(),
            source,
            l.initial_assignment(),
            config.clone(),
        )
        .unwrap();
        for r in [lr, lc, ls] {
            assert_result(
                r.best.unwrap().score,
                r.completed_steps,
                r.evaluations,
                r.termination,
                2,
                candidates,
            );
        }
        let config = MixedLocalSearchConfig {
            search: config,
            joint_moves: true,
        };
        let mr = mixed_local_search_from(&m, &mn, m.initial_assignment(), config.clone()).unwrap();
        let mc = mixed_calculator_local_search_from(
            &m,
            m.initial_assignment(),
            || Calculator::<MixedAssignment<usize, usize>>(0, PhantomData),
            config.clone(),
        )
        .unwrap();
        let mut g = StreamGraphBuilder::new();
        let source = MixedStreamSources::<usize, usize>::new(&mut g);
        g.score("basic", source.basic.port(), |(i, v)| {
            s(i64::from(*v == Some(1 - i)))
        })
        .unwrap();
        g.score("list", source.lists.values.port(), |v| {
            s(i64::from(v.position.unwrap().entity == 1 - v.value))
        })
        .unwrap();
        let ms =
            mixed_stream_local_search_from(&m, g.build(), source, m.initial_assignment(), config)
                .unwrap();
        for r in [mr, mc, ms] {
            assert_result(
                r.best.unwrap().score,
                r.completed_steps,
                r.evaluations,
                r.termination,
                4,
                2 * candidates + candidates * candidates,
            );
        }
    }
}

#[test]
fn combined_unique_family_respects_selected_value_order() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let values = [BasicValueOrder::new(0, vec![Some(2), Some(1), Some(0)])];
    let actual = basic_moves_configured(
        &p,
        &a,
        BasicNeighborhood {
            neighborhood: Neighborhood::ChangeAndUniqueSwap,
            variables: Some(&[1, 0]),
            value_orders: &values,
            ..Default::default()
        },
    )
    .unwrap()
    .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            Move::Change {
                variable: 1,
                value: Some(0)
            },
            Move::Change {
                variable: 1,
                value: Some(2)
            },
            Move::Change {
                variable: 0,
                value: Some(2)
            },
            Move::Change {
                variable: 0,
                value: Some(1)
            },
            Move::Swap { left: 1, right: 0 }
        ]
    );
    for variables in [
        vec![],
        vec![Variable::new(vec![1]).with_initial(1)],
        vec![
            Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
            Variable::new(vec![1, 2]).with_initial(2),
        ],
    ] {
        let p = FiniteProblem::new(variables).unwrap();
        let a = p.initial_assignment();
        let mut moves = unique_swap_moves(&p, &a).unwrap();
        assert!(moves.next().is_none());
        assert!(moves.next().is_none());
    }
}
