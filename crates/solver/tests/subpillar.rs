//! Original finite subset enumeration and search regressions.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;

#[test]
fn bounded_subsets_match_bitmask_oracle_including_consecutive_order_and_large_bounds() {
    for n in 1..=7 {
        let pillar = Pillar::new((0..n).rev()).unwrap();
        for min in 1..=n + 1 {
            for max in [min, n + 1, usize::MAX] {
                for consecutive in [false, true] {
                    let selection = if consecutive {
                        SubPillarSelection::consecutive(min, max)
                    } else {
                        SubPillarSelection::combinations(min, max)
                    }
                    .unwrap();
                    assert_eq!(
                        (
                            selection.minimum(),
                            selection.maximum(),
                            selection.is_consecutive()
                        ),
                        (min, max, consecutive)
                    );
                    let actual: Vec<_> = pillar
                        .subpillars(selection)
                        .map(|p| p.variables())
                        .collect();
                    let mut expected: Vec<Vec<usize>> = (1..1usize << n)
                        .map(|mask| (0..n).filter(|i| mask & (1 << i) != 0).collect())
                        .filter(|v: &Vec<usize>| {
                            v.len() >= min
                                && v.len() <= max
                                && (!consecutive || v.windows(2).all(|w| w[1] == w[0] + 1))
                        })
                        .collect();
                    expected.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
                    assert_eq!(actual, expected);
                }
            }
        }
    }
    for (min, max) in [(0, 0), (0, 1), (2, 1), (1, 0)] {
        assert!(SubPillarSelection::combinations(min, max).is_err());
        assert!(SubPillarSelection::consecutive(min, max).is_err());
    }
    let pillar = Pillar::from_rows(vec![vec![8, 2], vec![5, 1], vec![10, 0]]).unwrap();
    let rows: Vec<_> = pillar
        .subpillars(SubPillarSelection::consecutive(2, 2).unwrap())
        .map(|p| p.rows().to_vec())
        .collect();
    assert_eq!(
        rows,
        vec![vec![vec![5, 1], vec![8, 2]], vec![vec![8, 2], vec![10, 0]]]
    );
    let large = Pillar::new(0..128).unwrap();
    assert_eq!(
        large
            .subpillars(SubPillarSelection::combinations(64, usize::MAX).unwrap())
            .next()
            .unwrap()
            .variables(),
        (0..64).collect::<Vec<_>>()
    );
    let mut none = large.subpillars(SubPillarSelection::combinations(129, usize::MAX).unwrap());
    assert!(none.next().is_none());
    assert!(none.next().is_none());
}

fn subset(mask: usize, a: &[Option<i32>]) -> Vec<usize> {
    (0..a.len()).filter(|i| mask & (1 << i) != 0).collect()
}
#[test]
fn neighborhood_matches_independent_successor_oracle_for_ranges_pins_nulls_and_asymmetric_swaps() {
    for x in [None, Some(0), Some(1)] {
        for y in [None, Some(1), Some(2)] {
            let p = FiniteProblem::new(vec![
                Variable::new(vec![0, 1]).allowing_unassigned(),
                Variable::new(vec![1, 2]).allowing_unassigned(),
                Variable::new(vec![0, 1, 2]).allowing_unassigned(),
                Variable::new(vec![0, 1, 2])
                    .with_initial(1)
                    .with_pinned(true),
            ])
            .unwrap();
            let a = Assignment::new(vec![x, y, x, Some(1)]);
            for mode in [
                PillarChangeMode::Disabled,
                PillarChangeMode::AssignedOnly,
                PillarChangeMode::All,
                PillarChangeMode::UnassignOnly,
            ] {
                for right_max in [0, 1, 2] {
                    let config = SubPillarConfig {
                        selection: SubPillarSelection::combinations(1, 2).unwrap(),
                        changes: mode,
                        swap_selection: (right_max != 0)
                            .then(|| SubPillarSelection::combinations(1, right_max).unwrap()),
                    };
                    let moves: Vec<_> = subpillar_moves(&p, &a, config).unwrap().collect();
                    let mut actual = BTreeSet::new();
                    for m in &moves {
                        let mut state = a.clone();
                        p.apply(&mut state, m).unwrap().commit();
                        assert_ne!(state, a);
                        assert!(p.is_complete(&state));
                        actual.insert(state.values().to_vec());
                    }
                    let mut expected = BTreeSet::new();
                    for mask in 1..8 {
                        let l = subset(mask, a.values());
                        if l.len() > 2 || !l.iter().all(|&i| a.values()[i] == a.values()[l[0]]) {
                            continue;
                        }
                        for target in [None, Some(0), Some(1), Some(2)] {
                            if mode == PillarChangeMode::Disabled
                                || (target.is_none() && mode == PillarChangeMode::AssignedOnly)
                                || (target.is_some() && mode == PillarChangeMode::UnassignOnly)
                            {
                                continue;
                            }
                            let mut state = a.values().to_vec();
                            for &i in &l {
                                state[i] = target;
                            }
                            if state != a.values()
                                && p.validate(&Assignment::new(state.clone())).is_ok()
                            {
                                expected.insert(state);
                            }
                        }
                        for other in 1..8 {
                            let r = subset(other, a.values());
                            if r.len() > right_max
                                || mask & other != 0
                                || !r.iter().all(|&i| a.values()[i] == a.values()[r[0]])
                                || a.values()[l[0]] == a.values()[r[0]]
                            {
                                continue;
                            }
                            let mut state = a.values().to_vec();
                            for &i in &l {
                                state[i] = a.values()[r[0]];
                            }
                            for &i in &r {
                                state[i] = a.values()[l[0]];
                            }
                            if p.validate(&Assignment::new(state.clone())).is_ok() {
                                expected.insert(state);
                            }
                        }
                    }
                    assert_eq!(actual, expected);
                }
            }
        }
    }
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    let config = SubPillarConfig::new(SubPillarSelection::combinations(1, usize::MAX).unwrap());
    assert_eq!(
        subpillar_moves(&empty, &empty.initial_assignment(), config)
            .unwrap()
            .count(),
        0
    );
    assert!(subpillar_moves(&empty, &Assignment::new(vec![Some(1)]), config).is_err());
}

fn score(v: &[Option<i32>]) -> SimpleScore {
    SimpleScore::new(if v == [Some(0); 3] {
        0
    } else if v == [Some(1), Some(0), Some(1)] {
        10
    } else {
        -5
    })
}
fn graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    let zero = b.filter(s.port(), |v| v.0 == 0).unwrap();
    let one = b.filter(s.port(), |v| v.0 == 1).unwrap();
    let two = b.filter(s.port(), |v| v.0 == 2).unwrap();
    let pair = b.join_equal(&zero, &one, |_| 0, |_| 0).unwrap();
    let triple = b.join_equal(&pair, &two, |_| 0, |_| 0).unwrap();
    b.score("trap", &triple, |((a, b), c)| score(&[a.1, b.1, c.1]))
        .unwrap();
    (b.build(), s)
}
#[test]
fn partial_groups_escape_whole_group_trap_in_both_backends_with_limits_and_seeded_orders() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("trap", vec![0, 1, 2], |v| {
        score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::Pillar { swaps: true },
        Neighborhood::SubPillar(SubPillarConfig::new(
            SubPillarSelection::combinations(2, 2).unwrap(),
        )),
    ] {
        for order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 23 },
            MoveOrder::ShuffledMoves {
                seed: 23,
                window: 2.try_into().unwrap(),
            },
        ] {
            for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
                for cap in [1, 100] {
                    let c = LocalSearchConfig {
                        neighborhood: neighborhood.clone(),
                        move_order: order,
                        selection,
                        max_evaluations: cap,
                        max_steps: 3,
                        assert_scores: true,
                        ..Default::default()
                    };
                    let scoped = local_search(&p, &n, c.clone()).unwrap();
                    let (g, s) = graph();
                    let stream =
                        stream_local_search_from(&p, g, s, p.initial_assignment(), c).unwrap();
                    let expected = if cap > 1 && matches!(neighborhood, Neighborhood::SubPillar(_))
                    {
                        10
                    } else {
                        0
                    };
                    assert_eq!(
                        scoped.best.as_ref().unwrap().score,
                        SimpleScore::new(expected)
                    );
                    assert_eq!(
                        stream.best.as_ref().unwrap().score,
                        SimpleScore::new(expected)
                    );
                    assert_eq!(
                        scoped.best.unwrap().assignment,
                        stream.best.unwrap().assignment
                    );
                    assert!(scoped.evaluations <= cap && stream.evaluations <= cap);
                }
            }
        }
    }
}

#[test]
fn consecutive_provider_has_documented_member_order_and_subset_local_domains() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 9]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let c = SubPillarConfig {
        selection: SubPillarSelection::consecutive(2, 2).unwrap(),
        changes: PillarChangeMode::AssignedOnly,
        swap_selection: None,
    };
    let moves: Vec<_> = subpillar_moves(&p, &a, c).unwrap().collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(moves[0].touched_variables(), vec![1, 2]);
    let mut state = a.clone();
    p.apply(&mut state, &moves[0]).unwrap().commit();
    assert_eq!(state.values(), &[Some(0), Some(1), Some(1)]);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap();
    let a = p.initial_assignment();
    let sets: Vec<_> = subpillar_moves(&p, &a, c)
        .unwrap()
        .map(|m| m.touched_variables())
        .collect();
    assert_eq!(sets, vec![vec![0, 1], vec![1, 2], vec![2, 3]]);
}

#[test]
fn large_combination_space_clones_only_selected_candidate_values() {
    struct V(usize, Rc<Cell<usize>>);
    impl Clone for V {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for V {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for V {}
    let count = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..128)
            .map(|_| {
                Variable::new(vec![V(0, count.clone()), V(1, count.clone())])
                    .with_initial(V(0, count.clone()))
            })
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let c = SubPillarConfig::new(SubPillarSelection::combinations(64, 64).unwrap());
    let mut moves = subpillar_moves(&p, &a, c).unwrap();
    assert_eq!(count.get(), 0);
    let first = moves.next().unwrap();
    assert_eq!(count.get(), 64);
    assert_eq!(first.touched_variables(), (0..64).collect::<Vec<_>>());
}

#[test]
fn undersized_swap_partners_are_skipped_before_expanding_large_left_subsets() {
    let mut variables = vec![Variable::new(vec![0, 1]).with_initial(0); 128];
    variables.push(Variable::new(vec![0, 1]).with_initial(1));
    let p = FiniteProblem::new(variables).unwrap();
    let a = p.initial_assignment();
    let config = SubPillarConfig {
        changes: PillarChangeMode::Disabled,
        ..SubPillarConfig::new(SubPillarSelection::combinations(64, 64).unwrap())
    };
    assert_eq!(subpillar_moves(&p, &a, config).unwrap().count(), 0);
}
