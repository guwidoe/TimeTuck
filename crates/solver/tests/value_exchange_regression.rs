// Original native regressions; these grant no upstream method credit.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;

#[test]
fn exhaustive_successors_match_independent_label_substitution_and_rollback() {
    let domains = [vec![0, 1, 2], vec![0, 1], vec![1, 2]];
    let order = [2, 0, 1];
    for encoded in 0..64 {
        let original: Vec<_> = (0..3)
            .map(|i| match (encoded >> (i * 2)) & 3 {
                3 => None,
                n => Some(n),
            })
            .collect();
        if original
            .iter()
            .zip(&domains)
            .any(|(v, d)| v.is_some_and(|v| !d.contains(&v)))
        {
            continue;
        }
        for pinned in [false, true] {
            let vars = domains
                .iter()
                .enumerate()
                .map(|(i, domain)| {
                    let mut v = Variable::new(domain.clone()).with_pinned(pinned && i == 1);
                    if i != 0 {
                        v = v.allowing_unassigned();
                    }
                    if let Some(value) = original[i] {
                        v = v.with_initial(value);
                    }
                    v
                })
                .collect();
            let p = FiniteProblem::new(vars).unwrap();
            let a = p.initial_assignment();
            for consecutive in [false, true] {
                for min in 1..=3 {
                    for include_unassigned in [false, true] {
                        let config = ValueExchangeConfig {
                            selection: if consecutive {
                                SubPillarSelection::consecutive(min, 3)
                            } else {
                                SubPillarSelection::combinations(min, 3)
                            }
                            .unwrap(),
                            include_unassigned,
                        };
                        let mut expected = BTreeSet::new();
                        let mut labels = vec![Some(0), Some(1), Some(2)];
                        if include_unassigned {
                            labels.push(None);
                        }
                        // Independent bit-mask selection and direct label mapping.
                        for mask in 1u32..8 {
                            let positions: Vec<_> =
                                (0..3).filter(|i| mask & (1 << i) != 0).collect();
                            if positions.len() < min
                                || (consecutive
                                    && positions.last().unwrap() - positions[0] + 1
                                        != positions.len())
                            {
                                continue;
                            }
                            for left in 0..labels.len() {
                                for right in left + 1..labels.len() {
                                    let mut next = original.clone();
                                    for &pos in &positions {
                                        let slot = order[pos];
                                        next[slot] = if original[slot] == labels[left] {
                                            labels[right]
                                        } else if original[slot] == labels[right] {
                                            labels[left]
                                        } else {
                                            original[slot]
                                        };
                                    }
                                    if next == original
                                        || (pinned && next[1] != original[1])
                                        || (next[0].is_none() && original[0].is_some())
                                    {
                                        continue;
                                    }
                                    if next
                                        .iter()
                                        .zip(&domains)
                                        .any(|(v, d)| v.is_some_and(|v| !d.contains(&v)))
                                    {
                                        continue;
                                    }
                                    expected.insert(next);
                                }
                            }
                        }
                        let mut actual = BTreeSet::new();
                        for m in value_exchange_moves_in_order(&p, &a, order, config).unwrap() {
                            let mut working = a.clone();
                            {
                                let applied = p.apply(&mut working, &m).unwrap();
                                // Move guards dereference to the candidate assignment.
                                actual.insert(applied.values().to_vec());
                            }
                            assert_eq!(working, a);
                        }
                        assert_eq!(actual, expected, "{original:?}, pin={pinned}, {config:?}");
                    }
                }
            }
        }
    }
}

#[test]
fn pins_do_not_close_gaps_in_contiguous_order() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 9]).with_initial(0),
        Variable::new(vec![0, 1, 9])
            .with_initial(9)
            .with_pinned(true),
        Variable::new(vec![0, 1, 9]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let c = ValueExchangeConfig::new(SubPillarSelection::consecutive(2, 2).unwrap());
    let swap = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(1),
        },
        Move::Change {
            variable: 2,
            value: Some(0),
        },
    ]);
    assert!(!value_exchange_moves(&p, &a, c).unwrap().any(|m| m == swap));
    assert!(
        value_exchange_moves_in_order(&p, &a, [0, 2], c)
            .unwrap()
            .any(|m| m == swap)
    );
    let full = ValueExchangeConfig::new(SubPillarSelection::consecutive(3, 3).unwrap());
    assert_eq!(
        value_exchange_moves(&p, &a, full)
            .unwrap()
            .collect::<Vec<_>>(),
        [swap]
    );
}

#[test]
fn rejects_invalid_orders_and_assignments_before_iteration() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let a = p.initial_assignment();
    let c = ValueExchangeConfig::new(SubPillarSelection::combinations(1, usize::MAX).unwrap());
    assert_eq!(
        value_exchange_moves_in_order(&p, &a, [0, 0], c).err(),
        Some(ValueExchangeError::RepeatedVariable { variable: 0 })
    );
    assert!(matches!(
        value_exchange_moves_in_order(&p, &a, [1], c).err(),
        Some(ValueExchangeError::Model(ModelError::Move(_)))
    ));
    assert!(matches!(
        value_exchange_moves(&p, &Assignment::new(vec![]), c).err(),
        Some(ValueExchangeError::Model(
            ModelError::AssignmentLength { .. }
        ))
    ));
    assert_eq!(
        value_exchange_moves_in_order(&p, &a, [], c)
            .unwrap()
            .count(),
        0
    );
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    assert_eq!(
        value_exchange_moves(&empty, &empty.initial_assignment(), c)
            .unwrap()
            .count(),
        0
    );
}

#[derive(Debug)]
struct Counted {
    id: i32,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Counted {}

#[test]
fn opens_without_cloning_and_clones_only_yielded_writes() {
    let clones = Rc::new(Cell::new(0));
    let value = |id| Counted {
        id,
        clones: clones.clone(),
    };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![value(0), value(1), value(2)]).with_initial(value(0)),
        Variable::new(vec![value(0), value(1), value(2)]).with_initial(value(1)),
    ])
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let c = ValueExchangeConfig::new(SubPillarSelection::consecutive(2, 2).unwrap());
    let mut moves = value_exchange_moves(&p, &a, c).unwrap();
    assert_eq!(clones.get(), 0);
    assert!(matches!(moves.next(), Some(Move::Composite(_))));
    assert_eq!(clones.get(), 2);
    assert!(matches!(
        moves.next(),
        Some(Move::Change { variable: 0, .. })
    ));
    assert_eq!(clones.get(), 3);
    drop(moves);
    assert_eq!(clones.get(), 3);
}

struct ExchangeProvider(ValueExchangeConfig);
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for ExchangeProvider {
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<i32>,
        a: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(value_exchange_moves(p, a, self.0).unwrap())
    }
}

#[test]
fn simultaneous_exchange_escapes_single_change_optimum_in_both_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let score = |a: i32, b: i32| {
        SimpleScore::new(if a == b {
            -10
        } else if a == 1 {
            2
        } else {
            0
        })
    };
    let mut network = ConstraintNetwork::new(2);
    network
        .add_constraint("pair", vec![0, 1], move |v| {
            score(*v[0].unwrap(), *v[1].unwrap())
        })
        .unwrap();
    let config = LocalSearchConfig {
        max_steps: 4,
        max_move_evaluations: Some(50),
        assert_scores: true,
        ..Default::default()
    };
    // Single-variable changes cannot reach [1,0] without a worse intermediate state.
    let mut single = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
        change_moves(p, a).unwrap().collect::<Vec<_>>()
    };
    let stuck =
        local_search_from_with_provider(&p, &network, p.initial_assignment(), config, &mut single)
            .unwrap();
    assert_eq!(stuck.best.unwrap().score, SimpleScore::new(0));
    for stream in [false, true] {
        let mut provider = ExchangeProvider(ValueExchangeConfig::new(
            SubPillarSelection::consecutive(2, 2).unwrap(),
        ));
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            let left = b.filter(source.port(), |r| r.0 == 0).unwrap();
            let right = b.filter(source.port(), |r| r.0 == 1).unwrap();
            let pairs = b.join_equal(&left, &right, |_| (), |_| ()).unwrap();
            b.score("pair", &pairs, move |(l, r)| {
                score(l.1.unwrap(), r.1.unwrap())
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
                &network,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.assignment.values(), &[Some(1), Some(0)]);
        assert_eq!(best.score, SimpleScore::new(2));
        assert_eq!(result.completed_steps, 1);
    }
}
