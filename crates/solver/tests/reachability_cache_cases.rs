// Adapted from pinned Community IterableFromEntityPropertyValueSelectorTest and
// FilteringValueRangeSelectorTest (Apache-2.0), with native domains/lifecycle events.
use SelectionLifecycleEvent::*;
use timetuck::*;
fn assert_values(view: WeightedSelection<&'static str>, expected: &[&str]) {
    let mut iter = view.items().iter().copied();
    assert_eq!(iter.by_ref().collect::<Vec<_>>(), expected);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
fn iterable(scope: SelectionCacheScope) {
    let p = FiniteProblem::new(vec![
        Variable::new(vec!["jan", "feb", "mar"]),
        Variable::new(vec!["apr", "may", "jun"]),
    ])
    .unwrap();
    let mut c = ReachabilityCache::new(BasicRangeSource, scope, 2).unwrap();
    c.event(SolveStarted, &p).unwrap();
    for steps in [2, 3] {
        c.event(PhaseStarted, &p).unwrap();
        for _ in 0..steps {
            c.event(StepStarted, &p).unwrap();
            assert_eq!(c.source_size().unwrap(), 6);
            assert_values(
                c.select(ReachabilityQuery::All).unwrap(),
                &["apr", "feb", "jan", "jun", "mar", "may"],
            );
            assert_values(
                c.select(ReachabilityQuery::Entity(&0)).unwrap(),
                &["feb", "jan", "mar"],
            );
            assert_values(
                c.select(ReachabilityQuery::Entity(&1)).unwrap(),
                &["apr", "jun", "may"],
            );
            c.event(StepEnded, &p).unwrap();
        }
        c.event(PhaseEnded, &p).unwrap();
    }
    c.event(SolveEnded, &p).unwrap();
}
fn filtering(scope: SelectionCacheScope) {
    let p = ListProblem::new(
        vec!["jan", "feb", "mar", "apr", "may", "jun"],
        vec![
            ListEntity::new(vec![]).with_value_range(vec!["jan", "feb", "mar"]),
            ListEntity::new(vec![]).with_value_range(vec!["apr", "may", "jun"]),
        ],
    )
    .unwrap();
    let mut c = ReachabilityCache::new(ListRangeSource, scope, 2).unwrap();
    c.event(SolveStarted, &p).unwrap();
    for steps in [2, 3] {
        c.event(PhaseStarted, &p).unwrap();
        for _ in 0..steps {
            c.event(StepStarted, &p).unwrap();
            for (origin, expected) in [
                ("jan", ["feb", "mar"]),
                ("feb", ["jan", "mar"]),
                ("mar", ["feb", "jan"]),
                ("apr", ["jun", "may"]),
                ("may", ["apr", "jun"]),
                ("jun", ["apr", "may"]),
            ] {
                assert_eq!(c.source_size().unwrap(), 6);
                assert_values(
                    c.select(ReachabilityQuery::CoValues(&origin)).unwrap(),
                    &expected,
                );
            }
            c.event(StepEnded, &p).unwrap();
        }
        c.event(PhaseEnded, &p).unwrap();
    }
    c.event(SolveEnded, &p).unwrap();
}
#[test]
fn iterable_solve() {
    iterable(SelectionCacheScope::Solve)
}
#[test]
fn iterable_phase() {
    iterable(SelectionCacheScope::Phase)
}
#[test]
fn iterable_step() {
    iterable(SelectionCacheScope::Step)
}
#[test]
fn filtering_solve() {
    filtering(SelectionCacheScope::Solve)
}
#[test]
fn filtering_phase() {
    filtering(SelectionCacheScope::Phase)
}
#[test]
fn filtering_step() {
    filtering(SelectionCacheScope::Step)
}
