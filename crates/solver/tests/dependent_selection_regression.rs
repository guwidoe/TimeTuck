use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
fn length(seed: usize, depth: usize, prefix: &[Option<usize>]) -> usize {
    (seed + depth + prefix.iter().flatten().sum::<usize>()) % 3
}
fn reference(
    levels: usize,
    ignore: bool,
    seed: usize,
    prefix: &mut Vec<Option<usize>>,
    out: &mut Vec<Vec<Option<usize>>>,
) {
    if prefix.len() == levels {
        if prefix.iter().any(Option::is_some) {
            out.push(prefix.clone());
        }
        return;
    }
    let n = length(seed, prefix.len(), prefix);
    if n == 0 && ignore {
        prefix.push(None);
        reference(levels, ignore, seed, prefix, out);
        prefix.pop();
    }
    for value in 0..n {
        prefix.push(Some(value));
        reference(levels, ignore, seed, prefix, out);
        prefix.pop();
    }
}
#[test]
fn dependent_trees_match_recursive_reference_and_empty_branches_backtrack() {
    for levels in 0..=5 {
        for seed in 0..10 {
            for ignore in [false, true] {
                let mut expected = vec![];
                reference(levels, ignore, seed, &mut vec![], &mut expected);
                let mut iter = DependentSelectionIter::new(
                    levels,
                    ignore,
                    10000,
                    |depth, prefix: &[Option<Arc<usize>>]| {
                        let prefix = prefix
                            .iter()
                            .map(|v| v.as_ref().map(|v| **v))
                            .collect::<Vec<_>>();
                        0..length(seed, depth, &prefix)
                    },
                );
                let actual = iter
                    .by_ref()
                    .map(|row| row.into_iter().map(|v| v.map(|v| *v)).collect::<Vec<_>>())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected);
                assert_eq!(iter.stats().stop, Some(DependentStop::Exhausted));
                assert_eq!(iter.stats().yielded, expected.len());
                assert_eq!(iter.next(), None);
            }
        }
    }
    let mut iter =
        DependentSelectionIter::new(2, false, 100, |depth, prefix: &[Option<Arc<usize>>]| {
            if depth == 0 {
                0..2
            } else if **prefix[0].as_ref().unwrap() == 0 {
                0..0
            } else {
                2..4
            }
        });
    assert_eq!(
        iter.by_ref()
            .map(|r| r.into_iter().map(|v| *v.unwrap()).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec![1, 2], vec![1, 3]]
    );
    assert_eq!(iter.stats().stop, Some(DependentStop::Exhausted));
}
#[test]
fn rejected_infinite_branches_and_zero_budgets_terminate_without_extra_opens() {
    let calls = Rc::new(Cell::new(0));
    let opened = calls.clone();
    let mut iter =
        DependentSelectionIter::new(2, false, 17, move |depth, _: &[Option<Arc<usize>>]| {
            opened.set(opened.get() + 1);
            if depth == 0 { 0..usize::MAX } else { 0..0 }
        });
    assert_eq!(calls.get(), 0);
    assert_eq!(iter.next(), None);
    assert_eq!(
        iter.stats(),
        DependentSelectionStats {
            polls: 17,
            opened: 9,
            yielded: 0,
            stop: Some(DependentStop::PollLimit)
        }
    );
    assert_eq!(iter.next(), None);
    assert_eq!(calls.get(), 9);
    let mut zero = DependentSelectionIter::new(
        3,
        true,
        0,
        |_, _: &[Option<Arc<usize>>]| -> std::ops::Range<usize> { panic!("zero budget factory") },
    );
    assert_eq!(zero.next(), None);
    assert_eq!(zero.stats().stop, Some(DependentStop::PollLimit));
    let mut none = DependentSelectionIter::new(
        0,
        false,
        0,
        |_, _: &[Option<Arc<usize>>]| -> std::ops::Range<usize> { panic!("zero levels factory") },
    );
    assert_eq!(none.next(), None);
    assert_eq!(none.stats().stop, Some(DependentStop::Exhausted));
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
#[test]
fn nonclone_prefixes_can_be_captured_by_suffix_cursors_and_rows_survive_drop() {
    let drops = Rc::new(Cell::new(0));
    let owned = drops.clone();
    let mut iter = DependentSelectionIter::new(
        2,
        false,
        10,
        move |depth, prefix: &[Option<Arc<Item>>]| -> Box<dyn Iterator<Item = Item>> {
            let drops = owned.clone();
            if depth == 0 {
                Box::new(std::iter::once(Item { value: 4, drops }))
            } else {
                let earlier = prefix[0].as_ref().unwrap().clone();
                Box::new(std::iter::once_with(move || Item {
                    value: earlier.value + 1,
                    drops,
                }))
            }
        },
    );
    let held = iter.next().unwrap();
    assert_eq!(held[0].as_ref().unwrap().value, 4);
    assert_eq!(held[1].as_ref().unwrap().value, 5);
    drop(iter);
    assert_eq!(drops.get(), 0);
    drop(held);
    assert_eq!(drops.get(), 2);
}
#[test]
fn sampling_uses_current_context_and_prefix_and_dead_ends_are_per_attempt() {
    let mut selection = DependentSelection::new(
        2,
        false,
        |start: &usize, depth, prefix: &[Option<Arc<usize>>]| {
            if depth == 0 {
                *start..*start + 2
            } else {
                0..**prefix[0].as_ref().unwrap()
            }
        },
        |_, _: &usize| {},
    );
    let row = selection
        .sample_with(&2, 20, |_, bound| bound - 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        row.into_iter().map(|v| *v.unwrap()).collect::<Vec<_>>(),
        [3, 2]
    );
    assert_eq!(selection.sample_with(&0, 20, |_, _| 0).unwrap(), None);
    let row = selection
        .sample_with(&0, 20, |_, bound| bound - 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        row.into_iter().map(|v| *v.unwrap()).collect::<Vec<_>>(),
        [1, 0]
    );
    assert!(matches!(
        selection.sample_with(&2, 20, |_, bound| bound),
        Err(DependentSampleError::InvalidDraw {
            depth: 0,
            index: 2,
            bound: 2
        })
    ));
    let mut skipped = DependentSelection::new(
        3,
        true,
        |_: &(), depth, _: &[Option<Arc<usize>>]| if depth == 1 { 0..0 } else { depth..depth + 1 },
        |_, _: &()| {},
    );
    let row = skipped.sample_with(&(), 10, |_, _| 0).unwrap().unwrap();
    assert_eq!(
        row.into_iter().map(|v| v.map(|v| *v)).collect::<Vec<_>>(),
        [Some(0), None, Some(2)]
    );
}
#[test]
fn random_path_collection_honors_total_poll_budget_including_exhaustion() {
    let opened = Rc::new(Cell::new(0));
    let calls = opened.clone();
    let mut selection = DependentSelection::new(
        2,
        false,
        move |_: &(), _, _: &[Option<Arc<usize>>]| {
            calls.set(calls.get() + 1);
            0..usize::MAX
        },
        |_, _: &()| {},
    );
    assert_eq!(
        selection.sample_with(&(), 0, |_, _| 0),
        Err(DependentSampleError::PollLimit { depth: 0 })
    );
    assert_eq!(opened.get(), 0);
    assert_eq!(
        selection.sample_with(&(), 5, |_, _| panic!("uncollected draw")),
        Err(DependentSampleError::PollLimit { depth: 0 })
    );
    assert_eq!(opened.get(), 1);
    let mut exact = DependentSelection::new(
        2,
        false,
        |_: &(), _, _: &[Option<Arc<usize>>]| 0..1,
        |_, _: &()| {},
    );
    assert_eq!(
        exact.sample_with(&(), 3, |_, _| 0),
        Err(DependentSampleError::PollLimit { depth: 1 })
    );
    assert!(exact.sample_with(&(), 4, |_, _| 0).unwrap().is_some());
}
#[test]
fn dependent_choices_solve_correlated_composite_moves_in_both_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2, 3]).with_initial(0),
        Variable::new(vec![0, 1, 2, 3]).with_initial(0),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap() + *v[1].unwrap()))
    })
    .unwrap();
    for sample in [false, true] {
        for stream in [false, true] {
            let mut selection = DependentSelection::new(
                2,
                false,
                |_: &Assignment<i32>, depth, prefix: &[Option<Arc<Move<i32>>>]| {
                    let values = if depth == 0 {
                        vec![1, 2, 3]
                    } else {
                        match prefix[0].as_ref().unwrap().as_ref() {
                            Move::Change { value: Some(v), .. } => vec![*v],
                            _ => unreachable!(),
                        }
                    };
                    values.into_iter().map(move |v| Move::Change {
                        variable: depth,
                        value: Some(v),
                    })
                },
                |_, _: &Assignment<i32>| {},
            );
            let mut provider = move |_: &FiniteProblem<i32>,
                                     assignment: &Assignment<i32>,
                                     _: u64| {
                let rows = if sample {
                    selection
                        .sample_with(assignment, 20, |_, n| n - 1)
                        .unwrap()
                        .into_iter()
                        .collect::<Vec<_>>()
                } else {
                    selection.iter(assignment, 100).collect()
                };
                rows.into_iter()
                    .map(|row| {
                        Move::Composite(row.into_iter().flatten().map(|m| (*m).clone()).collect())
                    })
                    .collect::<Vec<_>>()
            };
            let config = LocalSearchConfig {
                max_steps: 1,
                max_move_evaluations: Some(20),
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
            assert_eq!(result.best.unwrap().score, SimpleScore::new(6));
        }
    }
}
