// Adapted from IterableFromSolutionPropertyValueSelectorTest and test helpers.
// Timefold Community, Apache-2.0; see compat/provenance.json and notices.
// Typed context values replace the reflected solution property.
use SelectionLifecycleEvent as E;
use timetuck::*;
struct PropertyValues;
impl SelectionSource<Vec<&'static str>, &'static str> for PropertyValues {
    fn size(&mut self, context: &Vec<&'static str>) -> Option<usize> {
        Some(context.len())
    }
    fn open<'a>(
        &'a mut self,
        context: &'a Vec<&'static str>,
    ) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(context.iter().copied())
    }
}
fn exercise(scope: SelectionCacheScope) {
    let context = vec!["jan", "feb", "mar", "apr", "may", "jun"];
    let mut selector = OrderedSelectionCache::sorted_by(
        PropertyValues,
        scope,
        6,
        |_: &Vec<&'static str>, a: &&'static str, b: &&'static str| a.cmp(b),
    )
    .unwrap();
    selector.event(E::SolveStarted, &context).unwrap();
    for steps in [2, 3] {
        selector.event(E::PhaseStarted, &context).unwrap();
        for _ in 0..steps {
            selector.event(E::StepStarted, &context).unwrap();
            let snapshot = selector.snapshot().unwrap();
            let mut iter = snapshot.iter();
            for expected in ["apr", "feb", "jan", "jun", "mar", "may"] {
                assert_eq!(iter.next(), Some(&expected));
            }
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
            assert!(!selector.is_never_ending());
            assert_eq!(selector.size().unwrap(), 6);
            selector.event(E::StepEnded, &context).unwrap();
        }
        selector.event(E::PhaseEnded, &context).unwrap();
    }
    selector.event(E::SolveEnded, &context).unwrap();
}
#[test]
fn original_selection_cache_type_solver() {
    exercise(SelectionCacheScope::Solve);
}
#[test]
fn original_selection_cache_type_phase() {
    exercise(SelectionCacheScope::Phase);
}
#[test]
fn original_selection_cache_type_step() {
    exercise(SelectionCacheScope::Step);
}
