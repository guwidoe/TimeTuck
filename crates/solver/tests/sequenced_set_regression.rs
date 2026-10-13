use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    sync::Arc,
};
use timetuck::*;
#[test]
fn rejected_mutations_preserve_cursor_positions_storage_and_live_views() {
    for n in [0, 1, 2, 16, 17, 40] {
        let s = ScalingSequencedSet::new();
        s.extend(0..n).unwrap();
        let storage = s.storage();
        let expected: Vec<_> = (0..n).collect();
        let reverse = s.reversed();
        for backwards in [false, true] {
            let mut cursor = if backwards {
                reverse.cursor()
            } else {
                s.cursor()
            };
            assert_eq!(
                cursor.remove_current(),
                Err(UnsupportedSequencedSetOperation::IteratorRemoval)
            );
            let oriented: Vec<_> = if backwards {
                expected.iter().copied().rev().collect()
            } else {
                expected.clone()
            };
            let mut remaining = oriented.into_iter();
            while cursor.len() > 0 {
                assert_eq!(*cursor.next().unwrap(), remaining.next().unwrap());
                let len = cursor.len();
                assert_eq!(
                    cursor.remove_current(),
                    Err(UnsupportedSequencedSetOperation::IteratorRemoval)
                );
                assert_eq!(cursor.len(), len);
                assert_eq!(cursor.size_hint(), (len, Some(len)));
                assert_eq!(
                    reverse.clear(),
                    Err(UnsupportedSequencedSetOperation::ReadOnlyClear)
                );
                assert_eq!(
                    reverse.insert(100),
                    Err(UnsupportedSequencedSetOperation::ReadOnlyInsert)
                );
                assert_eq!(
                    reverse.remove(&0),
                    Err(UnsupportedSequencedSetOperation::ReadOnlyRemove)
                );
                assert_eq!(s.insert(100), Err(SequencedSetBorrowed));
                if cursor.len() > 0 {
                    assert_eq!(*cursor.next_back().unwrap(), remaining.next_back().unwrap());
                }
            }
            assert!(cursor.next().is_none());
            assert!(cursor.next_back().is_none());
            assert_eq!(
                cursor.remove_current(),
                Err(UnsupportedSequencedSetOperation::IteratorRemoval)
            );
        }
        assert_eq!(values(&s), expected);
        assert_eq!(s.storage(), storage);
        assert_eq!(
            s.insert_first(100),
            Err(UnsupportedSequencedSetOperation::InsertFirst)
        );
        assert_eq!(
            s.insert_last(100),
            Err(UnsupportedSequencedSetOperation::InsertLast)
        );
        assert_eq!(values(&s), expected);
        s.insert(100).unwrap();
        assert_eq!(*reverse.first().unwrap(), 100);
    }
}
#[test]
fn unsupported_bulk_and_read_only_operations_never_consume_streamed_inputs() {
    let s = ScalingSequencedSet::<u32>::new();
    s.extend([1, 2]).unwrap();
    let reverse = s.reversed();
    assert_eq!(
        s.retain_all(std::iter::from_fn(|| -> Option<&u32> {
            panic!("retain input read")
        })),
        Err(UnsupportedSequencedSetOperation::RetainAll)
    );
    assert_eq!(
        s.remove_all(std::iter::from_fn(|| -> Option<&u32> {
            panic!("remove input read")
        })),
        Err(UnsupportedSequencedSetOperation::RemoveAll)
    );
    assert_eq!(
        reverse.extend(std::iter::from_fn(|| -> Option<u32> {
            panic!("extend input read")
        })),
        Err(UnsupportedSequencedSetOperation::ReadOnlyExtend)
    );
    assert_eq!(values(&s), [1, 2]);
    let empty = ScalingSequencedSet::<u32>::new();
    assert_eq!(
        empty.reversed().clear(),
        Err(UnsupportedSequencedSetOperation::ReadOnlyClear)
    );
}
fn values(s: &ScalingSequencedSet<u32>) -> Vec<u32> {
    s.iter().map(|v| *v).collect()
}
fn draw(seed: &mut u64) -> u32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*seed >> 32) as u32
}
#[test]
fn generated_churn_preserves_order_membership_and_double_ended_iteration() {
    for hint in [0, 1, 16, 80] {
        let s = ScalingSequencedSet::with_capacity(hint);
        let reverse = s.reversed();
        let mut expected = Vec::new();
        let mut seed = 91824;
        for step in 0..4000 {
            let value = draw(&mut seed) % 50;
            match draw(&mut seed) % 20 {
                0 => {
                    s.clear().unwrap();
                    expected.clear();
                }
                1..=8 => {
                    let present = expected.contains(&value);
                    assert_eq!(s.remove(&value).unwrap(), present);
                    expected.retain(|v| *v != value);
                }
                _ => {
                    let absent = !expected.contains(&value);
                    assert_eq!(s.insert(value).unwrap(), absent);
                    if absent {
                        expected.push(value);
                    }
                }
            }
            assert_eq!(values(&s), expected, "hint {hint}, step {step}");
            assert_eq!(
                reverse.iter().map(|v| *v).collect::<Vec<_>>(),
                expected.iter().rev().copied().collect::<Vec<_>>()
            );
            for value in 0..50 {
                assert_eq!(s.contains(&value), expected.contains(&value));
            }
            assert_eq!(s.first().as_deref(), expected.first());
            assert_eq!(s.last().as_deref(), expected.last());
            let mut iterator = s.iter();
            let mut left = 0;
            let mut right = expected.len();
            while left < right {
                assert_eq!(iterator.len(), right - left);
                if (left + right) % 2 == 0 {
                    right -= 1;
                    assert_eq!(*iterator.next_back().unwrap(), expected[right]);
                } else {
                    assert_eq!(*iterator.next().unwrap(), expected[left]);
                    left += 1;
                }
            }
            assert!(iterator.next().is_none());
            assert!(iterator.next_back().is_none());
        }
    }
}
#[derive(Debug)]
struct Payload {
    key: u32,
    label: &'static str,
}
impl PartialEq for Payload {
    fn eq(&self, b: &Self) -> bool {
        self.key == b.key
    }
}
impl Eq for Payload {}
impl Hash for Payload {
    fn hash<H: Hasher>(&self, h: &mut H) {
        0u8.hash(h);
    }
}
#[test]
fn collisions_nonclone_payloads_and_independent_clones_preserve_first_identity() {
    let s = ScalingSequencedSet::new();
    let first = Arc::new(Payload {
        key: 0,
        label: "original",
    });
    s.insert_shared(first.clone()).unwrap();
    for key in 1..40 {
        s.insert(Payload {
            key,
            label: "other",
        })
        .unwrap();
    }
    assert_eq!(s.storage(), SequencedSetStorage::Indexed);
    assert!(
        !s.insert(Payload {
            key: 0,
            label: "duplicate"
        })
        .unwrap()
    );
    assert!(Arc::ptr_eq(&s.first().unwrap(), &first));
    let copy = s.clone();
    for key in 1..40 {
        s.remove(&Payload {
            key,
            label: "probe",
        })
        .unwrap();
    }
    assert_eq!(s.storage(), SequencedSetStorage::Single);
    assert_eq!(s.first().unwrap().label, "original");
    assert_eq!(copy.len(), 40);
    assert!(Arc::ptr_eq(&copy.first().unwrap(), &first));
    s.remove(&Payload {
        key: 0,
        label: "probe",
    })
    .unwrap();
    assert_eq!(s.storage(), SequencedSetStorage::Empty);
    s.insert(Payload {
        key: 0,
        label: "new",
    })
    .unwrap();
    assert_eq!(s.first().unwrap().label, "new");
    assert_eq!(copy.first().unwrap().label, "original");
}
#[test]
fn iterator_leases_reject_edits_but_reverse_views_and_snapshots_do_not() {
    let s = ScalingSequencedSet::new();
    let r = s.reversed();
    s.extend(0..=16).unwrap();
    assert_eq!(s.storage(), SequencedSetStorage::Indexed);
    let saved = s.snapshot();
    let mut it = r.iter();
    assert_eq!(*it.next().unwrap(), 16);
    assert_eq!(s.insert(90), Err(SequencedSetBorrowed));
    assert_eq!(s.remove(&1), Err(SequencedSetBorrowed));
    assert_eq!(s.clear(), Err(SequencedSetBorrowed));
    drop(it);
    s.remove(&16).unwrap();
    assert_eq!(s.storage(), SequencedSetStorage::Indexed);
    s.remove(&0).unwrap();
    assert_eq!(s.storage(), SequencedSetStorage::Array);
    s.insert(90).unwrap();
    assert_eq!(*r.first().unwrap(), 90);
    assert_eq!(
        saved.iter().map(|v| **v).collect::<Vec<_>>(),
        (0..=16).collect::<Vec<_>>()
    );
    assert_eq!(
        ScalingSequencedSet::<u32>::with_capacity(0).storage(),
        SequencedSetStorage::Empty
    );
    s.clear().unwrap();
    assert!(r.is_empty());
}
#[test]
fn collector_orders_current_occurrences_across_key_moves_and_replacements() {
    let mut node = CollectNode::<usize, u8, (usize, u32), OrderedSetCollector<usize, u32>>::new();
    let mut rows = BTreeMap::new();
    let mut seed = 68712;
    for step in 0..3000 {
        let id = (draw(&mut seed) % 60) as usize;
        let new = (!draw(&mut seed).is_multiple_of(5)).then(|| {
            (
                (draw(&mut seed) % 3) as u8,
                ((id * 7) % 61, draw(&mut seed) % 30),
            )
        });
        if let Some(v) = new {
            rows.insert(id, v);
        } else {
            rows.remove(&id);
        }
        node.apply(TupleChange { id, value: new });
        let mut expected = BTreeMap::new();
        for group in 0..3 {
            let mut occurrences = rows
                .values()
                .filter(|(g, _)| *g == group)
                .map(|(_, pair)| *pair)
                .collect::<Vec<_>>();
            occurrences.sort();
            let mut distinct = Vec::new();
            for (_, v) in occurrences {
                if !distinct.contains(&v) {
                    distinct.push(v);
                }
            }
            if !distinct.is_empty() {
                expected.insert(group, distinct);
            }
        }
        let actual = node
            .groups()
            .map(|(g, vs)| (*g, vs.iter().map(|v| **v).collect::<Vec<_>>()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected, "step {step}");
    }
}
#[test]
fn collector_snapshots_clones_replacements_and_duplicate_ids() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut c = OrderedSetCollector::default();
    for (id, v) in [(20, 3), (10, 1), (5, 3), (30, 2)] {
        c.insert(&(id, v));
    }
    let saved = c.finish();
    let mut branch = c.clone();
    assert!(c.replace(&(10, 1), &(10, 1)));
    assert_eq!(
        c.finish().iter().map(|v| **v).collect::<Vec<_>>(),
        [3, 1, 2]
    );
    assert!(!c.replace(&(99, 1), &(10, 1)));
    assert!(!c.remove(&(99, 3)));
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&(10, 99)))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&(5, 3), &(10, 99)))).is_err());
    assert_eq!(
        c.finish().iter().map(|v| **v).collect::<Vec<_>>(),
        [3, 1, 2]
    );
    c.remove(&(5, 3));
    c.remove(&(20, 3));
    c.insert(&(40, 3));
    branch.remove(&(10, 1));
    assert_eq!(saved.iter().map(|v| **v).collect::<Vec<_>>(), [3, 1, 2]);
    assert_eq!(
        c.finish().iter().map(|v| **v).collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        branch.finish().iter().map(|v| **v).collect::<Vec<_>>(),
        [3, 2]
    );
    assert!(Arc::ptr_eq(&saved[2], &c.finish()[1]));
}
#[test]
fn stable_distinct_order_contributes_to_incremental_graph_scoring_and_rollback() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i32)>();
    let distinct = b
        .collect::<_, _, _, OrderedSetCollector<usize, i32>>(source.port(), |v| *v)
        .unwrap();
    b.score("position weighted", &distinct, |values| {
        SimpleScore::new(
            values
                .iter()
                .enumerate()
                .map(|(i, v)| (i as i64 + 1) * i64::from(**v))
                .sum(),
        )
    })
    .unwrap();
    let mut g = b.build();
    {
        let mut t = g.transaction();
        for (id, v) in [3, 1, 3, 2].into_iter().enumerate() {
            t.apply(&source, TupleChange::put(id, (id, v))).unwrap();
        }
        t.commit().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(11));
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::retract(0)).unwrap();
        t.apply(&source, TupleChange::retract(2)).unwrap();
        t.apply(&source, TupleChange::put(0, (4, 3))).unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(11));
    g.assert_consistent().unwrap();
    {
        let mut t = g.transaction();
        t.apply(&source, TupleChange::retract(0)).unwrap();
        t.apply(&source, TupleChange::retract(2)).unwrap();
        t.apply(&source, TupleChange::put(0, (4, 3))).unwrap();
        t.commit().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(14));
    g.assert_consistent().unwrap();
}
