use std::{cell::Cell, num::NonZeroUsize};
use timetuck::*;
fn interval(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).unwrap()
}
struct Numbers {
    opens: usize,
    events: Vec<SelectionLifecycleEvent>,
}
impl SelectionSource<(), usize> for Numbers {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        None
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        Box::new(0..)
    }
}
#[test]
fn polling_spans_yields_fuses_after_stop_and_restarts_per_open() {
    let polls = Cell::new(0);
    let mut source = FilteredSelectionSource::new(
        Numbers {
            opens: 0,
            events: vec![],
        },
        |_: &(), n: &usize| n.is_multiple_of(2),
    )
    .with_termination(interval(3), || {
        polls.set(polls.get() + 1);
        true
    });
    source.event(SelectionLifecycleEvent::PhaseStarted, &());
    assert_eq!(source.size(&()), None);
    drop(source.open(&()));
    assert_eq!(source.source().opens, 0);
    for run in 1..=2 {
        let mut iter = source.open(&());
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(polls.get(), run - 1);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.size_hint(), (0, Some(0)));
        drop(iter);
        assert_eq!(polls.get(), run);
        assert_eq!(
            source.stats(),
            CandidateFilterStats {
                scanned: 3,
                rejected: 1,
                yielded: 2,
                stop: Some(CandidateFilterStop::Terminated)
            }
        );
    }
    assert_eq!(source.source().opens, 2);
    assert_eq!(
        source.source().events,
        [SelectionLifecycleEvent::PhaseStarted]
    );
}
#[test]
fn provider_limits_precede_poll_and_zero_budget_never_opens_child() {
    let opens = Cell::new(0);
    let polls = Cell::new(0);
    for (scan, selected, expected) in [
        (0, 3, CandidateFilterStop::ScanLimit),
        (3, 0, CandidateFilterStop::SelectionLimit),
        (2, 3, CandidateFilterStop::ScanLimit),
        (3, 2, CandidateFilterStop::SelectionLimit),
    ] {
        let mut p = FilteredMoveProvider::new(
            |_: &(), _: &(), _: u64| {
                opens.set(opens.get() + 1);
                vec![1, 2, 3]
            },
            scan,
            selected,
            |_: &(), _: &(), _: u64, _: &i32| true,
        )
        .with_termination(interval(2), || {
            polls.set(polls.get() + 1);
            true
        });
        assert_eq!(p.moves(&(), &(), 0).count(), scan.min(selected));
        assert_eq!(p.stats().stop, Some(expected));
    }
    assert_eq!(opens.get(), 2);
    assert_eq!(polls.get(), 0);
}
#[test]
fn early_drop_and_panicking_poll_do_not_claim_exhaustion_and_can_reopen() {
    let panic_once = Cell::new(true);
    let mut p = FilteredSelectionSource::new(
        Numbers {
            opens: 0,
            events: vec![],
        },
        |_: &(), _: &usize| true,
    )
    .with_termination(interval(1), || {
        if panic_once.replace(false) {
            panic!("poll failed");
        }
        true
    });
    assert_eq!(p.open(&()).next(), Some(0));
    assert_eq!(p.stats().stop, None);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut i = p.open(&());
        assert_eq!(i.next(), Some(0));
        i.next();
    }));
    assert!(r.is_err());
    assert_eq!(p.stats().stop, None);
    assert_eq!(p.open(&()).collect::<Vec<_>>(), [0]);
    assert_eq!(p.stats().stop, Some(CandidateFilterStop::Terminated));
}
struct RejectedMoves;
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for RejectedMoves {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(std::iter::repeat(Move::Change {
            variable: 0,
            value: Some(1),
        }))
    }
}
#[test]
fn cancellation_during_rejected_work_stops_both_basic_scoring_backends() {
    let problem = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut network = ConstraintNetwork::new(1);
    network
        .add_constraint("value", vec![0], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    for stream in [false, true] {
        let token = CancellationToken::new();
        let inspected = Cell::new(0);
        let mut provider = FilteredMoveProvider::new(
            RejectedMoves,
            usize::MAX,
            usize::MAX,
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, _: &Move<i32>| {
                inspected.set(inspected.get() + 1);
                if inspected.get() == 7 {
                    token.cancel();
                }
                false
            },
        )
        .with_termination(interval(4), || token.is_cancelled());
        let config = LocalSearchConfig {
            max_steps: 1,
            cancellation: Some(&token),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &problem,
                b.build(),
                source,
                problem.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &problem,
                &network,
                problem.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        assert_eq!(result.termination, LocalSearchTermination::Cancelled);
        assert_eq!(result.move_evaluations(), 0);
        let best = result.best.unwrap();
        assert_eq!(best.assignment, problem.initial_assignment());
        assert_eq!(best.score, SimpleScore::new(0));
        assert_eq!(inspected.get(), 8);
        assert_eq!(
            provider.stats(),
            CandidateFilterStats {
                scanned: 8,
                rejected: 8,
                yielded: 0,
                stop: Some(CandidateFilterStop::Terminated)
            }
        );
    }
}
