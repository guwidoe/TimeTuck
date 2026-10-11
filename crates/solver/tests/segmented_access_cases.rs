// Adapted from pinned Timefold Community MultipleDelegateListTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn rows() -> Vec<Vec<String>> {
    vec![
        vec!["a".into(), "b".into(), "c".into()],
        vec!["d".into(), "e".into()],
        vec!["f".into()],
    ]
}
fn view<'a>(rows: &'a [Vec<String>]) -> SegmentedSlice<'a, String> {
    SegmentedSlice::new(rows.iter().map(Vec::as_slice))
}
fn expected() -> Vec<String> {
    ["a", "b", "c", "d", "e", "f"].map(String::from).to_vec()
}
fn bounds(index: isize) -> SegmentAccessError {
    SegmentAccessError::Slot {
        index: index as i128,
        length: 6,
    }
}
#[test]
fn get_index_of_value() {
    let rows = rows();
    let flat = view(&rows);
    let owners = ["e1", "e2", "e3"];
    let index = SegmentedOwnerLookup::new(&flat, &owners).unwrap();
    let lookup = |v: &String| match v.as_str() {
        "a" => Some(("e1", 0)),
        "b" => Some(("e1", 1)),
        "c" => Some(("e1", 2)),
        "d" => Some(("e2", 0)),
        "e" => Some(("e2", 1)),
        "f" => Some(("e3", 0)),
        _ => None,
    };
    for (i, value) in expected().iter().enumerate() {
        assert_eq!(index.resolve_assigned_value(value, lookup).unwrap(), i);
    }
    let error = index
        .resolve_assigned_value(&String::from("g"), lookup)
        .unwrap_err();
    assert_eq!(error, SegmentAccessError::MissingValue("g".into()));
    assert_eq!(
        error.to_string(),
        "Value (g) is not contained in any entity list"
    );
}
#[test]
fn to_array_using_supplied() {
    let rows = rows();
    let v = view(&rows);
    let expected = expected();
    let mut supplied = vec![None; 6];
    let pointer = supplied.as_ptr();
    let out = v.write_segment_references(&mut supplied);
    assert!(out.uses_supplied_storage());
    assert_eq!(out.segment_references().as_ptr(), pointer);
    assert_eq!(
        out.segment_references()
            .iter()
            .map(|v| v.cloned())
            .collect::<Vec<_>>(),
        expected.iter().cloned().map(Some).collect::<Vec<_>>()
    );
    drop(out);
    let g = String::from("g");
    let mut supplied = vec![None; 7];
    supplied[6] = Some(&g);
    let pointer = supplied.as_ptr();
    let out = v.write_segment_references(&mut supplied);
    assert!(out.uses_supplied_storage());
    assert_eq!(out.segment_references().as_ptr(), pointer);
    assert_eq!(
        out.segment_references()
            .iter()
            .map(|v| v.cloned())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .cloned()
            .map(Some)
            .chain([None])
            .collect::<Vec<_>>()
    );
    drop(out);
    let mut supplied = vec![None; 5];
    let pointer = supplied.as_ptr();
    let out = v.write_segment_references(&mut supplied);
    assert!(!out.uses_supplied_storage());
    assert_ne!(out.segment_references().as_ptr(), pointer);
    assert_eq!(
        out.segment_references()
            .iter()
            .map(|v| v.cloned())
            .collect::<Vec<_>>(),
        expected.iter().cloned().map(Some).collect::<Vec<_>>()
    );
}
#[test]
fn get() {
    let rows = rows();
    let v = view(&rows);
    for (i, value) in expected().iter().enumerate() {
        assert_eq!(v.segment_slot(i as isize).unwrap(), value);
    }
    for i in [-1, 6] {
        let error = v.segment_slot(i).unwrap_err();
        assert_eq!(error, bounds(i));
        assert_eq!(
            error.to_string(),
            format!("Index ({i}) out of bounds for a list of size (6)")
        );
    }
}
#[test]
fn set() {
    let mut rows = rows();
    let mut v = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
    for (i, value) in expected().iter().enumerate() {
        assert_eq!(
            &v.replace_segment_slot(i as isize, format!("v{i}")).unwrap(),
            value
        );
        let flat = v.view();
        match i {
            0..=2 => {
                assert_eq!(flat.segment(0).unwrap()[i], format!("v{i}"));
                assert_eq!(flat.segment(1).unwrap(), ["d", "e"]);
                assert_eq!(flat.segment(2).unwrap(), ["f"]);
            }
            3..=4 => {
                assert_eq!(flat.segment(1).unwrap()[i - 3], format!("v{i}"));
                assert_eq!(flat.segment(0).unwrap(), ["v0", "v1", "v2"]);
                assert_eq!(flat.segment(2).unwrap(), ["f"]);
            }
            5 => {
                assert_eq!(flat.segment(2).unwrap()[i - 5], format!("v{i}"));
                assert_eq!(flat.segment(0).unwrap(), ["v0", "v1", "v2"]);
                assert_eq!(flat.segment(1).unwrap(), ["v3", "v4"]);
            }
            _ => unreachable!(),
        }
    }
    for i in [-1, 6] {
        let error = v.replace_segment_slot(i, String::new()).unwrap_err();
        assert_eq!(error, bounds(i));
        assert_eq!(
            error.to_string(),
            format!("Index ({i}) out of bounds for a list of size (6)")
        );
    }
}
fn changed(rows: &[Vec<String>]) {
    assert_eq!(rows, [vec!["v0", "v1", "v2"], vec!["v3", "v4"], vec!["v5"]]);
}
fn reject_cursor(cursor: &mut SegmentedListCursor<'_, '_, String>) {
    assert_eq!(
        cursor.reject_cursor_insert(&String::new()),
        Err(SegmentAccessError::FixedLayout)
    );
    assert_eq!(
        cursor.reject_cursor_remove(),
        Err(SegmentAccessError::FixedLayout)
    );
}
#[test]
fn list_iterator() {
    let mut rows = rows();
    let mut v = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
    let mut cursor = v.segment_cursor_at(0).unwrap();
    let expected = expected();
    assert!(!cursor.has_backward_slot());
    for (i, value) in expected.iter().enumerate() {
        assert!(cursor.has_forward_slot());
        assert_eq!(cursor.after_next_position(), Some(i + 1));
        assert_eq!(cursor.step_next().unwrap(), value);
        assert!(cursor.has_backward_slot());
    }
    assert!(!cursor.has_forward_slot());
    for i in (0..6).rev() {
        assert!(cursor.has_backward_slot());
        assert_eq!(cursor.previous_slot_index(), Some(i));
        assert_eq!(cursor.step_previous().unwrap(), &expected[i]);
        assert!(cursor.has_forward_slot());
    }
    assert!(!cursor.has_backward_slot());
    for i in 0..6 {
        assert!(cursor.has_forward_slot());
        cursor.replace_upcoming_slot(format!("v{i}")).unwrap();
        assert_eq!(cursor.step_next().unwrap(), &format!("v{i}"));
        assert!(cursor.has_backward_slot());
    }
    reject_cursor(&mut cursor);
    drop(v);
    changed(&rows);
}
#[test]
fn list_iterator_starting_at_index() {
    let mut rows = rows();
    let mut v = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
    let mut cursor = v.segment_cursor_at(6).unwrap();
    let expected = expected();
    assert!(!cursor.has_forward_slot());
    for i in (0..6).rev() {
        assert!(cursor.has_backward_slot());
        assert_eq!(cursor.previous_slot_index(), Some(i));
        assert_eq!(cursor.step_previous().unwrap(), &expected[i]);
        assert!(cursor.has_forward_slot());
    }
    assert!(!cursor.has_backward_slot());
    for (i, value) in expected.iter().enumerate() {
        assert!(cursor.has_forward_slot());
        assert_eq!(cursor.after_next_position(), Some(i + 1));
        assert_eq!(cursor.step_next().unwrap(), value);
        assert!(cursor.has_backward_slot());
    }
    assert!(!cursor.has_forward_slot());
    for i in (0..6).rev() {
        assert!(cursor.has_backward_slot());
        cursor.step_previous().unwrap();
        cursor.replace_upcoming_slot(format!("v{i}")).unwrap();
        assert_eq!(cursor.step_next().unwrap(), &format!("v{i}"));
        cursor.step_previous().unwrap();
        assert!(cursor.has_forward_slot());
    }
    assert!(!cursor.has_backward_slot());
    reject_cursor(&mut cursor);
    drop(v);
    changed(&rows);
}
#[test]
fn unsupported_operations() {
    let mut rows = rows();
    let mut v = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
    let g = String::from("g");
    let gh = [String::from("g"), String::from("h")];
    let a = String::from("a");
    let ab = [String::from("a"), String::from("b")];
    for operation in [
        FixedSegmentRequest::Append(&g),
        FixedSegmentRequest::Insert {
            index: 0,
            value: &g,
        },
        FixedSegmentRequest::Extend(&gh),
        FixedSegmentRequest::InsertAll {
            index: 0,
            values: &gh,
        },
        FixedSegmentRequest::Clear,
        FixedSegmentRequest::RemoveValue(&a),
        FixedSegmentRequest::RemoveAt(0),
        FixedSegmentRequest::RemoveAll(&ab),
        FixedSegmentRequest::RetainAll(&ab),
    ] {
        assert_eq!(
            v.reject_segment_resize(operation),
            Err(SegmentAccessError::FixedLayout)
        );
    }
    drop(v);
    assert_eq!(rows, super_rows());
}
fn super_rows() -> Vec<Vec<String>> {
    rows()
}
