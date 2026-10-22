//! Original independent uniform-storage controls; no upstream-method credit.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};
fn pos(i: usize) -> Position {
    Position {
        page: PageKey([usize::MAX, i, usize::MAX - i, i / 16]),
        cell: i % 16,
    }
}
fn check(a: &Arena, model: &[Option<IntCollector>]) {
    for (i, value) in model.iter().enumerate() {
        if let Some(value) = value {
            assert_eq!(a.get(i), value);
            assert_eq!(a.count(i), value.occurrence_count());
            assert_eq!(a.position(i), pos(i));
        }
    }
}
#[test]
fn uniform_arena_keeps_arbitrary_values_counts_immutable_clones_promotions_and_reuse() {
    for value in [0, -7, i64::MIN, i64::MAX] {
        let mut a = Arena::default();
        let mut model = Vec::new();
        for i in 0..96 {
            let c: IntCollector = std::iter::repeat_n(value, i % 5 + 1).collect();
            assert_eq!(a.insert(pos(i), c.clone()), i);
            model.push(Some(c));
        }
        assert!(matches!(a.storage, Storage::Uniform(_)));
        let saved = a.clone();
        let old = model.clone();
        for round in 0..12 {
            for (i, slot) in model.iter_mut().enumerate() {
                let edit = if (i + round) % 2 == 0 {
                    ScalarEdit::Insert(value)
                } else {
                    ScalarEdit::Remove(value)
                };
                if matches!(edit, ScalarEdit::Remove(_))
                    && slot.as_ref().unwrap().occurrence_count() == 0
                {
                    continue;
                }
                let output = a.try_uniform_update(i, edit, "input occurrence").unwrap();
                match edit {
                    ScalarEdit::Insert(v) => slot.as_mut().unwrap().insert(v),
                    ScalarEdit::Remove(v) => assert!(slot.as_mut().unwrap().remove(v)),
                    _ => unreachable!(),
                }
                assert_eq!(
                    IntCollector::from_single_state(output.0, output.1),
                    *slot.as_ref().unwrap()
                );
            }
            check(&a, &model);
            assert!(matches!(a.storage, Storage::Uniform(_)));
        }
        check(&saved, &old);
        let pointer = a.get(1) as *const IntCollector;
        let before = a.get(1).clone();
        let foreign = value.wrapping_add(1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| a.try_uniform_update(
                1,
                ScalarEdit::Remove(foreign),
                "input occurrence"
            )))
            .is_err()
        );
        assert_eq!(a.get(1), &before);
        assert_eq!(a.get(1) as *const _, pointer);
        assert!(
            a.try_uniform_update(1, ScalarEdit::Insert(foreign), "input occurrence")
                .is_none()
        );
        assert_eq!(a.get(1) as *const _, pointer);
        a.get_mut(1).insert(foreign);
        model[1].as_mut().unwrap().insert(foreign);
        assert!(matches!(a.storage, Storage::General(_)));
        check(&a, &model);
        check(&saved, &old);
        let mut live = saved.clone_live([0, 3, 95].into_iter());
        for i in [0, 3, 95] {
            assert_eq!(live.get(i), old[i].as_ref().unwrap());
        }
        live.retire(3);
        let c = IntCollector::from_single_state(value, 3);
        assert_eq!(live.insert(pos(3), c.clone()), 3);
        assert_eq!(live.get(3), &c);
        assert!(matches!(live.storage, Storage::Uniform(_)));
    }
    if usize::BITS == 64 {
        assert_eq!(std::mem::size_of::<Counter>(), 24);
        assert_eq!(std::mem::size_of::<Option<Counter>>(), 24);
        assert_eq!(std::mem::size_of::<Value>(), 64);
    }
}
#[test]
fn uniform_arena_count_overflow_preserves_views_and_never_limits_the_general_domain() {
    for value in [0, -7, i64::MIN, i64::MAX] {
        let mut a = Arena::default();
        let original = IntCollector::from_single_state(value, usize::MAX);
        let slot = a.insert(pos(0), original.clone());
        let pointer = a.get(slot) as *const IntCollector;
        assert!(
            catch_unwind(AssertUnwindSafe(|| a.try_uniform_update(
                slot,
                ScalarEdit::Insert(value),
                "input occurrence"
            )))
            .is_err()
        );
        assert_eq!(a.get(slot), &original);
        assert_eq!(a.get(slot) as *const _, pointer);
        let (v, n) = a
            .try_uniform_update(slot, ScalarEdit::Remove(value), "input occurrence")
            .unwrap();
        assert_eq!(n, usize::MAX - 1);
        assert_eq!(v, value);
        a.try_uniform_update(slot, ScalarEdit::Insert(value), "inverse group occurrence")
            .unwrap();
        assert_eq!(a.get(slot), &original);
        let foreign = value.wrapping_add(1);
        assert!(
            a.try_uniform_update(
                slot,
                ScalarEdit::Replace(value, foreign),
                "input occurrence"
            )
            .is_none()
        );
        assert!(a.get_mut(slot).replace_captured(value, foreign));
        assert_eq!(a.get(slot).summary().count, usize::MAX);
        assert_eq!(a.get(slot).summary().distinct_count, 2);
        assert!(a.get_mut(slot).replace_captured(foreign, value));
        assert_eq!(a.get(slot), &original);
    }
}
