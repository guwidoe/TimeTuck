//! Original model, transaction, and exhaustive-search regressions.
use std::{
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::{
    HardSoftScore, ListAssignment, ListEntity, ListError, ListMove, ListPosition, ListProblem,
    SimpleScore, Termination, exhaustive_list_search,
};

fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn empty_problem(n: i32, m: usize, optional: bool) -> ListProblem<i32> {
    let p = ListProblem::new(
        (0..n).collect(),
        (0..m).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap();
    if optional { p.allowing_unassigned() } else { p }
}

// Independent reference: choose an owner (or unassigned) for each value, then
// enumerate each owner's permutations. No insertion-tree algorithm is used.
fn permutations(values: &[i32]) -> Vec<Vec<i32>> {
    if values.is_empty() {
        return vec![vec![]];
    }
    let mut out = vec![];
    for (i, v) in values.iter().enumerate() {
        let rest: Vec<_> = values
            .iter()
            .enumerate()
            .filter_map(|(j, v)| (j != i).then_some(*v))
            .collect();
        for mut p in permutations(&rest) {
            p.insert(0, *v);
            out.push(p);
        }
    }
    out
}
fn reference(n: i32, m: usize, optional: bool) -> BTreeSet<Vec<Vec<i32>>> {
    fn owners(
        value: i32,
        n: i32,
        m: usize,
        optional: bool,
        lists: &mut Vec<Vec<i32>>,
        out: &mut BTreeSet<Vec<Vec<i32>>>,
    ) {
        if value == n {
            fn combine(
                entity: usize,
                lists: &[Vec<i32>],
                state: &mut Vec<Vec<i32>>,
                out: &mut BTreeSet<Vec<Vec<i32>>>,
            ) {
                if entity == lists.len() {
                    out.insert(state.clone());
                    return;
                }
                for p in permutations(&lists[entity]) {
                    state.push(p);
                    combine(entity + 1, lists, state, out);
                    state.pop();
                }
            }
            combine(0, lists, &mut vec![], out);
            return;
        }
        for entity in 0..m {
            lists[entity].push(value);
            owners(value + 1, n, m, optional, lists, out);
            lists[entity].pop();
        }
        if optional {
            owners(value + 1, n, m, optional, lists, out);
        }
    }
    let mut out = BTreeSet::new();
    owners(0, n, m, optional, &mut vec![vec![]; m], &mut out);
    out
}
fn quality(a: &ListAssignment<i32>) -> HardSoftScore {
    let hard = -(a.lists().iter().filter(|l| l.len() > 2).count() as i64);
    let soft = a
        .lists()
        .iter()
        .enumerate()
        .map(|(e, l)| {
            l.iter()
                .enumerate()
                .map(|(i, v)| -((*v as i64 + 1) * (i as i64 + 1) * (e as i64 + 1)))
                .sum::<i64>()
        })
        .sum();
    HardSoftScore::new(hard, soft)
}
fn assert_shadows(a: &ListAssignment<i32>, n: i32) {
    for v in 0..n {
        let expected = a
            .lists()
            .iter()
            .enumerate()
            .flat_map(|(e, l)| l.iter().enumerate().map(move |(i, x)| (e, i, x)))
            .find(|(_, _, x)| **x == v);
        match expected {
            None => {
                assert_eq!(a.location(&v), None);
                assert!(a.element(&v).is_none());
            }
            Some((e, i, _)) => {
                let shadow = a.element(&v).unwrap();
                assert_eq!(shadow.position, pos(e, i));
                assert_eq!(
                    shadow.previous,
                    if i == 0 {
                        None
                    } else {
                        Some(&a.lists()[e][i - 1])
                    }
                );
                assert_eq!(shadow.next, a.lists()[e].get(i + 1));
            }
        }
    }
}
#[test]
fn exact_enumeration_matches_independent_owner_permutations() {
    for n in 0..=4 {
        for m in 0..=3 {
            for optional in [false, true] {
                let p = empty_problem(n, m, optional);
                let expected = reference(n, m, optional);
                let mut actual = BTreeSet::new();
                let mut optimum = None;
                let result = exhaustive_list_search(
                    &p,
                    |a| {
                        assert!(p.is_complete(a));
                        assert_shadows(a, n);
                        assert!(actual.insert(a.lists().to_vec()), "duplicate assignment");
                        quality(a)
                    },
                    u64::MAX,
                );
                for lists in &expected {
                    let score = quality(&ListAssignment::new(lists.clone()));
                    optimum = Some(optimum.map_or(score, |best: HardSoftScore| best.max(score)));
                }
                assert_eq!(actual, expected, "n={n},m={m},optional={optional}");
                assert_eq!(result.evaluations, expected.len() as u64);
                assert_eq!(result.termination, Termination::Exhausted);
                assert_eq!(result.best.map(|b| b.score), optimum);
            }
        }
    }
}
#[test]
fn exact_budgets_ties_and_structural_rejections() {
    let p = empty_problem(3, 2, false);
    let mut first = None;
    let full = exhaustive_list_search(
        &p,
        |a| {
            first.get_or_insert_with(|| a.clone());
            SimpleScore::new(1)
        },
        100,
    );
    assert_eq!(full.evaluations, 24);
    assert_eq!(full.best.as_ref().unwrap().assignment, first.unwrap());
    for budget in [0, 1, 23, 24, 25] {
        let mut calls = 0;
        let r = exhaustive_list_search(
            &p,
            |_| {
                calls += 1;
                SimpleScore::new(0)
            },
            budget,
        );
        assert_eq!(calls, budget.min(24));
        assert_eq!(r.evaluations, calls);
        assert_eq!(
            r.termination,
            if budget < 24 {
                Termination::EvaluationLimit
            } else {
                Termination::Exhausted
            }
        );
        assert_eq!(r.is_optimal(), budget >= 24);
    }
    let rejected = exhaustive_list_search(&p, |_| SimpleScore::new(100).with_structural(-1), 24);
    assert!(rejected.best.is_none());
    assert!(!rejected.is_optimal());
    assert_eq!(rejected.termination, Termination::Exhausted);
    let infeasible = exhaustive_list_search(&p, |_| HardSoftScore::new(-1, 0), 24);
    assert!(infeasible.is_optimal());
    assert!(!infeasible.best.unwrap().score.is_feasible());
}
#[test]
fn zero_entities_and_empty_domains_have_truthful_exhaustion() {
    let empty = empty_problem(0, 0, false);
    let r = exhaustive_list_search(
        &empty,
        |a| {
            assert!(a.lists().is_empty());
            SimpleScore::new(0)
        },
        1,
    );
    assert_eq!(r.evaluations, 1);
    assert!(r.is_optimal());
    let impossible = empty_problem(2, 0, false);
    let r = exhaustive_list_search(&impossible, |_| panic!("empty space scored"), 0);
    let _: timetuck::ListSearchResult<i32, SimpleScore> = r.clone();
    assert_eq!(r.evaluations, 0);
    assert_eq!(r.termination, Termination::Exhausted);
    let optional = empty_problem(2, 0, true);
    let r = exhaustive_list_search(&optional, |_| SimpleScore::new(0), 1);
    assert_eq!(r.evaluations, 1);
    assert!(r.is_optimal());
}
#[test]
fn pins_and_initial_values_match_filtered_independent_space() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![1]).with_pinned(true),
            ListEntity::new(vec![3]),
        ],
    )
    .unwrap();
    let expected: BTreeSet<_> = reference(4, 3, false)
        .into_iter()
        .filter(|a| a[0].first() == Some(&0) && a[1] == [1])
        .collect();
    let mut actual = BTreeSet::new();
    let r = exhaustive_list_search(
        &p,
        |a| {
            assert!(p.is_complete(a));
            actual.insert(a.lists().to_vec());
            quality(a)
        },
        1000,
    );
    assert_eq!(actual, expected);
    assert_eq!(r.evaluations, 6);
    assert!(r.is_optimal());
    assert_eq!(
        p.initial_assignment().lists(),
        &[vec![0, 2], vec![1], vec![3]]
    );
    let full_prefix = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]).with_pinned_prefix(1)],
    )
    .unwrap();
    let r = exhaustive_list_search(&full_prefix, quality, 10);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.best.unwrap().assignment.lists(), &[vec![0, 1]]);
}
#[test]
fn model_rejects_duplicate_unknown_and_invalid_pin_states() {
    assert_eq!(
        ListProblem::new(vec![0, 0], vec![]).unwrap_err(),
        ListError::DuplicateDomainValue
    );
    assert_eq!(
        ListProblem::new(vec![0], vec![ListEntity::new(vec![1])]).unwrap_err(),
        ListError::ValueOutsideDomain
    );
    assert_eq!(
        ListProblem::new(
            vec![0],
            vec![ListEntity::new(vec![0]), ListEntity::new(vec![0])]
        )
        .unwrap_err(),
        ListError::DuplicateAssignedValue
    );
    for e in [
        ListEntity::new(vec![0]).with_pinned_prefix(2),
        ListEntity::new(vec![0])
            .with_pinned_prefix(1)
            .with_pinned(true),
    ] {
        assert_eq!(
            ListProblem::new(vec![0], vec![e]).unwrap_err(),
            ListError::InvalidPin { entity: 0 }
        );
    }
    let p = empty_problem(2, 2, false);
    assert_eq!(
        p.validate(&ListAssignment::new(vec![vec![]])),
        Err(ListError::ListCount {
            expected: 2,
            actual: 1
        })
    );
    assert_eq!(
        p.validate(&ListAssignment::new(vec![vec![0, 0], vec![]])),
        Err(ListError::DuplicateAssignedValue)
    );
    assert!(!p.is_complete(&p.initial_assignment()));
    assert!(
        p.clone()
            .allowing_unassigned()
            .is_complete(&p.initial_assignment())
    );
}
#[test]
fn every_small_change_swap_and_unassign_preserves_invariants_and_undo() {
    let p = empty_problem(3, 2, true);
    for lists in reference(3, 2, true) {
        let original = ListAssignment::new(lists);
        let mut moves = vec![];
        for e in 0..2 {
            for i in 0..original.lists()[e].len() {
                for d in 0..2 {
                    let max = original.lists()[d].len() - usize::from(e == d);
                    for j in 0..=max {
                        moves.push(ListMove::Change {
                            source: pos(e, i),
                            destination: pos(d, j),
                        });
                    }
                    for j in 0..original.lists()[d].len() {
                        moves.push(ListMove::Swap {
                            left: pos(e, i),
                            right: pos(d, j),
                        });
                    }
                }
            }
            for start in 0..=original.lists()[e].len() {
                for end in start..=original.lists()[e].len() {
                    moves.push(ListMove::Unassign {
                        entity: e,
                        start,
                        end,
                    });
                }
            }
            for v in 0..3 {
                if original.location(&v).is_none() {
                    for i in 0..=original.lists()[e].len() {
                        moves.push(ListMove::Assign {
                            value: v,
                            destination: pos(e, i),
                        });
                    }
                }
            }
        }
        for movement in moves {
            let mut state = original.clone();
            let candidate;
            {
                let applied = p.apply(&mut state, &movement).unwrap();
                assert_eq!(applied.original(), &original);
                assert!(p.is_complete(&applied));
                assert_shadows(&applied, 3);
                candidate = (*applied).clone();
                match movement {
                    ListMove::Change {
                        source,
                        destination,
                    } => {
                        let moved = original.lists()[source.entity][source.index];
                        assert_eq!(applied.location(&moved), Some(destination));
                        let mut before: Vec<_> =
                            original.lists().iter().flatten().copied().collect();
                        before.sort();
                        let mut after: Vec<_> = applied.lists().iter().flatten().copied().collect();
                        after.sort();
                        assert_eq!(before, after);
                    }
                    ListMove::Swap { left, right } => {
                        assert_eq!(
                            applied.lists()[left.entity][left.index],
                            original.lists()[right.entity][right.index]
                        );
                        assert_eq!(
                            applied.lists()[right.entity][right.index],
                            original.lists()[left.entity][left.index]
                        );
                    }
                    _ => {}
                }
            }
            assert_eq!(state, original);
            p.apply(&mut state, &movement).unwrap().commit();
            assert_eq!(state, candidate);
            assert_shadows(&state, 3);
        }
    }
}
#[test]
fn invalid_moves_and_composites_roll_back_atomically() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![])],
    )
    .unwrap();
    let original = p.initial_assignment();
    let invalid = [
        ListMove::Assign {
            value: 0,
            destination: pos(1, 0),
        },
        ListMove::Assign {
            value: 9,
            destination: pos(1, 0),
        },
        ListMove::Assign {
            value: 2,
            destination: pos(2, 0),
        },
        ListMove::Assign {
            value: 2,
            destination: pos(1, 1),
        },
        ListMove::Change {
            source: pos(0, 2),
            destination: pos(1, 0),
        },
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(0, 2),
        },
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
        ListMove::Unassign {
            entity: 0,
            start: 2,
            end: 1,
        },
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: usize::MAX,
        },
    ];
    for movement in invalid {
        let mut a = original.clone();
        assert!(p.apply(&mut a, &movement).is_err());
        assert_eq!(a, original);
        let composite = ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(0, 1),
            },
            ListMove::Composite(vec![movement]),
        ]);
        assert!(p.apply(&mut a, &composite).is_err());
        assert_eq!(a, original);
    }
}
#[test]
fn pin_checks_distinguish_whole_list_and_prefix() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0]).with_pinned_prefix(1),
            ListEntity::new(vec![1]).with_pinned(true),
        ],
    )
    .unwrap();
    let original = p.initial_assignment();
    for movement in [
        ListMove::Assign {
            value: 2,
            destination: pos(0, 0),
        },
        ListMove::Assign {
            value: 2,
            destination: pos(1, 1),
        },
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
    ] {
        let mut a = original.clone();
        assert!(matches!(
            p.apply(&mut a, &movement),
            Err(ListError::PinChanged { .. })
        ));
        assert_eq!(a, original);
    }
    let mut a = original.clone();
    p.apply(
        &mut a,
        &ListMove::Assign {
            value: 2,
            destination: pos(0, 1),
        },
    )
    .unwrap()
    .commit();
    assert!(p.is_complete(&a));
    assert_eq!(a.lists(), &[vec![0, 2], vec![1]]);
    let atomic = ListMove::Composite(vec![
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: 0,
            destination: pos(0, 0),
        },
    ]);
    p.apply(&mut a, &atomic).unwrap().commit();
    assert_eq!(a.lists(), &[vec![0, 2], vec![1]]);
}
#[test]
fn panic_unwinding_restores_lists_and_derived_shadows() {
    let p = empty_problem(3, 2, true);
    let mut a = ListAssignment::new(vec![vec![0, 1], vec![2]]);
    let original = a.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let candidate = p
                .apply(
                    &mut a,
                    &ListMove::Change {
                        source: pos(0, 1),
                        destination: pos(1, 0),
                    },
                )
                .unwrap();
            assert_shadows(&candidate, 3);
            panic!("scorer panic");
        }))
        .is_err()
    );
    assert_eq!(a, original);
    assert_shadows(&a, 3);
}
#[test]
fn composite_metadata_and_deep_moves_are_iterative() {
    let p = empty_problem(1, 2, true);
    let mut a = p.initial_assignment();
    let mut movement = ListMove::Assign {
        value: 0,
        destination: pos(1, 0),
    };
    for _ in 0..3000 {
        movement = ListMove::Composite(vec![movement]);
    }
    assert_eq!(movement.touched_entities(), vec![1]);
    {
        let candidate = p.apply(&mut a, &movement).unwrap();
        assert_eq!(candidate.list(1), Some([0].as_slice()));
    }
    assert_eq!(a, p.initial_assignment());
    // Disassemble iteratively too: ordinary enum destruction can recurse.
    let mut pending = vec![movement];
    while let Some(m) = pending.pop() {
        if let ListMove::Composite(steps) = m {
            pending.extend(steps);
        }
    }
    let both: ListMove<i32> = ListMove::Composite(vec![
        ListMove::Swap {
            left: pos(1, 0),
            right: pos(0, 0),
        },
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 0,
        },
    ]);
    assert_eq!(both.touched_entities(), vec![0, 1]);
}
#[test]
fn long_optional_empty_lists_enumerate_without_recursive_stack() {
    let p = empty_problem(2000, 0, true);
    let r = exhaustive_list_search(&p, |_| SimpleScore::new(0), 1);
    assert_eq!(r.evaluations, 1);
    assert!(r.is_optimal());
}

#[test]
fn clone_panics_after_earlier_composite_edits_restore_original() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[derive(Debug)]
    struct Value {
        id: i32,
        armed: Arc<AtomicBool>,
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Value {}
    impl Clone for Value {
        fn clone(&self) -> Self {
            assert!(
                !(self.id == 2 && self.armed.load(Ordering::SeqCst)),
                "injected clone panic"
            );
            Self {
                id: self.id,
                armed: self.armed.clone(),
            }
        }
    }
    let armed = Arc::new(AtomicBool::new(false));
    let make = |id| Value {
        id,
        armed: armed.clone(),
    };
    let p = ListProblem::new(
        vec![make(0), make(1), make(2)],
        vec![ListEntity::new(vec![make(0), make(1)])],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let original = a.clone();
    let movement = ListMove::Composite(vec![
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 1),
        },
        ListMove::Assign {
            value: make(2),
            destination: pos(0, 2),
        },
    ]);
    armed.store(true, Ordering::SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = p.apply(&mut a, &movement);
        }))
        .is_err()
    );
    assert_eq!(a, original);
    assert_eq!(a.location(&make(0)), Some(pos(0, 0)));
    // Snapshot cloning can also fail, before any edit is attempted.
    armed.store(false, Ordering::SeqCst);
    p.apply(
        &mut a,
        &ListMove::Assign {
            value: make(2),
            destination: pos(0, 2),
        },
    )
    .unwrap()
    .commit();
    armed.store(true, Ordering::SeqCst);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = p.apply(
                &mut a,
                &ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 1,
                },
            );
        }))
        .is_err()
    );
    assert_eq!(
        a.lists()[0].iter().map(|v| v.id).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}
