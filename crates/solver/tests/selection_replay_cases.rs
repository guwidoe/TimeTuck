// Adapted from Timefold Community MimicReplayingEntitySelectorTest and
// MimicReplayingValueSelectorTest, Apache-2.0. See compat/provenance.json.
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Source {
    values: Vec<&'static str>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
}
impl SelectionSource<(), &'static str> for Source {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        self.opens += 1;
        Box::new(self.values.iter().copied())
    }
}
fn original(values: &[&'static str]) {
    let mut recording = RecordingSelection::new(Source {
        values: values.to_vec(),
        events: vec![],
        opens: 0,
    });
    let replaying = recording.replaying();
    // Native shared lifecycle has one forwarding owner; replay views observe its invalidation.
    recording.event(SolveStarted, &());
    for steps in [2, 1] {
        recording.event(PhaseStarted, &());
        for _ in 0..steps {
            recording.event(StepStarted, &());
            {
                let mut rec = recording.open(&());
                let mut replay = replaying.cursor();
                assert!(rec.has_next());
                assert!(replay.has_next());
                assert_eq!(*rec.next().unwrap(), values[0]);
                assert_eq!(*replay.next_replayed().unwrap().unwrap(), values[0]);
                assert!(rec.has_next());
                assert!(replay.has_next());
                assert_eq!(*rec.next().unwrap(), values[1]);
                assert_eq!(*replay.next_replayed().unwrap().unwrap(), values[1]);
                assert!(!replay.has_next());
                assert_eq!(replay.next_replayed().unwrap(), None);
                assert!(rec.has_next());
                assert!(replay.has_next());
                assert!(replay.has_next());
                assert_eq!(*rec.next().unwrap(), values[2]);
                assert_eq!(*replay.next_replayed().unwrap().unwrap(), values[2]);
                assert!(!rec.has_next());
                assert_eq!(rec.next(), None);
                assert!(!replay.has_next());
                assert_eq!(replay.next_replayed().unwrap(), None);
                assert!(!replay.has_next());
                assert_eq!(replay.next_replayed().unwrap(), None);
            }
            assert!(!recording.is_never_ending());
            assert!(!replaying.is_never_ending());
            assert_eq!(recording.size(&()), Some(3));
            assert_eq!(replaying.size(), Some(3));
            recording.event(StepEnded, &());
        }
        recording.event(PhaseEnded, &());
    }
    recording.event(SolveEnded, &());
    for (e, n) in [
        (SolveStarted, 1),
        (SolveEnded, 1),
        (PhaseStarted, 2),
        (PhaseEnded, 2),
        (StepStarted, 3),
        (StepEnded, 3),
    ] {
        assert_eq!(
            recording
                .source()
                .events
                .iter()
                .filter(|&&v| v == e)
                .count(),
            n
        );
    }
    assert_eq!(recording.source().opens, 3);
}
#[test]
fn entity_original_selection() {
    original(&["e1", "e2", "e3"]);
}
#[test]
fn value_original_selection() {
    original(&["v1", "v2", "v3"]);
}
