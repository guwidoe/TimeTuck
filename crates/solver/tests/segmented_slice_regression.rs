use timetuck::*;
#[test]
fn empty_segments_indices_ranges_and_mutations_match_flat_reference() {
    for lengths in [[0, 0, 0, 0], [0, 3, 0, 2], [2, 0, 2, 1], [1, 1, 1, 1]] {
        let mut start = 0;
        let original: Vec<Vec<usize>> = lengths
            .into_iter()
            .map(|n| {
                let row = (start..start + n).collect();
                start += n;
                row
            })
            .collect();
        let v = SegmentedSlice::new(original.iter().map(Vec::as_slice));
        assert_eq!(v.len(), start);
        for (e, row) in original.iter().enumerate() {
            for (i, value) in row.iter().enumerate() {
                let index = v.flat_index(e, i).unwrap();
                assert_eq!(index, *value);
                assert_eq!(v.locate(index), Some((e, i)));
                assert_eq!(v.get(index), Some(value));
            }
            assert_eq!(v.flat_index(e, row.len()), None);
        }
        assert_eq!(v.flat_index(4, 0), None);
        assert_eq!(v.get(start), None);
        assert_eq!(v.get(usize::MAX), None);
        assert!(v.range(start + 1..start + 1).is_none());
        for from in 0..=start {
            for to in from..=start {
                let mut rows = original.clone();
                let mut expected: Vec<_> = (0..start).collect();
                expected[from..to].reverse();
                let mut v = SegmentedSliceMut::new(rows.iter_mut().map(Vec::as_mut_slice));
                assert!(v.reverse(from..to));
                assert_eq!(v.view().iter().copied().collect::<Vec<_>>(), expected);
                assert!(!v.swap(0, usize::MAX));
                assert!(!v.reverse(0..start + 1));
                assert!(v.range_mut(start + 1..start + 1).is_none());
                assert_eq!(v.replace(usize::MAX, 8), None);
                assert_eq!(
                    v.view().iter().rev().copied().collect::<Vec<_>>(),
                    expected.into_iter().rev().collect::<Vec<_>>()
                );
            }
        }
    }
}
#[test]
fn overlapping_immutable_segments_and_nonclone_payloads_remain_borrowed() {
    struct Payload(usize);
    let values = [Payload(1), Payload(2), Payload(3)];
    let view = SegmentedSlice::new([&values[..], &[], &values[1..]]);
    assert_eq!(
        view.iter().map(|v| v.0).collect::<Vec<_>>(),
        [1, 2, 3, 2, 3]
    );
    assert!(std::ptr::eq(view.get(1).unwrap(), view.get(3).unwrap()));
    let mut rows = vec![vec![Payload(1)], vec![], vec![Payload(2), Payload(3)]];
    repartition_lists(&mut rows, &[0, 2, 1]).unwrap();
    assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [0, 2, 1]);
    assert_eq!(
        rows.into_iter().flatten().map(|v| v.0).collect::<Vec<_>>(),
        [1, 2, 3]
    );
}
#[test]
fn repartition_preserves_order_and_rejects_invalid_shapes_atomically() {
    let original = vec![vec![0, 1], vec![], vec![2, 3, 4]];
    for a in 0..=5 {
        for b in 0..=5 - a {
            let mut rows = original.clone();
            let lengths = [a, b, 5 - a - b];
            repartition_lists(&mut rows, &lengths).unwrap();
            assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), lengths);
            assert_eq!(
                rows.iter().flatten().copied().collect::<Vec<_>>(),
                [0, 1, 2, 3, 4]
            );
            repartition_lists(&mut rows, &[2, 0, 3]).unwrap();
            assert_eq!(rows, original);
        }
    }
    let mut rows = original.clone();
    for (lengths, error) in [
        (vec![5], RepartitionError::SegmentCount),
        (vec![1, 1, 1], RepartitionError::TotalLength),
        (vec![usize::MAX, 1, 0], RepartitionError::LengthOverflow),
    ] {
        assert_eq!(repartition_lists(&mut rows, &lengths), Err(error));
        assert_eq!(rows, original);
    }
    repartition_lists::<usize>(&mut [], &[]).unwrap();
}
#[test]
fn kopt_repartition_sampling_filters_dependency_cycles() {
    let base = ListProblem::new(
        (0..6).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]),
            ListEntity::new(vec![3, 4, 5]),
        ],
    )
    .unwrap();
    let p = base
        .clone()
        .with_dependencies(vec![vec![], vec![0], vec![1], vec![2], vec![3], vec![4]])
        .unwrap();
    let a = p.initial_assignment();
    let config = ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 17, 2000);
    let mut expected = vec![];
    let mut rejected = 0;
    for m in sample_list_k_opt_repartitions(&base, &a, config).unwrap() {
        let mut current = a.clone();
        match p.apply(&mut current, &m) {
            Ok(trial) => {
                p.validate(&trial).unwrap();
                expected.push(m.clone());
            }
            Err(ListError::DependencyCycle { .. }) => rejected += 1,
            Err(e) => panic!("unexpected model error: {e}"),
        }
        assert_eq!(current, a);
    }
    assert!(!expected.is_empty());
    assert!(rejected > 0);
    let actual: Vec<_> = sample_list_k_opt_repartitions(&p, &a, config)
        .unwrap()
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn kopt_iterator_setup_and_invalid_tour_do_not_clone_value_payloads() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct CountedValue(usize, Rc<Cell<usize>>);
    impl Clone for CountedValue {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for CountedValue {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for CountedValue {}
    let clones = Rc::new(Cell::new(0));
    let v = |i| CountedValue(i, clones.clone());
    let p = ListProblem::new(
        (0..6).map(v).collect(),
        vec![
            ListEntity::new((0..3).map(v).collect()),
            ListEntity::new((3..6).map(v).collect()),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let config = ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 1, 100);
    let _iterator = sample_list_k_opt_repartitions(&p, &a, config).unwrap();
    assert_eq!(clones.get(), 0);
    assert!(
        p.k_opt_repartition_move(&a, &[0, 1], &[], &[], (0, 1), &[3, 3])
            .is_err()
    );
    assert_eq!(clones.get(), 0);
}
