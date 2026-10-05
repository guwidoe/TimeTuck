// Adapted from pinned Community ReinitializeVariableValueSelectorTest and
// UnassignedListValueSelectorTest, with PlannerAssert helpers (Apache-2.0).
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Context {
    values: Vec<Option<&'static str>>,
    target: usize,
}
struct Source {
    values: Vec<&'static str>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
    sizes: usize,
}
impl SelectionSource<Context, &'static str> for Source {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &Context) {
        self.events.push(e)
    }
    fn size(&mut self, _: &Context) -> Option<usize> {
        self.sizes += 1;
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a Context) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        self.opens += 1;
        Box::new(self.values.iter().copied())
    }
}
fn reinitialize(multi: bool) {
    let values = if multi {
        vec!["s1", "s2", "s3"]
    } else {
        vec!["v1", "v2", "v3"]
    };
    let mut source = GatedSelectionSource::new(
        Source {
            values: values.clone(),
            events: vec![],
            opens: 0,
            sizes: 0,
        },
        |c: &Context| c.values[c.target].is_none(),
    );
    let mut c = Context {
        values: vec![None; if multi { 4 } else { 2 }],
        target: if multi { 1 } else { 0 },
    };
    source.event(SolveStarted, &c);
    for phase in 0..2 {
        source.event(PhaseStarted, &c);
        for step in 0..2 {
            source.event(StepStarted, &c);
            let expected = match (phase, step, multi) {
                (0, 0, true) => {
                    c.values[0] = Some("p1");
                    values.clone()
                }
                (0, 0, false) => values.clone(),
                (0, 1, _) => {
                    c.target = if multi { 3 } else { 1 };
                    c.values[c.target] = Some(values[1]);
                    vec![]
                }
                (1, 0, _) => {
                    c.target = if multi { 3 } else { 1 };
                    c.values[c.target] = None;
                    values.clone()
                }
                (1, 1, true) => {
                    c.target = 1;
                    c.values[0] = None;
                    values.clone()
                }
                (1, 1, false) => {
                    c.target = 0;
                    c.values[0] = Some(values[2]);
                    vec![]
                }
                _ => unreachable!(),
            };
            assert_eq!(source.size(&c), Some(expected.len()));
            assert!(!source.is_never_ending());
            let mut cursor = source.open(&c);
            assert_eq!(cursor.by_ref().collect::<Vec<_>>(), expected);
            assert_eq!(cursor.next(), None);
            assert_eq!(cursor.next(), None);
            drop(cursor);
            source.event(StepEnded, &c);
        }
        source.event(PhaseEnded, &c);
    }
    source.event(SolveEnded, &c);
    for e in [SolveStarted, SolveEnded] {
        assert_eq!(
            source.source().events.iter().filter(|&&x| x == e).count(),
            1
        );
    }
    for e in [PhaseStarted, PhaseEnded] {
        assert_eq!(
            source.source().events.iter().filter(|&&x| x == e).count(),
            2
        );
    }
    for e in [StepStarted, StepEnded] {
        assert_eq!(
            source.source().events.iter().filter(|&&x| x == e).count(),
            4
        );
    }
    assert!(source.source().opens <= 4);
    assert!(source.source().sizes <= 4);
}
#[test]
fn one_variable() {
    reinitialize(false)
}
#[test]
fn multi_variable() {
    reinitialize(true)
}
#[test]
fn filter_out_assigned_values() {
    let p = ListProblem::new(
        vec!["1", "2", "3", "4", "5"],
        vec![ListEntity::new(vec!["1", "3"])],
    )
    .unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut source = SelectionCache::uniform(
        ListValueSource(ListValueSelection::Unassigned),
        SelectionCacheScope::Step,
        CachedSelectionOrder::Original,
        5,
    )
    .unwrap();
    for e in [SolveStarted, PhaseStarted, StepStarted] {
        source.event(e, &c).unwrap();
    }
    assert_eq!(source.size().unwrap(), 3);
    assert!(!source.is_never_ending());
    // Global, entity-independent and ending views retain the same finite values.
    for _ in 0..3 {
        let mut cursor = source.list_iter().unwrap().copied();
        assert_eq!(cursor.by_ref().collect::<Vec<_>>(), ["2", "4", "5"]);
        assert_eq!(cursor.next(), None);
        assert_eq!(cursor.next(), None);
    }
}
struct Infinite;
impl SelectionSource<(), usize> for Infinite {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        panic!("metadata must reject first")
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        panic!("must not open")
    }
}
#[test]
fn require_ending_child_value_selector() {
    assert!(matches!(
        SelectionCache::uniform(
            Infinite,
            SelectionCacheScope::Step,
            CachedSelectionOrder::Original,
            5
        ),
        Err(SelectionCacheError::UnboundedSource)
    ));
}
