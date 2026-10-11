// Original segmented access regressions; no upstream-method credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
#[test]
fn cursor_matches_a_flat_oracle_through_empty_segments_and_direction_changes() {
    for lengths in [
        vec![],
        vec![0, 0],
        vec![0, 3, 0, 2, 0],
        vec![1, 0, 1, 0, 1],
        vec![0, 1, 0],
    ] {
        let n = lengths.iter().sum::<usize>();
        for start in 0..=n {
            let mut next = 0i32;
            let mut rows = lengths
                .iter()
                .map(|&length| {
                    (0..length)
                        .map(|_| {
                            let v = next;
                            next += 1;
                            v
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let mut flat = rows.iter().flatten().copied().collect::<Vec<_>>();
            let mut position = start;
            let mut view = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
            let mut cursor = view.segment_cursor_at(start).unwrap();
            let mut state = (start as u64) + 31;
            for step in 0..180 {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                match (state >> 32) % 3 {
                    0 => {
                        if position < n {
                            assert_eq!(*cursor.step_next().unwrap(), flat[position]);
                            position += 1;
                        } else {
                            assert!(matches!(
                                cursor.step_next(),
                                Err(SegmentAccessError::NoForwardSlot)
                            ));
                        }
                    }
                    1 => {
                        if position > 0 {
                            position -= 1;
                            assert_eq!(*cursor.step_previous().unwrap(), flat[position]);
                        } else {
                            assert!(matches!(
                                cursor.step_previous(),
                                Err(SegmentAccessError::NoBackwardSlot)
                            ));
                        }
                    }
                    _ => {
                        if position < n {
                            let old = std::mem::replace(&mut flat[position], -step - 1);
                            assert_eq!(cursor.replace_upcoming_slot(-step - 1).unwrap(), old);
                        } else {
                            assert_eq!(
                                cursor.replace_upcoming_slot(-step - 1),
                                Err(SegmentAccessError::NoForwardSlot)
                            );
                        }
                    }
                }
                assert_eq!(cursor.cursor_position(), position);
                assert_eq!(cursor.has_forward_slot(), position < n);
                assert_eq!(cursor.has_backward_slot(), position > 0);
                assert_eq!(
                    cursor.after_next_position(),
                    (position < n).then_some(position + 1)
                );
                assert_eq!(cursor.previous_slot_index(), position.checked_sub(1));
            }
            drop(view);
            assert_eq!(rows.into_iter().flatten().collect::<Vec<_>>(), flat);
        }
    }
}
#[derive(Debug, PartialEq)]
struct Payload(u32);
#[test]
fn references_and_replacements_support_nonclone_payloads() {
    let mut a = [Payload(1), Payload(2)];
    let mut b = [Payload(3)];
    let first = a.as_ptr();
    let mut view = SegmentedSliceMut::new([a.as_mut_slice(), b.as_mut_slice()]);
    let mut cursor = view.segment_cursor_at(0).unwrap();
    assert!(std::ptr::eq(cursor.step_next().unwrap(), first));
    assert_eq!(
        cursor.replace_upcoming_slot(Payload(8)).unwrap(),
        Payload(2)
    );
    assert_eq!(cursor.step_next().unwrap(), &Payload(8));
    assert_eq!(cursor.step_next().unwrap(), &Payload(3));
    drop(view);
    assert_eq!(a, [Payload(1), Payload(8)]);
    assert_eq!(b, [Payload(3)]);
}
#[test]
fn supplied_buffers_preserve_short_inputs_and_tail_after_one_sentinel() {
    let a = [Payload(1), Payload(2)];
    let b = [Payload(3)];
    let guard = Payload(99);
    let view = SegmentedSlice::new([a.as_slice(), &[], b.as_slice()]);
    for length in 0..=6 {
        let mut supplied = vec![Some(&guard); length];
        let pointer = supplied.as_ptr();
        let out = view.write_segment_references(&mut supplied);
        let refs = out.segment_references();
        assert_eq!(out.uses_supplied_storage(), length >= 3);
        assert_eq!(
            refs[..3].iter().map(|v| v.unwrap().0).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(std::ptr::eq(refs[0].unwrap(), &a[0]));
        if length >= 3 {
            assert_eq!(refs.as_ptr(), pointer);
            if length > 3 {
                assert!(refs[3].is_none());
            }
            assert!(
                refs.iter()
                    .skip(4)
                    .all(|v| std::ptr::eq(v.unwrap(), &guard))
            );
        } else {
            assert_eq!(refs.len(), 3);
            assert_ne!(refs.as_ptr(), pointer);
        }
        drop(out);
        if length < 3 {
            assert!(supplied.iter().all(|v| std::ptr::eq(v.unwrap(), &guard)));
        }
    }
    let empty = SegmentedSlice::<Payload>::new([&[][..]]);
    let mut supplied = [Some(&guard), Some(&guard)];
    let out = empty.write_segment_references(&mut supplied);
    assert_eq!(out.segment_references(), [None, Some(&guard)]);
}
#[test]
fn owner_lookup_rejects_ambiguous_or_stale_metadata_without_mutation() {
    let a = [10, 11];
    let b = [12];
    let flat = SegmentedSlice::new([a.as_slice(), &[], b.as_slice()]);
    assert!(matches!(
        SegmentedOwnerLookup::new(&flat, &[0, 1]),
        Err(SegmentAccessError::OwnerCount {
            owners: 2,
            segments: 3
        })
    ));
    assert!(matches!(
        SegmentedOwnerLookup::new(&flat, &[0, 0, 2]),
        Err(SegmentAccessError::DuplicateOwner)
    ));
    let ids = [7, 4, 9];
    let index = SegmentedOwnerLookup::new(&flat, &ids).unwrap();
    assert_eq!(index.resolve_assigned_value(&12, |_| Some((9, 0))), Ok(2));
    assert_eq!(
        index.resolve_assigned_value(&12, |_| Some((4, 0))),
        Err(SegmentAccessError::StaleOwnerPosition)
    );
    assert_eq!(
        index.resolve_assigned_value(&12, |_| Some((7, 1))),
        Err(SegmentAccessError::StaleOwnerPosition)
    );
    assert_eq!(
        index.resolve_assigned_value(&12, |_| Some((7, 99))),
        Err(SegmentAccessError::StaleOwnerPosition)
    );
    assert_eq!(
        index.resolve_assigned_value(&12, |_| Some((8, 0))),
        Err(SegmentAccessError::MissingValue("12".into()))
    );
    assert!(
        catch_unwind(AssertUnwindSafe(
            || index.resolve_assigned_value(&12, |_| panic!("lookup"))
        ))
        .is_err()
    );
    assert_eq!(flat.iter().copied().collect::<Vec<_>>(), [10, 11, 12]);
    assert_eq!(index.resolve_assigned_value(&12, |_| Some((9, 0))), Ok(2));
}
#[test]
fn invalid_bounds_and_resize_requests_preserve_all_storage() {
    let mut a = [1, 2];
    let mut b = [3];
    let mut v = SegmentedSliceMut::new([a.as_mut_slice(), b.as_mut_slice()]);
    for index in [isize::MIN, -1, 3, isize::MAX] {
        assert_eq!(
            v.replace_segment_slot(index, 7),
            Err(SegmentAccessError::Slot {
                index: index as i128,
                length: 3
            })
        );
    }
    assert!(matches!(
        v.segment_cursor_at(usize::MAX),
        Err(SegmentAccessError::Slot { length: 3, .. })
    ));
    assert!(matches!(
        v.segment_cursor_at(4),
        Err(SegmentAccessError::Slot {
            index: 4,
            length: 3
        })
    ));
    assert_eq!(
        v.reject_segment_resize(FixedSegmentRequest::Insert {
            index: -1,
            value: &8
        }),
        Err(SegmentAccessError::FixedLayout)
    );
    assert_eq!(
        v.reject_segment_resize(FixedSegmentRequest::Clear),
        Err(SegmentAccessError::FixedLayout)
    );
    assert_eq!(v.view().iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
}
fn planned_move(p: &ListProblem<i32>, before: &ListAssignment<i32>) -> ListMove<i32> {
    let flat = SegmentedSlice::new(before.lists().iter().map(Vec::as_slice));
    let owners = [0usize, 1];
    let index = SegmentedOwnerLookup::new(&flat, &owners).unwrap();
    let locate = |value: &i32| {
        before
            .lists()
            .iter()
            .enumerate()
            .find_map(|(entity, row)| row.iter().position(|v| v == value).map(|i| (entity, i)))
    };
    let slot = |value| index.resolve_assigned_value(&value, locate).unwrap();
    let removed = [(slot(11), slot(12)), (slot(14), slot(15))];
    let mut added = [
        vec![removed[0].0, removed[0].1],
        vec![removed[1].0, removed[1].1],
    ];
    {
        let mut v = SegmentedSliceMut::new(added.iter_mut().map(Vec::as_mut_slice));
        let mut c = v.segment_cursor_at(1).unwrap();
        c.replace_upcoming_slot(slot(14)).unwrap();
        c.step_next().unwrap();
        c.replace_upcoming_slot(slot(12)).unwrap();
    }
    let edges = added.iter().map(|row| (row[0], row[1])).collect::<Vec<_>>();
    p.k_opt_repartition_move(
        before,
        &owners,
        &removed,
        &edges,
        (slot(10), slot(11)),
        &[3, 3],
    )
    .unwrap()
}
fn model() -> ListProblem<i32> {
    ListProblem::new(
        (10..16).collect(),
        vec![
            ListEntity::new(vec![10, 11, 12]),
            ListEntity::new(vec![13, 14, 15]),
        ],
    )
    .unwrap()
}
#[test]
fn owner_lookup_and_cursor_prepare_a_real_kopt_repartition_with_undo() {
    let p = model();
    let before = p.initial_assignment();
    let movement = planned_move(&p, &before);
    let mut a = before.clone();
    {
        let trial = p.apply(&mut a, &movement).unwrap();
        assert_eq!(trial.lists(), [vec![10, 11, 14], vec![13, 12, 15]]);
    }
    assert_eq!(a, before);
}
#[test]
fn cursor_built_kopt_move_updates_actual_dense_scores_and_rolls_back() {
    let p = model();
    let before = p.initial_assignment();
    let movement = planned_move(&p, &before);
    let mut n = ListConstraintNetwork::new(2, 6);
    n.add_constraint(
        "weighted",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |rows| {
            let ListFact::Entity(a) = rows[0] else {
                panic!()
            };
            let ListFact::Entity(b) = rows[1] else {
                panic!()
            };
            SimpleScore::new(
                -a.iter()
                    .chain(b.iter())
                    .enumerate()
                    .map(|(i, &v)| (i as i64 + 1) * i64::from(v - 10))
                    .sum::<i64>(),
            )
        },
    )
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, before.clone()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-70));
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-66));
    }
    assert_eq!(d.assignment(), &before);
    assert_eq!(d.score(), SimpleScore::new(-70));
}
#[test]
fn cursor_built_kopt_move_updates_actual_stream_scores_and_rolls_back() {
    let p = model();
    let before = p.initial_assignment();
    let movement = planned_move(&p, &before);
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("weighted", sources.entities.port(), |(entity, values)| {
        SimpleScore::new(
            -values
                .iter()
                .enumerate()
                .map(|(i, &v)| (*entity as i64 * 3 + i as i64 + 1) * i64::from(v - 10))
                .sum::<i64>(),
        )
    })
    .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, b.build(), sources, before.clone()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-70));
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-66));
    }
    assert_eq!(d.assignment(), &before);
    assert_eq!(d.score(), SimpleScore::new(-70));
    d.assert_consistent().unwrap();
}
