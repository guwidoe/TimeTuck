// Adapted from Timefold Community EntitySelectorFactoryTest, ValueSelectorFactoryTest
// and MoveSelectorFactoryTest (Apache-2.0); see compat/provenance.json and notices.
// Native plans and retained source identities replace Java wrapper/reflection types.
use SelectionCacheScope::{Phase, Step, Uncached};
use SelectionLifecycleEvent::*;
use SelectionOrder::*;
use std::rc::Rc;
use timetuck::*;
#[derive(Clone)]
struct Source {
    identity: Rc<()>,
    random: bool,
}
impl SelectionSource<(), i32> for Source {
    fn is_never_ending(&self) -> bool {
        self.random
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(3)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        if self.random {
            Box::new([3, 1, 2].into_iter().cycle())
        } else {
            Box::new([3, 1, 2].into_iter())
        }
    }
}
fn policy(minimum: SelectionCacheScope) -> SelectionPolicy {
    SelectionPolicy {
        source_minimum_scope: minimum,
        ..SelectionPolicy::default()
    }
}
fn config(scope: SelectionCacheScope, order: SelectionOrder) -> SelectionConfig {
    SelectionConfig {
        cache_scope: Some(scope),
        order: Some(order),
        ..SelectionConfig::default()
    }
}
fn start<S: SelectionSource<(), i32>>(s: &mut ConfiguredSelection<(), i32, S>) {
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
}
fn finish<S: SelectionSource<(), i32>>(s: &mut ConfiguredSelection<(), i32, S>) {
    for e in [StepEnded, PhaseEnded, SolveEnded] {
        s.event(e, &()).unwrap();
    }
}
fn configuration(
    scope: SelectionCacheScope,
    order: SelectionOrder,
    source_min: SelectionCacheScope,
    expected: SelectionCacheScope,
    expected_source: SelectionCacheScope,
) {
    let identity = Rc::new(());
    let b = SelectionBuilder::<(), i32>::new(config(scope, order));
    if scope == Uncached && order == Shuffled {
        let result = b.build(policy(source_min), |_| -> Source {
            panic!("invalid configuration must not call factory")
        });
        assert!(matches!(
            result,
            Err(SelectionBuildError::RequiresCache(Shuffled))
        ));
        return;
    }
    let mut s = b
        .build(policy(source_min), |request| {
            assert_eq!(request.cache_scope, expected_source);
            assert_eq!(request.random, expected == Uncached && order == Random);
            Source {
                identity: identity.clone(),
                random: request.random,
            }
        })
        .unwrap();
    assert_eq!(s.plan().cache_scope, expected);
    assert_eq!(s.plan().order, order);
    assert!(Rc::ptr_eq(&s.source().identity, &identity));
    start(&mut s);
    assert_eq!(s.is_never_ending(), order == Random);
    let rows: Vec<_> = s.iter(&(), 19).unwrap().take(9).collect();
    if order == Original {
        assert_eq!(rows, [3, 1, 2]);
    } else if order == Shuffled {
        let mut sorted = rows.clone();
        sorted.sort();
        assert_eq!(sorted, [1, 2, 3]);
        assert_eq!(s.iter(&(), 77).unwrap().collect::<Vec<_>>(), rows);
    } else {
        assert_eq!(rows.len(), 9);
        assert!(rows.iter().all(|x| [1, 2, 3].contains(x)));
    }
    finish(&mut s);
    assert_eq!(s.state(), SelectionCacheState::Idle);
}
fn sorting(factory: bool) {
    let b = SelectionBuilder::<(), i32>::new(config(Phase, Sorted));
    let b = if factory {
        b.with_comparator_factory(|_| |a: &i32, b: &i32| a.cmp(b))
    } else {
        b.with_comparator(|_, a, b| a.cmp(b))
    };
    let mut s = b
        .build(policy(Uncached), |_| Source {
            identity: Rc::new(()),
            random: false,
        })
        .unwrap();
    assert_eq!(s.plan().order, Sorted);
    start(&mut s);
    assert_eq!(s.iter(&(), 0).unwrap().collect::<Vec<_>>(), [1, 2, 3]);
    finish(&mut s);
}
fn probability(inherited: bool) {
    let mut c = config(Phase, Probabilistic);
    let mut p = policy(Uncached);
    if inherited {
        c.order = None;
        p.inherited_order = Probabilistic;
    }
    let mut s = SelectionBuilder::<(), i32>::new(c)
        .with_weight(|_, _| 0.0)
        .build(p, |_| Source {
            identity: Rc::new(()),
            random: false,
        })
        .unwrap();
    assert_eq!(s.plan().order, Probabilistic);
    start(&mut s);
    assert!(s.is_never_ending());
    assert_eq!(s.iter(&(), 0).unwrap().next(), None);
    finish(&mut s);
}

mod entity {
    use super::*;
    #[test]
    fn phase_original() {
        configuration(Phase, Original, Step, Phase, Phase);
    }
    #[test]
    fn phase_random() {
        configuration(Phase, Random, Step, Phase, Phase);
    }
    #[test]
    fn phase_shuffled() {
        configuration(Phase, Shuffled, Step, Phase, Phase);
    }
    #[test]
    fn step_original() {
        configuration(Step, Original, Step, Step, Step);
    }
    #[test]
    fn step_random() {
        configuration(Step, Random, Step, Step, Step);
    }
    #[test]
    fn step_shuffled() {
        configuration(Step, Shuffled, Step, Step, Step);
    }
    #[test]
    fn just_in_time_original() {
        configuration(Uncached, Original, Step, Step, Step);
    }
    #[test]
    fn just_in_time_random() {
        configuration(Uncached, Random, Step, Step, Step);
    }
    #[test]
    fn just_in_time_shuffled() {
        configuration(Uncached, Shuffled, Step, Uncached, Step);
    }
    #[test]
    fn sorting_comparator() {
        sorting(false);
    }
    #[test]
    fn sorting_factory() {
        sorting(true);
    }
    #[test]
    fn weighted_selection() {
        probability(false);
    }
}

mod value {
    use super::*;
    #[test]
    fn phase_original() {
        configuration(Phase, Original, Phase, Phase, Phase);
    }
    #[test]
    fn phase_random() {
        configuration(Phase, Random, Phase, Phase, Phase);
    }
    #[test]
    fn phase_shuffled() {
        configuration(Phase, Shuffled, Phase, Phase, Phase);
    }
    #[test]
    fn step_original() {
        configuration(Step, Original, Phase, Phase, Phase);
    }
    #[test]
    fn step_random() {
        configuration(Step, Random, Phase, Phase, Phase);
    }
    #[test]
    fn step_shuffled() {
        configuration(Step, Shuffled, Phase, Step, Phase);
    }
    #[test]
    fn just_in_time_original() {
        configuration(Uncached, Original, Phase, Phase, Phase);
    }
    #[test]
    fn just_in_time_random() {
        configuration(Uncached, Random, Phase, Phase, Phase);
    }
    #[test]
    fn just_in_time_shuffled() {
        configuration(Uncached, Shuffled, Phase, Uncached, Phase);
    }
    #[test]
    fn weighted_selection() {
        probability(false);
    }
    #[test]
    fn filtering_callback() {
        let mut s = SelectionBuilder::<(), i32>::new(config(Uncached, Original))
            .build_filtered(
                policy(Uncached),
                |_| Source {
                    identity: Rc::new(()),
                    random: false,
                },
                |_, _| true,
            )
            .unwrap();
        start(&mut s);
        assert_eq!(s.iter(&(), 0).unwrap().collect::<Vec<_>>(), [3, 1, 2]);
        finish(&mut s);
    }
}

mod movement {
    use super::*;
    #[test]
    fn phase_original() {
        configuration(Phase, Original, Uncached, Phase, Phase);
    }
    #[test]
    fn phase_random() {
        configuration(Phase, Random, Uncached, Phase, Phase);
    }
    #[test]
    fn phase_shuffled() {
        configuration(Phase, Shuffled, Uncached, Phase, Phase);
    }
    #[test]
    fn step_original() {
        configuration(Step, Original, Uncached, Step, Step);
    }
    #[test]
    fn step_random() {
        configuration(Step, Random, Uncached, Step, Step);
    }
    #[test]
    fn step_shuffled() {
        configuration(Step, Shuffled, Uncached, Step, Step);
    }
    #[test]
    fn just_in_time_original() {
        configuration(Uncached, Original, Uncached, Uncached, Uncached);
    }
    #[test]
    fn just_in_time_random() {
        configuration(Uncached, Random, Uncached, Uncached, Uncached);
    }
    #[test]
    fn just_in_time_shuffled() {
        configuration(Uncached, Shuffled, Uncached, Uncached, Uncached);
    }
    #[test]
    fn sorting_comparator() {
        sorting(false);
    }
    #[test]
    fn sorting_factory() {
        sorting(true);
    }
    #[test]
    fn weighted_selection() {
        probability(true);
    }
    #[test]
    fn incompatible_sorting_order() {
        let mut c = config(Phase, Random);
        c.sort_order = Some(SelectionSortOrder::Ascending);
        assert_eq!(
            SelectionBuilder::<(), i32>::new(c).resolve(policy(Uncached)),
            Err(SelectionBuildError::SortingRequiresSortedOrder)
        );
    }
    #[test]
    fn missing_comparator() {
        let mut c = config(Phase, Sorted);
        c.sort_order = Some(SelectionSortOrder::Ascending);
        assert_eq!(
            SelectionBuilder::<(), i32>::new(c).resolve(policy(Uncached)),
            Err(SelectionBuildError::MissingComparator)
        );
    }
    #[test]
    fn non_movable_filter() {
        let mut s = SelectionBuilder::<(), i32>::new(SelectionConfig::default())
            .build_filtered(
                SelectionPolicy {
                    minimum_scope: Phase,
                    ..Default::default()
                },
                |_| Source {
                    identity: Rc::new(()),
                    random: false,
                },
                |_, _| false,
            )
            .unwrap();
        start(&mut s);
        assert_eq!(s.iter(&(), 0).unwrap().next(), None);
        assert_eq!(s.source().stats().yielded, 0);
        finish(&mut s);
    }
}
