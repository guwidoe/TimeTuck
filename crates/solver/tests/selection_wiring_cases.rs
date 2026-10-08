// Adapted from Timefold Community selector factory/limit tests, Apache-2.0.
// See compat/provenance.json and notices. Native source types replace reflection.
use SelectionCacheScope::Phase;
use SelectionLifecycleEvent::*;
use SelectionOrder::*;
use timetuck::*;
fn replay_override() {
    let registry = SelectionRegistry::<i32>::new();
    let builder = SelectionBuilder::<(), i32>::new(SelectionConfig {
        selected_count_limit: Some(10),
        ..Default::default()
    });
    assert!(matches!(
        builder.build_replaying("someSelectorId", &registry),
        Err(SelectionRegistrationError::ReplayOverrides)
    ));
}
#[test]
fn entity_replay_rejects_other_properties() {
    replay_override();
}
#[test]
fn value_replay_rejects_other_properties() {
    replay_override();
}
struct SharedRange(Vec<Option<i32>>);
impl SelectionSource<BasicValueContext<'_, i32>, Option<i32>> for SharedRange {
    fn size(&mut self, _: &BasicValueContext<'_, i32>) -> Option<usize> {
        Some(self.0.len())
    }
    fn open<'a>(
        &'a mut self,
        _: &'a BasicValueContext<'_, i32>,
    ) -> Box<dyn Iterator<Item = Option<i32>> + 'a>
    where
        Option<i32>: 'a,
    {
        Box::new(self.0.iter().copied())
    }
}
fn sorted_range<C, S: SelectionSource<C, Option<i32>>>(
    source: S,
    context: &C,
    factory: bool,
    expected: &[Option<i32>],
) {
    let b = SelectionBuilder::new(SelectionConfig {
        cache_scope: Some(Phase),
        order: Some(Sorted),
        ..Default::default()
    });
    let b = if factory {
        b.with_comparator_factory(|_: &C| |a: &Option<i32>, b: &Option<i32>| a.cmp(b))
    } else {
        b.with_comparator(|_: &C, a: &Option<i32>, b: &Option<i32>| a.cmp(b))
    };
    let mut s = b
        .build(ConfiguredSelectionPolicy::default(), |_| source)
        .unwrap();
    assert_eq!(s.plan().cache_scope, Phase);
    assert_eq!(s.plan().order, Sorted);
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, context).unwrap();
    }
    assert_eq!(s.iter(context, 0).unwrap().collect::<Vec<_>>(), expected);
    for e in [StepEnded, PhaseEnded, SolveEnded] {
        s.event(e, context).unwrap();
    }
}
fn value_sorting(factory: bool) {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![3, 1]).with_initial(3),
        Variable::new(vec![4, 2]).with_initial(4),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let c = BasicSelectionContext::new(&p, &a).unwrap();
    // Solution-provided and entity-provided branches both retain phase sorting.
    sorted_range(
        SharedRange(vec![Some(4), Some(1), Some(3), Some(2)]),
        &c.variable(0).unwrap(),
        factory,
        &[Some(1), Some(2), Some(3), Some(4)],
    );
    for (entity, expected) in [(0, [Some(1), Some(3)]), (1, [Some(2), Some(4)])] {
        sorted_range(
            BasicValueSource,
            &c.variable(entity).unwrap(),
            factory,
            &expected,
        );
    }
}
#[test]
fn value_sorting_comparator() {
    value_sorting(false);
}
#[test]
fn value_sorting_factory() {
    value_sorting(true);
}
#[test]
fn optional_list_filter_configuration_all_six_combinations() {
    for optional in [true, false] {
        let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1])]).unwrap();
        let p = if optional { p.allowing_unassigned() } else { p };
        let a = p.initial_assignment();
        let c = ListSelectionContext::new(&p, &a).unwrap();
        for filter in [
            OptionalListValueFilter::Assigned,
            OptionalListValueFilter::Unassigned,
            OptionalListValueFilter::All,
        ] {
            let mut source = ListValueSource::configured(&p, filter);
            let (expected_mode, expected) = if !optional {
                (ListValueSelection::All, vec![1, 2])
            } else {
                match filter {
                    OptionalListValueFilter::Assigned => (ListValueSelection::Assigned, vec![1]),
                    OptionalListValueFilter::Unassigned => {
                        (ListValueSelection::Unassigned, vec![2])
                    }
                    OptionalListValueFilter::All => (ListValueSelection::All, vec![1, 2]),
                }
            };
            assert_eq!(source.0, expected_mode);
            assert_eq!(source.open(&c).collect::<Vec<_>>(), expected);
        }
    }
}
struct Entities;
impl SelectionSource<(), &'static str> for Entities {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(4)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        Box::new(["e1", "e2", "e3", "e4"].into_iter())
    }
}
#[test]
fn ending_iterator_ignores_random_selection_limit() {
    let mut s = SelectionBuilder::new(SelectionConfig {
        cache_scope: Some(Phase),
        order: Some(Random),
        selected_count_limit: Some(2),
        ..Default::default()
    })
    .build(ConfiguredSelectionPolicy::default(), |_| Entities)
    .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
    assert_eq!(s.iter(&(), 0).unwrap().count(), 2);
    assert_eq!(
        s.ending_iter(&()).unwrap().collect::<Vec<_>>(),
        ["e1", "e2", "e3", "e4"]
    );
}
#[test]
fn build_sublist_selector_preserves_bounds() {
    let bounds = SubListSize::new(2, 3).unwrap();
    let s = SelectionBuilder::<ListSelectionContext<'_, i32>, ListRange>::new(
        SelectionConfig::default(),
    )
    .build(ConfiguredSelectionPolicy::default(), |request| {
        SubListSource::configured(request, bounds, 0)
    })
    .unwrap();
    assert_eq!(s.source().bounds().minimum(), 2);
    assert_eq!(s.source().bounds().maximum(), 3);
}
type SpanRecorder = RecordingSelection<
    ListSelectionContext<'static, i32>,
    ListRange,
    ConfiguredSelection<ListSelectionContext<'static, i32>, ListRange, SubListSource>,
>;
fn registered_spans() -> (SelectionRegistry<ListRange>, SpanRecorder) {
    let mut registry = SelectionRegistry::new();
    let recorder = SelectionBuilder::new(SelectionConfig::default())
        .build_recording(
            ConfiguredSelectionPolicy::default(),
            |request| SubListSource::configured(request, SubListSize::new(3, 10).unwrap(), 0),
            "someSelectorId",
            &mut registry,
        )
        .unwrap();
    (registry, recorder)
}
#[test]
fn build_named_sublist_recording() {
    let (registry, recorder) = registered_spans();
    assert_eq!(registry.len(), 1);
    assert_eq!(
        recorder.source().source().bounds(),
        SubListSize::new(3, 10).unwrap()
    );
    assert!(registry.replay("someSelectorId").is_ok());
}
#[test]
fn build_named_sublist_replaying() {
    let (registry, _recorder) = registered_spans();
    let replay = SelectionBuilder::<(), ListRange>::new(SelectionConfig::default())
        .build_replaying("someSelectorId", &registry)
        .unwrap();
    assert!(matches!(
        replay.cursor().next_replayed(),
        Err(SelectionReplayError::NotRecorded)
    ));
}
