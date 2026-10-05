//! Original native enumeration, sampling and score-director integration checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

const MOVABLE: DatasetEnumerationOptions = DatasetEnumerationOptions {
    include_null: true,
    exclude_pinned: true,
};

#[test]
fn null_is_permanent_and_distinct_from_duplicate_unassigned_payloads() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Option<i32>>();
    let d = EnumeratedDataset::capture(
        &mut b,
        s.port(),
        DatasetEnumerationOptions {
            include_null: true,
            exclude_pinned: false,
        },
        |_| panic!("disabled pin predicate"),
    )
    .unwrap();
    let plain = EnumeratedDataset::capture(
        &mut b,
        s.port(),
        DatasetEnumerationOptions::default(),
        |_| false,
    )
    .unwrap();
    let mut g = b.build();
    assert_eq!(d.len(&g).unwrap(), 1);
    assert!(!d.is_empty(&g).unwrap());
    assert!(plain.is_empty(&g).unwrap());
    let mut tx = g.transaction();
    for id in [7, 2] {
        tx.apply(&s, TupleChange::put(id, None)).unwrap();
    }
    tx.commit().unwrap();
    let held = d.snapshot(&g).unwrap();
    assert_eq!(
        held.iter().copied().collect::<Vec<_>>(),
        [None, Some(None), Some(None)]
    );
    assert_eq!(plain.len(&g).unwrap(), 2);
    let mut tx = g.transaction();
    for id in [2, 7] {
        tx.apply(&s, TupleChange::retract(id)).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(d.iter(&g).unwrap().collect::<Vec<_>>(), [None]);
    assert_eq!(held.len(), 3);
    g.assert_consistent().unwrap();
}

#[test]
fn pin_updates_and_panics_restore_membership_and_held_snapshots() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (i32, bool)>();
    let d = EnumeratedDataset::capture(&mut b, s.port(), MOVABLE, |row| {
        calls.set(calls.get() + 1);
        assert_ne!(row.0, 99);
        row.1
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(4, (10, false))).unwrap();
    tx.apply(&s, TupleChange::put(8, (20, true))).unwrap();
    tx.commit().unwrap();
    let before = d.snapshot(&g).unwrap();
    let count = calls.get();
    assert_eq!(d.iter(&g).unwrap().count(), 2);
    assert_eq!(d.len(&g).unwrap(), 2);
    assert_eq!(calls.get(), count, "reading must not re-evaluate pins");
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(4, (10, true))).unwrap();
    tx.apply(&s, TupleChange::put(8, (20, false))).unwrap();
    let candidate = d.snapshot(tx.graph().unwrap()).unwrap();
    assert_eq!(
        candidate.iter().copied().collect::<Vec<_>>(),
        [None, Some((20, false))]
    );
    drop(tx);
    assert_eq!(
        d.snapshot(&g).unwrap().iter().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(4)).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(
            || tx.apply(&s, TupleChange::put(8, (99, false)))
        ))
        .is_err()
    );
    drop(tx);
    assert_eq!(
        d.snapshot(&g).unwrap().iter().collect::<Vec<_>>(),
        before.iter().collect::<Vec<_>>()
    );
    assert_eq!(candidate.len(), 2);
    g.assert_consistent().unwrap();
}

#[test]
fn every_enumeration_operation_rejects_a_foreign_graph_even_for_null_only_views() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let d = EnumeratedDataset::capture(&mut b, s.port(), MOVABLE, |_| false).unwrap();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    for exclude_pinned in [false, true] {
        assert!(matches!(
            EnumeratedDataset::capture(
                &mut other,
                s.port(),
                DatasetEnumerationOptions {
                    exclude_pinned,
                    ..MOVABLE
                },
                |_| false
            ),
            Err(GraphError::ForeignPort)
        ));
    }
    let g = other.build();
    assert!(matches!(d.iter(&g), Err(GraphError::ForeignPort)));
    assert!(matches!(d.len(&g), Err(GraphError::ForeignPort)));
    assert!(matches!(d.is_empty(&g), Err(GraphError::ForeignPort)));
    assert!(matches!(d.snapshot(&g), Err(GraphError::ForeignPort)));
}

fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]).allowing_unassigned(),
        Variable::new(vec![1, 2]).with_initial(2),
    ])
    .unwrap()
}

#[test]
fn basic_views_keep_unassigned_entities_and_sample_committed_rows() {
    let p = basic_problem();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source();
    let d = basic_entity_dataset(&mut b, &source, &p, MOVABLE).unwrap();
    let all =
        basic_entity_dataset(&mut b, &source, &p, DatasetEnumerationOptions::default()).unwrap();
    let mut director =
        StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let rows = |g: &StreamGraph<'_, SimpleScore>| {
        d.iter(g).unwrap().map(|r| r.copied()).collect::<Vec<_>>()
    };
    let before = vec![None, Some((1, None)), Some((2, Some(2)))];
    assert_eq!(rows(director.graph()), before);
    assert_eq!(all.len(director.graph()).unwrap(), 3);
    let movement = Move::Change {
        variable: 1,
        value: Some(1),
    };
    let trial = director.apply(&movement).unwrap();
    assert_eq!(
        rows(trial.graph()),
        [None, Some((1, Some(1))), Some((2, Some(2)))]
    );
    drop(trial);
    assert_eq!(rows(director.graph()), before);
    assert!(
        director
            .apply(&Move::Change {
                variable: 0,
                value: Some(2)
            })
            .is_err()
    );
    director.apply(&movement).unwrap().commit();
    let snapshot = d.snapshot(director.graph()).unwrap();
    let sample = snapshot
        .samples(SizeSampler::all(), 42)
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(sample.len(), 3);
    for row in snapshot.iter() {
        assert!(sample.contains(row));
    }
    director.assert_consistent().unwrap();
}

fn list_problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![10, 11, 12, 13, 14, 15],
        vec![
            ListEntity::new(vec![10, 11]).with_pinned(true),
            ListEntity::new(vec![12, 13]).with_pinned_prefix(1),
            ListEntity::new(vec![14]),
        ],
    )
    .unwrap()
    .allowing_unassigned()
}

#[test]
fn list_views_follow_every_single_move_and_rollback_against_model_reference() {
    let p = list_problem();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let entities = list_entity_dataset(&mut b, &sources, &p, MOVABLE).unwrap();
    let values = list_value_dataset(&mut b, &sources, &p, MOVABLE).unwrap();
    let mut director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let before = values.snapshot(director.graph()).unwrap();
    let assert_rows = |g: &StreamGraph<'_, SimpleScore>, a: &ListAssignment<i32>| {
        let actual: Vec<_> = entities.iter(g).unwrap().map(|r| r.cloned()).collect();
        assert_eq!(
            actual,
            vec![
                None,
                Some((1, a.lists()[1].clone())),
                Some((2, a.lists()[2].clone()))
            ]
        );
        let rows: Vec<_> = values.iter(g).unwrap().collect();
        assert_eq!(rows[0], None);
        assert_eq!(
            rows[1..]
                .iter()
                .map(|r| r.unwrap().value)
                .collect::<Vec<_>>(),
            [13, 14, 15]
        );
        for row in rows.into_iter().flatten() {
            let element = a.element(&row.value);
            assert_eq!(row.position, element.as_ref().map(|e| e.position));
            assert_eq!(
                row.previous,
                element.as_ref().and_then(|e| e.previous.copied())
            );
            assert_eq!(row.next, element.as_ref().and_then(|e| e.next.copied()));
        }
    };
    let moves: Vec<_> = list_moves(&p, director.assignment(), Neighborhood::ChangeAndSwap)
        .unwrap()
        .collect();
    assert!(!moves.is_empty());
    for movement in moves {
        let trial = director.apply(&movement).unwrap();
        assert_rows(trial.graph(), trial.assignment());
        drop(trial);
        assert_rows(director.graph(), director.assignment());
    }
    director
        .apply(&ListMove::Assign {
            value: 15,
            destination: ListPosition {
                entity: 1,
                index: 2,
            },
        })
        .unwrap()
        .commit();
    assert_rows(director.graph(), director.assignment());
    assert_eq!(
        before
            .iter()
            .flatten()
            .find(|r| r.value == 15)
            .unwrap()
            .position,
        None
    );
    director.assert_consistent().unwrap();
}

#[test]
fn mixed_views_rollback_and_commit_one_joint_candidate() {
    let p = MixedProblem::new(basic_problem(), list_problem());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut b);
    let basics = basic_entity_dataset(&mut b, &sources.basic, p.basic(), MOVABLE).unwrap();
    let entities = list_entity_dataset(&mut b, &sources.lists, p.lists(), MOVABLE).unwrap();
    let values = list_value_dataset(&mut b, &sources.lists, p.lists(), MOVABLE).unwrap();
    let mut director =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 1,
            value: Some(2),
        },
        lists: ListMove::Assign {
            value: 15,
            destination: ListPosition {
                entity: 2,
                index: 1,
            },
        },
    };
    let initial = basics.snapshot(director.graph()).unwrap();
    let trial = director.apply(&movement).unwrap();
    assert!(
        basics
            .iter(trial.graph())
            .unwrap()
            .any(|r| r == Some(&(1, Some(2))))
    );
    assert!(
        entities
            .iter(trial.graph())
            .unwrap()
            .any(|r| r == Some(&(2, vec![14, 15])))
    );
    let held = values.snapshot(trial.graph()).unwrap();
    assert_eq!(
        held.iter()
            .flatten()
            .find(|r| r.value == 15)
            .unwrap()
            .position,
        Some(ListPosition {
            entity: 2,
            index: 1
        })
    );
    drop(trial);
    assert_eq!(
        basics
            .snapshot(director.graph())
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        initial.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        values
            .iter(director.graph())
            .unwrap()
            .flatten()
            .find(|r| r.value == 15)
            .unwrap()
            .position,
        None
    );
    director.apply(&movement).unwrap().commit();
    assert_eq!(
        values
            .snapshot(director.graph())
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        held.iter().collect::<Vec<_>>()
    );
    director.assert_consistent().unwrap();
}
