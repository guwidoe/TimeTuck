//! Original named-pipeline integration and selection-budget cases.
use SelectionCacheScope::{Phase, Uncached};
use SelectionLifecycleEvent::*;
use SelectionOrder::*;
use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
struct Values {
    rows: Vec<i32>,
    opens: Rc<Cell<usize>>,
    repeating: bool,
}
impl SelectionSource<(), i32> for Values {
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.rows.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = i32> + 'a>
    where
        i32: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        if self.repeating {
            Box::new(self.rows.iter().copied().cycle())
        } else {
            Box::new(self.rows.iter().copied())
        }
    }
}
fn cfg(order: SelectionOrder, limit: Option<usize>) -> SelectionConfig {
    SelectionConfig {
        cache_scope: Some(Phase),
        order: Some(order),
        selected_count_limit: limit,
        ..Default::default()
    }
}
#[test]
fn budgets_reset_per_open_and_zero_does_not_open_a_direct_source() {
    for order in [Original, Random, Shuffled, Sorted, Probabilistic] {
        for limit in [0, 1, 5] {
            let opens = Rc::new(Cell::new(0));
            let b = SelectionBuilder::new(cfg(order, Some(limit)));
            let b = match order {
                Sorted => b.with_comparator(|_: &(), a: &i32, b: &i32| a.cmp(b)),
                Probabilistic => b.with_weight(|_, v| f64::from(*v == 2)),
                _ => b,
            };
            let mut s = b
                .build(ConfiguredSelectionPolicy::default(), |_| Values {
                    rows: vec![1, 2, 3],
                    opens: opens.clone(),
                    repeating: false,
                })
                .unwrap();
            for e in [SolveStarted, PhaseStarted, StepStarted] {
                s.event(e, &()).unwrap();
            }
            let expected = if matches!(order, Random | Probabilistic) {
                limit
            } else {
                limit.min(3)
            };
            assert!(!s.is_never_ending());
            assert_eq!(s.size(&()).unwrap(), Some(expected));
            for _ in 0..2 {
                let rows: Vec<_> = s.iter(&(), 1).unwrap().collect();
                assert_eq!(rows.len(), expected);
                if order == Probabilistic {
                    assert!(rows.iter().all(|v| *v == 2));
                }
            }
            assert_eq!(s.ending_iter(&()).unwrap().count(), 3);
            assert_eq!(opens.get(), 1);
        }
    }
    let opens = Rc::new(Cell::new(0));
    let mut c = cfg(Random, Some(0));
    c.cache_scope = Some(Uncached);
    let mut s = SelectionBuilder::new(c)
        .build(ConfiguredSelectionPolicy::default(), |_| Values {
            rows: vec![1, 2],
            opens: opens.clone(),
            repeating: true,
        })
        .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
    assert_eq!(s.size(&()).unwrap(), Some(0));
    assert_eq!(s.iter(&(), 0).unwrap().count(), 0);
    assert_eq!(opens.get(), 0);
    assert!(matches!(
        s.ending_iter(&()),
        Err(SelectionCacheError::UnboundedSource)
    ));
}
#[test]
fn named_views_share_publications_and_only_the_owner_drives_lifecycle() {
    let mut registry = SelectionRegistry::new();
    let opens = Rc::new(Cell::new(0));
    let mut recorder = SelectionBuilder::new(cfg(Sorted, Some(2)))
        .with_comparator(|_: &(), a: &i32, b: &i32| b.cmp(a))
        .build_recording(
            ConfiguredSelectionPolicy::default(),
            |_| Values {
                rows: vec![1, 3, 2],
                opens: opens.clone(),
                repeating: false,
            },
            "entities",
            &mut registry,
        )
        .unwrap();
    let a = SelectionBuilder::<(), i32>::new(Default::default())
        .build_replaying("entities", &registry)
        .unwrap();
    let b = registry.replay("entities").unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        recorder.event(e, &());
    }
    {
        let mut recording = recorder.open(&());
        let mut x = a.cursor();
        let mut y = b.cursor();
        assert!(recording.has_next());
        assert!(matches!(
            x.next_replayed(),
            Err(SelectionReplayError::NotRecorded)
        ));
        let first = recording.next().unwrap();
        assert_eq!(*first, 3);
        assert!(Arc::ptr_eq(&first, &x.next_replayed().unwrap().unwrap()));
        assert!(Arc::ptr_eq(&first, &y.next_replayed().unwrap().unwrap()));
        assert_eq!(x.next_replayed().unwrap(), None);
        let second = recording.next().unwrap();
        assert_eq!(*second, 2);
        assert!(Arc::ptr_eq(&second, &x.next_replayed().unwrap().unwrap()));
        assert_eq!(recording.next(), None);
        assert_eq!(y.next_replayed().unwrap(), None);
    }
    assert_eq!(opens.get(), 1);
    let mut old = a.cursor();
    recorder.event(StepEnded, &());
    assert!(matches!(
        old.next_replayed(),
        Err(SelectionReplayError::StaleCursor)
    ));
    recorder.event(PhaseEnded, &());
    recorder.event(SolveEnded, &());
    assert_eq!(recorder.source().state(), SelectionCacheState::Idle);
    assert!(registry.remove("entities").is_some());
    assert!(matches!(
        registry.replay("entities"),
        Err(SelectionRegistrationError::UnknownId(_))
    ));
    // Removal only unbinds the name. Existing views still follow their original owner.
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        recorder.event(e, &());
    }
    let mut recording = recorder.open(&());
    let mut old_view = a.cursor();
    let next = recording.next().unwrap();
    assert!(Arc::ptr_eq(
        &next,
        &old_view.next_replayed().unwrap().unwrap()
    ));
}
#[test]
fn invalid_names_overrides_and_configuration_never_replace_an_existing_recorder() {
    let mut registry = SelectionRegistry::new();
    let _owner = registry
        .register(
            "x",
            Values {
                rows: vec![7],
                opens: Rc::new(Cell::new(0)),
                repeating: false,
            },
        )
        .unwrap();
    for id in ["x", ""] {
        let result = SelectionBuilder::<(), i32>::new(Default::default()).build_recording(
            ConfiguredSelectionPolicy::default(),
            |_| -> Values { panic!("must validate name before factory") },
            id,
            &mut registry,
        );
        assert!(matches!(
            result,
            Err(SelectionRegistrationError::DuplicateId(_) | SelectionRegistrationError::EmptyId)
        ));
    }
    let b = SelectionBuilder::<(), i32>::new(cfg(Sorted, None));
    assert!(matches!(
        b.build_recording(
            ConfiguredSelectionPolicy::default(),
            |_| -> Values { panic!("invalid sorting must not create source") },
            "new",
            &mut registry
        ),
        Err(SelectionRegistrationError::Build(
            SelectionBuildError::MissingComparator
        ))
    ));
    assert_eq!(registry.len(), 1);
    assert!(registry.replay("x").is_ok());
    assert!(registry.replay("new").is_err());
    assert!(matches!(
        SelectionBuilder::<(), i32>::new(Default::default())
            .with_weight(|_, _| 1.0)
            .build_replaying("x", &registry),
        Err(SelectionRegistrationError::ReplayOverrides)
    ));
}
#[test]
fn sublist_source_integrates_limits_recording_and_current_pin_aware_context() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![
            ListEntity::new(vec![1, 2, 3, 4]).with_pinned_prefix(1),
            ListEntity::new(vec![5]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut registry = SelectionRegistry::new();
    let mut config = cfg(Random, Some(20));
    config.cache_scope = Some(Uncached);
    let mut recorder = SelectionBuilder::new(config)
        .build_recording(
            ConfiguredSelectionPolicy::default(),
            |r| SubListSource::configured(r, SubListSize::new(2, 3).unwrap(), 17),
            "spans",
            &mut registry,
        )
        .unwrap();
    let replay = registry.replay("spans").unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        recorder.event(e, &c);
    }
    let mut stream = recorder.open(&c);
    let mut view = replay.cursor();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..20 {
        let span = stream.next().unwrap();
        assert_eq!(span.entity, 0);
        assert!(span.start >= 1 && span.end <= 4);
        assert!((2..=3).contains(&(span.end - span.start)));
        assert!(Arc::ptr_eq(&span, &view.next_replayed().unwrap().unwrap()));
        seen.insert((span.start, span.end));
    }
    assert_eq!(stream.next(), None);
    assert_eq!(seen.len(), 3);
}

#[test]
fn empty_weighted_and_unknown_repeating_sources_do_not_invent_available_draws() {
    let mut s = SelectionBuilder::new(cfg(Probabilistic, Some(5)))
        .with_weight(|_: &(), _: &i32| 0.0)
        .build(ConfiguredSelectionPolicy::default(), |_| Values {
            rows: vec![1, 2],
            opens: Rc::new(Cell::new(0)),
            repeating: false,
        })
        .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
    assert_eq!(s.size(&()).unwrap(), Some(0));
    assert_eq!(s.iter(&(), 0).unwrap().count(), 0);
    assert_eq!(s.ending_iter(&()).unwrap().collect::<Vec<_>>(), [1, 2]);
    struct Unknown;
    impl SelectionSource<(), i32> for Unknown {
        fn is_never_ending(&self) -> bool {
            true
        }
        fn size(&mut self, _: &()) -> Option<usize> {
            None
        }
        fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = i32> + 'a>
        where
            i32: 'a,
        {
            Box::new(std::iter::empty())
        }
    }
    let mut config = cfg(Random, Some(5));
    config.cache_scope = Some(Uncached);
    let mut s = SelectionBuilder::new(config)
        .build(ConfiguredSelectionPolicy::default(), |_| Unknown)
        .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        s.event(e, &()).unwrap();
    }
    assert_eq!(s.size(&()).unwrap(), None);
    assert_eq!(s.iter(&(), 0).unwrap().count(), 0);
}
