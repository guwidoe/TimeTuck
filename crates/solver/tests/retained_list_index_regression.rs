//! Original cache and mutation regressions; no upstream-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn assert_layout(index: &RetainedListIndex, a: &ListAssignment<usize>, n: usize) {
    let snapshot = index.snapshot().expect("current committed cache");
    assert_eq!(snapshot.positions.len(), n);
    for value in 0..n {
        let expected = a
            .lists()
            .iter()
            .enumerate()
            .find_map(|(e, row)| row.iter().position(|v| *v == value).map(|i| pos(e, i)));
        assert_eq!(snapshot.positions[value], expected);
        assert_eq!(index.location(value), Some(expected));
    }
    assert_eq!(index.location(n), None);
}
macro_rules! mutations {
    ($d:ident, $p:ident) => {{
        let first = $d.retained_list_index();
        assert_layout(&first, $d.assignment(), 6);
        let start = first.snapshot().unwrap();
        drop($d.assignment().clone());
        assert_eq!(first.snapshot(), Some(start.clone()));
        assert_eq!($d.retained_list_index().snapshot(), Some(start.clone()));
        let change = ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, 0),
        };
        {
            let _trial = $d.apply(&change).unwrap();
            assert_eq!(first.snapshot(), Some(start.clone()));
        }
        assert_eq!(first.snapshot(), Some(start.clone()));
        $d.apply(&change).unwrap().commit();
        assert!(first.snapshot().is_none());
        let next = $d.retained_list_index();
        assert!(first.same_instance(&next));
        assert_layout(&first, $d.assignment(), 6);
        assert_eq!(
            first.snapshot().unwrap().refreshed_entities,
            start.refreshed_entities + 2
        );
        // Union two unread edits, including overlapping old/new row memberships.
        $d.apply(&ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        })
        .unwrap()
        .commit();
        $d.apply(&ListMove::SubListChange {
            entity: 1,
            start: 0,
            end: 2,
            destination: pos(2, 0),
            reverse: true,
        })
        .unwrap()
        .commit();
        $d.retained_list_index();
        assert_layout(&first, $d.assignment(), 6);
        let before = $d.assignment().clone();
        {
            let mut edits = $d.edit_session().unwrap();
            edits
                .apply(&ListMove::Assign {
                    value: 5,
                    destination: pos(2, 2),
                })
                .unwrap();
            assert_layout(
                &edits.director().retained_list_index(),
                edits.assignment(),
                6,
            );
        }
        assert_eq!($d.assignment(), &before);
        assert!(first.snapshot().is_none());
        assert!(first.same_instance(&$d.retained_list_index()));
        assert_layout(&first, $d.assignment(), 6);
        let old = first.snapshot();
        assert!(
            $d.apply(&ListMove::Assign {
                value: 99,
                destination: pos(0, 0)
            })
            .is_err()
        );
        assert_eq!(first.snapshot(), old);
        $d.apply(&ListMove::Unassign {
            entity: 2,
            start: 0,
            end: 1,
        })
        .unwrap()
        .commit();
        $d.retained_list_index();
        assert_layout(&first, $d.assignment(), 6);
        // Retained phases may install full best snapshots and later construct again.
        $d.run_solver(SolverSequence {
            limits: Default::default(),
            phases: vec![
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 4,
                    ..Default::default()
                }),
                SolverPhase::Construction(Default::default()),
            ],
        })
        .unwrap();
        assert!(first.same_instance(&$d.retained_list_index()));
        assert_layout(&first, $d.assignment(), 6);
        $p.validate($d.assignment()).unwrap();
        drop($d);
        assert!(first.snapshot().is_none());
    }};
}
#[test]
fn dense_commits_rollback_and_retained_phases_update_one_index() {
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let n = ListConstraintNetwork::<usize, SimpleScore>::new(3, 6);
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    mutations!(d, p);
}
#[test]
fn stream_commits_rollback_and_retained_phases_update_one_index() {
    let p = ListProblem::new_indexed(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    mutations!(d, p);
}
#[test]
fn stream_assignment_replacement_is_transactional_and_identity_is_director_owned() {
    fn thread_safe<T: Send + Sync>() {}
    thread_safe::<ListAssignment<usize>>();
    thread_safe::<RetainedListIndex>();
    let p = ListProblem::new(
        (0..3).collect(),
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2])],
    )
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let index = d.retained_list_index();
    let original = index.snapshot();
    let replacement = ListAssignment::new(vec![vec![2], vec![1, 0]]);
    {
        let _change = d.replace_assignment(replacement.clone()).unwrap();
        assert_eq!(index.snapshot(), original);
    }
    assert_eq!(index.snapshot(), original);
    d.replace_assignment(replacement.clone()).unwrap().commit();
    assert!(index.snapshot().is_none());
    assert!(index.same_instance(&d.retained_list_index()));
    assert_layout(&index, d.assignment(), 3);
    let n = ListConstraintNetwork::<usize, SimpleScore>::new(2, 3);
    let other = ListScoreDirector::new(&p, &n, d.assignment().clone()).unwrap();
    assert!(!index.same_instance(&other.retained_list_index()));
    assert_eq!(
        format!("{replacement:?}"),
        "ListAssignment { lists: [[2], [1, 0]] }"
    );
}
#[derive(Clone, Debug)]
struct Checked {
    id: usize,
    comparisons: Rc<Cell<usize>>,
    panic: Rc<Cell<bool>>,
}
impl PartialEq for Checked {
    fn eq(&self, other: &Self) -> bool {
        assert!(!self.panic.get(), "equality fixture");
        self.comparisons.set(self.comparisons.get() + 1);
        self.id == other.id
    }
}
impl Eq for Checked {}
#[test]
fn eq_only_registry_caches_lookups_and_retries_failed_publication() {
    let comparisons = Rc::new(Cell::new(0));
    let panic = Rc::new(Cell::new(false));
    let values: Vec<_> = (0..4)
        .map(|id| Checked {
            id,
            comparisons: comparisons.clone(),
            panic: panic.clone(),
        })
        .collect();
    let p = ListProblem::new(
        values.clone(),
        vec![
            ListEntity::new(values[..2].to_vec()),
            ListEntity::new(values[2..].to_vec()),
        ],
    )
    .unwrap();
    let n = ListConstraintNetwork::<Checked, SimpleScore>::new(2, 4);
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let index = d.retained_list_index();
    comparisons.set(0);
    for _ in 0..100 {
        assert!(index.same_instance(&d.retained_list_index()));
        assert_eq!(index.location(3), Some(Some(pos(1, 1))));
    }
    assert_eq!(comparisons.get(), 0);
    d.apply(&ListMove::Change {
        source: pos(0, 0),
        destination: pos(1, 0),
    })
    .unwrap()
    .commit();
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| d.retained_list_index())).is_err());
    assert!(index.snapshot().is_none());
    panic.set(false);
    assert!(index.same_instance(&d.retained_list_index()));
    assert_eq!(index.location(0), Some(Some(pos(1, 0))));
    assert_eq!(index.location(1), Some(Some(pos(0, 0))));
}
#[test]
fn sparse_best_snapshot_and_scoring_panic_cannot_publish_stale_positions() {
    let p = ListProblem::new_indexed(
        (0..40).collect(),
        (0..40).map(|id| ListEntity::new(vec![id])).collect(),
    )
    .unwrap();
    let panic = Cell::new(false);
    let mut n = ListConstraintNetwork::<usize, SimpleScore>::new(40, 40);
    n.add_constraint("receiver", vec![ListScope::Entity(0)], |facts| {
        assert!(!panic.get(), "score fixture");
        let ListFact::Entity(row) = &facts[0] else {
            unreachable!()
        };
        SimpleScore::new(row.len() as i64)
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let index = d.retained_list_index();
    let before = index.snapshot();
    panic.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _trial = d.apply(&ListMove::Change {
                source: pos(1, 0),
                destination: pos(0, 0),
            });
        }))
        .is_err()
    );
    panic.set(false);
    assert_eq!(index.snapshot(), before);
    assert_layout(&index, d.assignment(), 40);
    d.run_solver(SolverSequence {
        limits: Default::default(),
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 2,
            ..Default::default()
        })],
    })
    .unwrap();
    assert!(index.same_instance(&d.retained_list_index()));
    assert_layout(&index, d.assignment(), 40);
    d.assert_consistent().unwrap();
}
