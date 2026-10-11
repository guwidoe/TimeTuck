//! Original ownership lookup work bounds and independent linear references.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
#[derive(Default)]
struct Counts {
    eq: Cell<usize>,
    hash: Cell<usize>,
    clones: Cell<usize>,
}
impl Counts {
    fn reset(&self) {
        self.eq.set(0);
        self.hash.set(0);
        self.clones.set(0);
    }
}
struct Value {
    id: usize,
    counts: Rc<Counts>,
    collision: bool,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.counts.clones.set(self.counts.clones.get() + 1);
        Self {
            id: self.id,
            counts: self.counts.clone(),
            collision: self.collision,
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.counts.eq.set(self.counts.eq.get() + 1);
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.counts.hash.set(self.counts.hash.get() + 1);
        if self.collision {
            0usize.hash(state)
        } else {
            self.id.hash(state)
        }
    }
}
fn counted(n: usize, indexed: bool, collision: bool) -> (ListProblem<Value>, Rc<Counts>) {
    let counts = Rc::new(Counts::default());
    let values = (0..n)
        .map(|id| Value {
            id,
            counts: counts.clone(),
            collision,
        })
        .collect::<Vec<_>>();
    let entities = vec![
        ListEntity::new(values[..n / 2].to_vec()),
        ListEntity::new(values[n / 2..3 * n / 4].to_vec()),
    ];
    let p = if indexed {
        ListProblem::new_indexed(values, entities)
    } else {
        ListProblem::new(values, entities)
    }
    .unwrap()
    .allowing_unassigned();
    (p, counts)
}
#[test]
fn all_mode_avoids_ownership_work_and_partial_indexed_scans_avoid_promotion() {
    for indexed in [false, true] {
        let (p, c) = counted(512, indexed, false);
        let a = p.initial_assignment();
        let view = ListSelectionContext::new(&p, &a).unwrap();
        c.reset();
        let mut source = ListValueSource(ListValueSelection::All);
        assert_eq!(source.size(&view), Some(512));
        assert_eq!((c.eq.get(), c.hash.get(), c.clones.get()), (0, 0, 0));
        assert_eq!(
            source.open(&view).take(3).map(|v| v.id).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!((c.eq.get(), c.hash.get(), c.clones.get()), (0, 0, 3));
        c.reset();
        let mut source = ListValueSource(ListValueSelection::Assigned);
        assert_eq!(source.open(&view).next().unwrap().id, 0);
        assert_eq!((c.eq.get(), c.hash.get(), c.clones.get()), (1, 0, 1));
    }
}
#[test]
fn indexed_full_filtering_has_linear_comparison_work_and_clones_only_outputs() {
    let (p, c) = counted(512, true, false);
    let a = p.initial_assignment();
    let view = ListSelectionContext::new(&p, &a).unwrap();
    for mode in [
        ListValueSelection::Assigned,
        ListValueSelection::Unassigned,
        ListValueSelection::Movable,
    ] {
        let mut source = ListValueSource(mode);
        let expected = if mode == ListValueSelection::Unassigned {
            128
        } else {
            384
        };
        c.reset();
        assert_eq!(source.size(&view), Some(expected));
        assert_eq!((c.hash.get(), c.clones.get()), (384, 0));
        assert!(c.eq.get() <= 768);
        c.reset();
        let output = source.open(&view).map(|v| v.id).collect::<Vec<_>>();
        assert_eq!(output.len(), expected);
        assert_eq!((c.hash.get(), c.clones.get()), (384, expected));
        assert!(c.eq.get() <= 512 + 3 * 384 + 4, "{}", c.eq.get());
        let wanted = if mode == ListValueSelection::Unassigned {
            (384..512).collect::<Vec<_>>()
        } else {
            (0..384).collect()
        };
        assert_eq!(output, wanted);
    }
}
#[test]
fn default_and_tiny_filtering_keep_direct_work_and_collisions_keep_identity_correct() {
    for (n, indexed, collision) in [(256, false, false), (16, true, false), (96, true, true)] {
        let (p, c) = counted(n, indexed, collision);
        let a = p.initial_assignment();
        let view = ListSelectionContext::new(&p, &a).unwrap();
        for mode in [
            ListValueSelection::Assigned,
            ListValueSelection::Unassigned,
            ListValueSelection::Movable,
        ] {
            c.reset();
            let expected = p
                .values()
                .iter()
                .filter(|v| {
                    let assigned = a.location(v).is_some();
                    if mode == ListValueSelection::Unassigned {
                        !assigned
                    } else {
                        assigned
                    }
                })
                .map(|v| v.id)
                .collect::<Vec<_>>();
            let reference_eq = c.eq.get();
            c.reset();
            let actual = ListValueSource(mode)
                .open(&view)
                .map(|v| v.id)
                .collect::<Vec<_>>();
            let actual_eq = c.eq.get();
            assert_eq!(actual, expected);
            if !collision {
                assert_eq!(actual_eq, reference_eq);
                assert_eq!(c.hash.get(), 0);
            } else {
                assert_eq!(c.hash.get(), 3 * n / 4);
            }
            assert_eq!(c.clones.get(), expected.len());
        }
    }
}
fn expected(
    p: &ListProblem<usize>,
    a: &ListAssignment<usize>,
    mode: ListValueSelection,
) -> Vec<usize> {
    p.values()
        .iter()
        .copied()
        .filter(|v| match (mode, a.location(v)) {
            (ListValueSelection::All, _) => true,
            (ListValueSelection::Assigned, p) => p.is_some(),
            (ListValueSelection::Unassigned, p) => p.is_none(),
            (ListValueSelection::Movable, Some(at)) => {
                !p.entities()[at.entity].is_pinned()
                    && at.index >= p.entities()[at.entity].pinned_prefix()
            }
            _ => false,
        })
        .collect()
}
#[test]
fn owned_and_borrowed_modes_match_linear_reference_after_commits_with_pins_and_empty_rows() {
    for indexed in [false, true] {
        let values = (0..128).collect::<Vec<_>>();
        let entities = vec![
            ListEntity::new((0..32).collect()).with_pinned_prefix(8),
            ListEntity::new((32..64).collect()).with_pinned(true),
            ListEntity::new(vec![]),
            ListEntity::new((64..96).collect()),
        ];
        let p = Arc::new(
            if indexed {
                ListProblem::new_indexed(values, entities)
            } else {
                ListProblem::new(values, entities)
            }
            .unwrap()
            .allowing_unassigned(),
        );
        let mut owned = (p.clone(), p.initial_assignment());
        for pass in 0..2 {
            let borrowed = ListSelectionContext::new(&p, &owned.1).unwrap();
            for mode in [
                ListValueSelection::All,
                ListValueSelection::Assigned,
                ListValueSelection::Unassigned,
                ListValueSelection::Movable,
            ] {
                let expected = expected(&p, &owned.1, mode);
                let mut source = ListValueSource(mode);
                assert_eq!(source.size(&borrowed), Some(expected.len()));
                assert_eq!(source.size(&owned), Some(expected.len()));
                assert_eq!(source.open(&borrowed).collect::<Vec<_>>(), expected);
                assert_eq!(source.open(&owned).collect::<Vec<_>>(), expected);
            }
            if pass == 0 {
                p.apply(
                    &mut owned.1,
                    &ListMove::Composite(vec![
                        ListMove::Unassign {
                            entity: 0,
                            start: 8,
                            end: 16,
                        },
                        ListMove::Assign {
                            value: 110,
                            destination: ListPosition {
                                entity: 2,
                                index: 0,
                            },
                        },
                    ]),
                )
                .unwrap()
                .commit();
            }
        }
    }
}
