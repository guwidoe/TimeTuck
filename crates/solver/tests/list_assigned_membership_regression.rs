//! Original differential checks for transaction-local assigned identity tracking.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn fixture(indexed: bool) -> ListProblem<usize> {
    let entities = vec![
        ListEntity::new((0..128).collect()).with_pinned_prefix(1),
        ListEntity::new((128..256).collect()),
        ListEntity::new(vec![]),
        ListEntity::new((256..512).collect()).with_pinned(true),
    ];
    if indexed {
        ListProblem::new_indexed((0..1024).rev().collect(), entities)
    } else {
        ListProblem::new((0..1024).rev().collect(), entities)
    }
    .unwrap()
    .allowing_unassigned()
}
fn sequence(p: &ListProblem<usize>) -> (ListMove<usize>, ListAssignment<usize>) {
    let mut a = p.initial_assignment();
    let mut steps = vec![];
    let mut append = |m: ListMove<usize>| {
        p.apply(&mut a, &m).unwrap().commit();
        steps.push(m);
    };
    for id in 512..896 {
        append(ListMove::Assign {
            value: id,
            destination: pos(2, id - 512),
        });
    }
    append(ListMove::Composite(vec![
        ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 0),
        },
        ListMove::Swap {
            left: pos(0, 2),
            right: pos(1, 2),
        },
        ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 4,
            destination: pos(1, 0),
            reverse: true,
        },
        ListMove::SubListSwap {
            left: ListRange {
                entity: 0,
                start: 1,
                end: 3,
            },
            right: ListRange {
                entity: 1,
                start: 0,
                end: 3,
            },
            reverse: true,
        },
        ListMove::Unassign {
            entity: 2,
            start: 0,
            end: 8,
        },
    ]));
    for id in 512..520 {
        append(ListMove::Assign {
            value: id,
            destination: pos(2, 376 + id - 512),
        });
    }
    (ListMove::Composite(steps), a)
}
#[test]
fn nested_all_leaf_types_match_direct_membership_and_retained_list_and_mixed_scoring() {
    let p = fixture(true);
    let direct = fixture(false);
    let before = p.initial_assignment();
    let (movement, expected) = sequence(&direct);
    let expected_values = direct.planning_values(&before, &movement).unwrap();
    assert_eq!(
        p.planning_values(&before, &movement).unwrap(),
        expected_values
    );
    let mut a = before.clone();
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        assert_eq!(*trial, expected);
        assert_eq!(trial.original(), &before);
    }
    assert_eq!(a, before);
    let mut net = ListConstraintNetwork::<_, SimpleScore>::new(4, 1024);
    net.add_constraint("receiver", vec![ListScope::Entity(2)], |facts| {
        let ListFact::Entity(row) = &facts[0] else {
            panic!()
        };
        SimpleScore::new(
            row.iter()
                .enumerate()
                .map(|(i, v)| ((i + 1) * (*v + 1)) as i64)
                .sum(),
        )
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &net, before.clone()).unwrap();
    let initial = d.score();
    for commit in [false, true] {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(trial.assignment(), &expected);
        assert_eq!(trial.score(), net.calculate(&p, &expected).unwrap());
        if commit {
            trial.commit();
        } else {
            drop(trial);
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), initial);
        }
    }
    d.assert_consistent().unwrap();
    let mixed = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        p,
    );
    let mixed_before = mixed.initial_assignment();
    let network = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 4, 1024);
    let mut d = MixedScoreDirector::new(&mixed, &network, mixed_before.clone()).unwrap();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: movement,
    };
    {
        let trial = d.apply(&m).unwrap();
        assert_eq!(trial.assignment().lists(), &expected);
        assert_eq!(trial.assignment().basic().values(), &[Some(1)]);
    }
    assert_eq!(d.assignment(), &mixed_before);
    d.assert_consistent().unwrap();
}
#[test]
fn duplicate_unknown_and_bad_coordinates_keep_error_precedence_and_atomic_rollback() {
    let indexed = fixture(true);
    let direct = fixture(false);
    let before = indexed.initial_assignment();
    let (prefix, _) = sequence(&direct);
    for tail in [
        ListMove::Assign {
            value: 512,
            destination: pos(2, 0),
        },
        ListMove::Assign {
            value: 9999,
            destination: pos(2, 0),
        },
        ListMove::Assign {
            value: 9999,
            destination: pos(2, 9999),
        },
        ListMove::Assign {
            value: 512,
            destination: pos(9, 0),
        },
        ListMove::Unassign {
            entity: 2,
            start: 0,
            end: 9999,
        },
        ListMove::Assign {
            value: 1000,
            destination: pos(3, 0),
        },
    ] {
        let m = ListMove::Composite(vec![prefix.clone(), tail]);
        let mut a = before.clone();
        let mut b = before.clone();
        let error = direct.apply(&mut b, &m).err().unwrap();
        assert_eq!(indexed.apply(&mut a, &m).err(), Some(error));
        assert_eq!(a, before);
        assert_eq!(b, before);
    }
}

use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
#[derive(Debug)]
struct Value {
    id: usize,
    payload: usize,
    hash_bomb: Rc<Cell<bool>>,
    clone_bomb: Rc<Cell<bool>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert!(!(self.clone_bomb.get() && self.id == 519), "clone failure");
        Self {
            id: self.id,
            payload: self.payload,
            hash_bomb: self.hash_bomb.clone(),
            clone_bomb: self.clone_bomb.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        assert!(!(self.hash_bomb.get() && self.id == 200), "hash failure");
        0usize.hash(h);
    }
}
#[test]
fn collision_identity_payloads_and_panics_during_index_build_or_assign_restore_exact_state() {
    let hash_bomb = Rc::new(Cell::new(false));
    let clone_bomb = Rc::new(Cell::new(false));
    let value = |id, payload| Value {
        id,
        payload,
        hash_bomb: hash_bomb.clone(),
        clone_bomb: clone_bomb.clone(),
    };
    let p = ListProblem::new_indexed(
        (0..1024).map(|id| value(id, 0)).collect(),
        vec![
            ListEntity::new((0..512).map(|id| value(id, id + 1000)).collect()),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let before = p.initial_assignment();
    let net = ListConstraintNetwork::<_, SimpleScore>::new(2, 1024);
    let mut d = ListScoreDirector::new(&p, &net, before.clone()).unwrap();
    let movement = ListMove::Composite(
        (512..896)
            .map(|id| ListMove::Assign {
                value: value(id, id + 2000),
                destination: pos(1, id - 512),
            })
            .collect(),
    );
    for bomb in [&hash_bomb, &clone_bomb] {
        bomb.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = d.apply(&movement);
            }))
            .is_err()
        );
        bomb.set(false);
        assert_eq!(d.assignment(), &before);
        assert!(
            d.assignment().lists()[0]
                .iter()
                .all(|v| v.payload == v.id + 1000)
        );
        d.assert_consistent().unwrap();
    }
    {
        let trial = d.apply(&movement).unwrap();
        assert!(
            trial.assignment().lists()[1]
                .iter()
                .all(|v| v.payload == v.id + 2000)
        );
    }
    assert_eq!(d.assignment(), &before);
}

#[test]
fn activated_membership_still_rejects_final_global_dependency_cycles() {
    let (prefix, expected) = sequence(&fixture(false));
    // Registry order is reversed: missing identity896 must precede identity0.
    let mut dependencies = vec![vec![]; 1024];
    dependencies[1023] = vec![127];
    let direct = fixture(false)
        .with_dependencies(dependencies.clone())
        .unwrap();
    let indexed = fixture(true).with_dependencies(dependencies).unwrap();
    let movement = ListMove::Composite(vec![
        prefix,
        ListMove::Assign {
            value: 896,
            destination: pos(0, expected.lists()[0].len()),
        },
    ]);
    let before = indexed.initial_assignment();
    let mut a = before.clone();
    let mut b = before.clone();
    let error = direct.apply(&mut b, &movement).err().unwrap();
    assert!(matches!(error, ListError::DependencyCycle { .. }));
    assert_eq!(indexed.apply(&mut a, &movement).err(), Some(error));
    assert_eq!(a, before);
    assert_eq!(b, before);
}
