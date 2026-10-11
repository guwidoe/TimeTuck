//! Original transaction isolation and allocation-work regressions; no upstream credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Debug)]
struct Tracking {
    clones: Vec<Cell<usize>>,
    panic_id: Cell<Option<usize>>,
}
#[derive(Debug)]
struct Value {
    id: usize,
    tracking: Rc<Tracking>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Clone for Value {
    fn clone(&self) -> Self {
        assert_ne!(
            self.tracking.panic_id.get(),
            Some(self.id),
            "injected clone panic"
        );
        let counter = &self.tracking.clones[self.id];
        counter.set(counter.get() + 1);
        Self {
            id: self.id,
            tracking: self.tracking.clone(),
        }
    }
}
fn fixture() -> (ListProblem<Value>, Rc<Tracking>) {
    let t = Rc::new(Tracking {
        clones: (0..13).map(|_| Cell::new(0)).collect(),
        panic_id: Cell::new(None),
    });
    let make = |id| Value {
        id,
        tracking: t.clone(),
    };
    let p = ListProblem::new(
        (0..13).map(make).collect(),
        (0..3)
            .map(|e| ListEntity::new((e * 4..e * 4 + 4).map(make).collect()))
            .collect(),
    )
    .unwrap()
    .allowing_unassigned();
    (p, t)
}
fn reset(t: &Tracking) {
    for c in &t.clones {
        c.set(0);
    }
}
fn counts(t: &Tracking) -> Vec<usize> {
    t.clones.iter().map(Cell::get).collect()
}
fn ids(a: &ListAssignment<Value>) -> Vec<Vec<usize>> {
    a.lists()
        .iter()
        .map(|v| v.iter().map(|v| v.id).collect())
        .collect()
}
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn swap() -> ListMove<Value> {
    ListMove::Swap {
        left: pos(0, 0),
        right: pos(1, 3),
    }
}
#[test]
fn every_move_kind_restores_or_commits_without_cloning_untouched_lists() {
    let (p, t) = fixture();
    let initial = vec![vec![0, 1, 2, 3], vec![4, 5, 6, 7], vec![8, 9, 10, 11]];
    let make = |id| Value {
        id,
        tracking: t.clone(),
    };
    let cases = vec![
        (
            ListMove::Assign {
                value: make(12),
                destination: pos(0, 2),
            },
            vec![vec![0, 1, 12, 2, 3], initial[1].clone(), initial[2].clone()],
            5,
        ),
        (
            ListMove::Change {
                source: pos(0, 1),
                destination: pos(1, 2),
            },
            vec![vec![0, 2, 3], vec![4, 5, 1, 6, 7], initial[2].clone()],
            8,
        ),
        (
            ListMove::Change {
                source: pos(0, 3),
                destination: pos(0, 0),
            },
            vec![vec![3, 0, 1, 2], initial[1].clone(), initial[2].clone()],
            4,
        ),
        (
            ListMove::SubListChange {
                entity: 0,
                start: 1,
                end: 3,
                destination: pos(1, 1),
                reverse: true,
            },
            vec![vec![0, 3], vec![4, 2, 1, 5, 6, 7], initial[2].clone()],
            8,
        ),
        (
            ListMove::SubListSwap {
                left: range(0, 1, 3),
                right: range(1, 0, 1),
                reverse: true,
            },
            vec![vec![0, 4, 3], vec![2, 1, 5, 6, 7], initial[2].clone()],
            8, // Undo snapshots only; span exchange moves the owned values.
        ),
        (
            ListMove::SubListSwap {
                left: range(0, 0, 1),
                right: range(0, 2, 4),
                reverse: true,
            },
            vec![vec![3, 2, 1, 0], initial[1].clone(), initial[2].clone()],
            4, // Same-row rotation needs only the original row snapshot.
        ),
        (
            swap(),
            vec![vec![7, 1, 2, 3], vec![4, 5, 6, 0], initial[2].clone()],
            8,
        ),
        (
            ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 3),
            },
            vec![vec![3, 1, 2, 0], initial[1].clone(), initial[2].clone()],
            4,
        ),
        (
            ListMove::Unassign {
                entity: 0,
                start: 1,
                end: 3,
            },
            vec![vec![0, 3], initial[1].clone(), initial[2].clone()],
            4,
        ),
        (
            ListMove::Composite(vec![
                swap(),
                ListMove::Composite(vec![
                    ListMove::Change {
                        source: pos(0, 0),
                        destination: pos(1, 0),
                    },
                    ListMove::Unassign {
                        entity: 1,
                        start: 1,
                        end: 2,
                    },
                ]),
            ]),
            vec![vec![1, 2, 3], vec![7, 5, 6, 0], initial[2].clone()],
            8,
        ),
        (ListMove::Composite(vec![]), initial.clone(), 0),
    ];
    for (movement, expected, clones) in cases {
        for commit in [false, true] {
            let mut a = p.initial_assignment();
            reset(&t);
            let edit = p.apply(&mut a, &movement).unwrap();
            assert_eq!(ids(&edit), expected);
            for (e, list) in initial.iter().enumerate() {
                assert_eq!(
                    edit.original_list(e)
                        .unwrap()
                        .iter()
                        .map(|v| v.id)
                        .collect::<Vec<_>>(),
                    *list
                );
            }
            assert!(edit.original_list(3).is_none());
            assert_eq!(counts(&t).iter().sum::<usize>(), clones);
            assert_eq!(&counts(&t)[8..12], &[0; 4]);
            let before_drop = counts(&t);
            if commit {
                edit.commit();
            } else {
                drop(edit);
            }
            assert_eq!(counts(&t), before_drop, "undo must not clone");
            assert_eq!(ids(&a), if commit { &expected } else { &initial }.clone());
        }
    }
}
#[test]
fn full_original_is_lazy_cached_and_recovers_from_clone_panic() {
    let (p, t) = fixture();
    let mut a = p.initial_assignment();
    let before = ids(&a);
    reset(&t);
    {
        let edit = p.apply(&mut a, &swap()).unwrap();
        assert_eq!(counts(&t).iter().sum::<usize>(), 8);
        // An untouched value is not cloned until the full historical view is requested.
        t.panic_id.set(Some(10));
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                edit.original();
            }))
            .is_err()
        );
        t.panic_id.set(None);
        reset(&t);
        let original = edit.original();
        assert_eq!(ids(original), before);
        assert_eq!(counts(&t), vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]);
        assert!(std::ptr::eq(original, edit.original()));
        assert_eq!(counts(&t), vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0]);
        // Rollback itself is safe even when cloning any changed value would panic.
        t.panic_id.set(Some(0));
    }
    t.panic_id.set(None);
    assert_eq!(ids(&a), before);
    // A panic escaping original() must also roll back the move.
    t.panic_id.set(Some(10));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            p.apply(&mut a, &swap()).unwrap().original();
        }))
        .is_err()
    );
    t.panic_id.set(None);
    assert_eq!(ids(&a), before);
    // Retain Send + Sync for ordinary value types despite lazy materialization.
    fn send_sync<T: Send + Sync>(_: &T) {}
    let simple = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let mut state = simple.initial_assignment();
    send_sync(
        &simple
            .apply(&mut state, &ListMove::Composite(vec![]))
            .unwrap(),
    );
}
#[test]
fn late_errors_and_assignment_clone_panics_restore_all_prior_edits() {
    let (p, t) = fixture();
    let mut a = p.initial_assignment();
    let before = ids(&a);
    let invalids = [
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(50, 0),
        },
        ListMove::SubListChange {
            entity: 1,
            start: 0,
            end: 100,
            destination: pos(0, 0),
            reverse: false,
        },
        ListMove::SubListSwap {
            left: range(0, 0, 3),
            right: range(0, 1, 4),
            reverse: false,
        },
    ];
    for bad in invalids {
        assert!(
            p.apply(&mut a, &ListMove::Composite(vec![swap(), bad]))
                .is_err()
        );
        assert_eq!(ids(&a), before);
    }
    let assignment = ListMove::Assign {
        value: Value {
            id: 12,
            tracking: t.clone(),
        },
        destination: pos(0, 0),
    };
    t.panic_id.set(Some(12));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = p.apply(&mut a, &ListMove::Composite(vec![swap(), assignment]));
        }))
        .is_err()
    );
    t.panic_id.set(None);
    assert_eq!(ids(&a), before);
    // Snapshotting later invalid references must not change the first reported error.
    let movement = ListMove::Composite(vec![
        ListMove::Assign {
            value: Value {
                id: 0,
                tracking: t.clone(),
            },
            destination: pos(0, 0),
        },
        ListMove::Unassign {
            entity: 99,
            start: 0,
            end: 1,
        },
    ]);
    assert!(matches!(
        p.apply(&mut a, &movement),
        Err(ListError::AlreadyAssigned)
    ));
    assert_eq!(ids(&a), before);
}
fn direct(a: &ListAssignment<Value>) -> SimpleScore {
    SimpleScore::new(
        a.lists()
            .iter()
            .enumerate()
            .flat_map(|(e, list)| {
                list.iter()
                    .enumerate()
                    .map(move |(i, v)| ((e + 1) * (i + 1) * (v.id + 1)) as i64)
            })
            .sum(),
    )
}
#[test]
fn scoped_scoring_reads_changed_lists_without_materializing_full_original() {
    let (p, t) = fixture();
    let mut net = ListConstraintNetwork::<Value, SimpleScore>::new(3, 13);
    for v in 0..13 {
        net.add_constraint(format!("value {v}"), vec![ListScope::Value(v)], |facts| {
            let ListFact::Value { value, element } = &facts[0] else {
                unreachable!()
            };
            SimpleScore::new(element.as_ref().map_or(0, |at| {
                ((at.position.entity + 1) * (at.position.index + 1) * (value.id + 1)) as i64
            }))
        })
        .unwrap();
    }
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let before = ids(d.assignment());
    let old = d.score();
    reset(&t);
    {
        let edit = d.apply(&swap()).unwrap();
        assert_eq!(edit.score(), direct(edit.assignment()));
        assert_eq!(edit.constraint_evaluations(), 8);
        assert_eq!(counts(&t), vec![1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0]);
    }
    assert_eq!(ids(d.assignment()), before);
    assert_eq!(d.score(), old);
    d.apply(&swap()).unwrap().commit();
    assert_eq!(d.score(), direct(d.assignment()));
    d.assert_consistent().unwrap();
}
#[test]
fn stream_scoring_preserves_unrelated_values_and_restores_graph_on_rejection() {
    let (p, t) = fixture();
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<Value>::new(&mut b);
    b.score("positions", sources.values.port(), |row| {
        SimpleScore::new(row.position.map_or(0, |at| {
            ((at.entity + 1) * (at.index + 1) * (row.value.id + 1)) as i64
        }))
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let before = ids(d.assignment());
    let old = d.score();
    reset(&t);
    {
        let edit = d.apply(&swap()).unwrap();
        assert_eq!(edit.score(), direct(edit.assignment()));
        assert_eq!(&counts(&t)[8..12], &[0; 4]);
    }
    assert_eq!(ids(d.assignment()), before);
    assert_eq!(d.score(), old);
    d.apply(&swap()).unwrap().commit();
    assert_eq!(d.score(), direct(d.assignment()));
    d.assert_consistent().unwrap();
}
