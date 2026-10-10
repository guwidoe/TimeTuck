//! Original rollback, borrowed-before-state and clone-cost regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
#[test]
fn only_edited_values_are_saved_and_full_original_is_lazy() {
    #[derive(Debug)]
    struct Value<'a> {
        id: usize,
        payload: &'static str,
        clones: &'a Cell<usize>,
    }
    impl Clone for Value<'_> {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                id: self.id,
                payload: self.payload,
                clones: self.clones,
            }
        }
    }
    impl PartialEq for Value<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }
    impl Eq for Value<'_> {}
    let basic_clones = Cell::new(0);
    let list_clones = Cell::new(0);
    let b = |id, payload| Value {
        id,
        payload,
        clones: &basic_clones,
    };
    let l = |id, payload| Value {
        id,
        payload,
        clones: &list_clones,
    };
    let p = MixedProblem::new(
        FiniteProblem::new(
            (0..64)
                .map(|i| {
                    Variable::new(vec![b(0, "allowed"), b(1, "allowed")])
                        .with_initial(b(0, "basic"))
                        .with_pinned(i != 0)
                })
                .collect(),
        )
        .unwrap(),
        ListProblem::new(
            (0..69).map(|id| l(id, "registry")).collect(),
            vec![
                ListEntity::new((0..3).map(|id| l(id, "left")).collect()),
                ListEntity::new((3..5).map(|id| l(id, "right")).collect()),
                ListEntity::new((5..69).map(|id| l(id, "pinned")).collect()).with_pinned(true),
            ],
        )
        .unwrap(),
    );
    let mut a = p.initial_assignment();
    let m = MixedMove {
        basic: Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(b(1, "first")),
            },
            Move::Change {
                variable: 0,
                value: Some(b(0, "second")),
            },
            Move::Change {
                variable: 0,
                value: Some(b(1, "last")),
            },
        ]),
        lists: ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 0),
        },
    };
    basic_clones.set(0);
    list_clones.set(0);
    {
        let trial = p.apply(&mut a, &m).unwrap();
        assert_eq!((basic_clones.get(), list_clones.get()), (3, 5));
        assert_eq!(
            trial.original_basic(0).unwrap().as_ref().unwrap().payload,
            "basic"
        );
        assert_eq!(
            trial.original_basic(63).unwrap().as_ref().unwrap().payload,
            "basic"
        );
        assert!(trial.original_basic(64).is_none());
        assert_eq!(trial.original_list(0).unwrap()[1].id, 1);
        assert_eq!(trial.original_list(2).unwrap().len(), 64);
        assert!(trial.original_list(3).is_none());
        assert_eq!((basic_clones.get(), list_clones.get()), (3, 5));
        let original = trial.original();
        assert_eq!(
            original.basic().values()[0].as_ref().unwrap().payload,
            "basic"
        );
        assert_eq!(original.lists().lists()[0][1].payload, "left");
        assert_eq!((basic_clones.get(), list_clones.get()), (67, 74));
        assert!(std::ptr::eq(original, trial.original()));
        assert_eq!((basic_clones.get(), list_clones.get()), (67, 74));
    }
    assert_eq!(a.basic().values()[0].as_ref().unwrap().payload, "basic");
    assert_eq!(
        a.lists().lists()[0]
            .iter()
            .map(|v| v.id)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!((basic_clones.get(), list_clones.get()), (67, 74));
    basic_clones.set(0);
    list_clones.set(0);
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!((basic_clones.get(), list_clones.get()), (3, 5));
    assert_eq!(a.basic().values()[0].as_ref().unwrap().payload, "last");
    assert_eq!(a.lists().lists()[1][0].id, 1);
}

#[derive(Default, Debug)]
struct Faults {
    clones: Cell<usize>,
    equals: Cell<usize>,
    clone_at: Cell<Option<usize>>,
    eq_at: Cell<Option<usize>>,
}
#[derive(Debug)]
struct Bomb {
    id: i32,
    faults: Rc<Faults>,
}
impl Clone for Bomb {
    fn clone(&self) -> Self {
        let n = self.faults.clones.get() + 1;
        self.faults.clones.set(n);
        assert_ne!(
            self.faults.clone_at.get(),
            Some(n),
            "injected clone failure"
        );
        Self {
            id: self.id,
            faults: Rc::clone(&self.faults),
        }
    }
}
impl PartialEq for Bomb {
    fn eq(&self, other: &Self) -> bool {
        let n = self.faults.equals.get() + 1;
        self.faults.equals.set(n);
        assert_ne!(
            self.faults.eq_at.get(),
            Some(n),
            "injected equality failure"
        );
        self.id == other.id
    }
}
impl Eq for Bomb {}
fn fixture(faults: &Rc<Faults>) -> (MixedProblem<Bomb, Bomb>, MixedMove<Bomb, Bomb>) {
    let v = |id| Bomb {
        id,
        faults: Rc::clone(faults),
    };
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![v(0), v(1), v(2)]).with_initial(v(0)),
        ])
        .unwrap(),
        ListProblem::new(
            (0..5).map(v).collect(),
            vec![
                ListEntity::new(vec![v(0), v(1)]).with_value_range((0..5).map(v).collect()),
                ListEntity::new(vec![v(2), v(3)]).with_value_range((0..5).map(v).collect()),
            ],
        )
        .unwrap(),
    );
    let movement = MixedMove {
        basic: Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(v(1)),
            },
            Move::Change {
                variable: 0,
                value: Some(v(2)),
            },
        ]),
        lists: ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 2),
            },
            ListMove::Assign {
                value: v(4),
                destination: pos(0, 1),
            },
        ]),
    };
    (p, movement)
}
fn assert_initial(a: &MixedAssignment<Bomb, Bomb>) {
    assert_eq!(a.basic().values()[0].as_ref().unwrap().id, 0);
    assert_eq!(
        a.lists()
            .lists()
            .iter()
            .map(|r| r.iter().map(|v| v.id).collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        vec![vec![0, 1], vec![2, 3]]
    );
}
#[test]
fn every_clone_and_equality_failure_restores_both_components() {
    let f = Rc::new(Faults::default());
    let (p, m) = fixture(&f);
    let mut a = p.initial_assignment();
    f.clones.set(0);
    f.equals.set(0);
    drop(p.apply(&mut a, &m).unwrap());
    let (clones, equals) = (f.clones.get(), f.equals.get());
    assert_eq!(clones, 7);
    assert!(equals > 10);
    for clone in [false, true] {
        for at in 1..=if clone { clones } else { equals } {
            f.clones.set(0);
            f.equals.set(0);
            if clone {
                f.clone_at.set(Some(at));
            } else {
                f.eq_at.set(Some(at));
            }
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _trial = p.apply(&mut a, &m).unwrap();
                }))
                .is_err(),
                "{clone} at {at}"
            );
            f.clone_at.set(None);
            f.eq_at.set(None);
            assert_initial(&a);
        }
    }
    p.apply(&mut a, &m).unwrap().commit();
    assert_eq!(a.basic().values()[0].as_ref().unwrap().id, 2);
    assert_eq!(a.lists().lists()[1][2].id, 0);
}
#[test]
fn a_failed_lazy_snapshot_can_be_retried_without_disarming_rollback() {
    let f = Rc::new(Faults::default());
    let (p, m) = fixture(&f);
    let mut a = p.initial_assignment();
    {
        let trial = p.apply(&mut a, &m).unwrap();
        f.clone_at.set(Some(f.clones.get() + 2));
        assert!(catch_unwind(AssertUnwindSafe(|| trial.original())).is_err());
        f.clone_at.set(None);
        assert_eq!(trial.basic().values()[0].as_ref().unwrap().id, 2);
        assert_initial(trial.original());
        let clones = f.clones.get();
        assert_initial(trial.original());
        assert_eq!(f.clones.get(), clones);
    }
    assert_initial(&a);
}
