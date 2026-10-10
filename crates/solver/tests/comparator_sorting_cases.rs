// Adapted from pinned Timefold Community comparator and move sorting tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Rows<T>(Vec<T>);
impl<T: Clone> SelectionSource<(), T> for Rows<T> {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.0.iter().cloned())
    }
}
fn config(direction: SelectionSortOrder) -> SelectionConfig {
    SelectionConfig {
        order: Some(SelectionOrder::Sorted),
        sort_order: Some(direction),
        ..Default::default()
    }
}
fn policy() -> ConfiguredSelectionPolicy {
    ConfiguredSelectionPolicy {
        minimum_scope: SelectionCacheScope::Phase,
        ..Default::default()
    }
}
fn open<T: Clone>(s: &mut ConfiguredSelection<(), T, Rows<T>>) {
    for e in [
        SelectionLifecycleEvent::SolveStarted,
        SelectionLifecycleEvent::PhaseStarted,
        SelectionLifecycleEvent::StepStarted,
    ] {
        s.event(e, &()).unwrap();
    }
}
fn factory_codes(direction: SelectionSortOrder, expected: &[&str]) {
    let mut s = SelectionBuilder::new(config(direction))
        .with_comparator_factory(|_: &()| {
            |a: &&str, b: &&str| a.as_bytes()[0].cmp(&b.as_bytes()[0])
        })
        .build(policy(), |_| Rows(vec!["C", "A", "D", "B"]))
        .unwrap();
    open(&mut s);
    assert_eq!(s.iter(&(), 0).unwrap().collect::<Vec<_>>(), expected);
}
#[test]
fn factory_sort_ascending() {
    factory_codes(SelectionSortOrder::Ascending, &["A", "B", "C", "D"]);
}
#[test]
fn factory_sort_descending() {
    factory_codes(SelectionSortOrder::Descending, &["D", "C", "B", "A"]);
}
#[test]
fn direct_comparator_preserves_duplicates_in_both_sort_directions() {
    for direction in [
        SelectionSortOrder::Ascending,
        SelectionSortOrder::Descending,
    ] {
        let mut s = SelectionBuilder::new(config(direction))
            .with_comparator(|_, a: &i32, b: &i32| a.cmp(b))
            .build(policy(), |_| Rows(vec![3, 4, 3, 5, 1]))
            .unwrap();
        open(&mut s);
        let values = s.iter(&(), 0).unwrap().collect::<Vec<_>>();
        assert!(values.windows(2).all(|v| match direction {
            SelectionSortOrder::Ascending => v[0] <= v[1],
            SelectionSortOrder::Descending => v[0] >= v[1],
        }));
        assert_eq!(values.iter().filter(|&&v| v == 3).count(), 2);
    }
}
// generateConfiguration fixture: ascending comparator factory and comparator.
fn sorting_configurations() -> [bool; 2] {
    [true, false]
}
#[test]
fn move_sorting_applies_comparator_and_factory_with_phase_minimum() {
    for factory in sorting_configurations() {
        let b = SelectionBuilder::new(config(SelectionSortOrder::Ascending));
        let b = if factory {
            b.with_comparator_factory(|_: &()| |a: &&str, b: &&str| a.cmp(b))
        } else {
            b.with_comparator(|_, a: &&str, b: &&str| a.cmp(b))
        };
        let mut s = b
            .build(policy(), |_| {
                Rows(vec!["jan", "feb", "mar", "apr", "may", "jun"])
            })
            .unwrap();
        open(&mut s);
        let mut iter = s.iter(&(), 0).unwrap();
        for code in ["apr", "feb", "jan", "jun", "mar", "may"] {
            assert_eq!(iter.next(), Some(code));
        }
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        drop(iter);
        assert!(!s.is_never_ending());
        assert_eq!(s.size(&()).unwrap(), Some(6));
    }
}
