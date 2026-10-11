// Original native stream-consumption regressions; no Java hasNext parity claim.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Debug, PartialEq)]
enum Event {
    Poll(usize, usize),
    Draw(usize),
}
fn draw(index: usize) -> f64 {
    ((index * 17) % 64) as f64 / 64.0
}
struct Child {
    id: usize,
    next: usize,
    count: Option<usize>,
    events: Rc<RefCell<Vec<Event>>>,
}
impl Iterator for Child {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        self.events
            .borrow_mut()
            .push(Event::Poll(self.id, self.next));
        if self.count == Some(self.next) {
            return None;
        }
        let item = (self.id, self.next);
        self.next += 1;
        Some(item)
    }
}
// Independent eager-per-round buffer reference with linear CDF selection. Quarter
// weights and unit draws are exactly representable; no shared distribution helper.
fn reference(
    counts: &[Option<usize>],
    weights: &[f64],
    limit: usize,
) -> (Vec<(usize, usize)>, Vec<Event>) {
    let mut positions = vec![0; counts.len()];
    let mut buffers = vec![None; counts.len()];
    let mut weights = weights.to_vec();
    let mut events = vec![];
    let mut output = vec![];
    for step in 0..limit {
        for i in 0..counts.len() {
            if weights[i] > 0.0 && buffers[i].is_none() {
                events.push(Event::Poll(i, positions[i]));
                if counts[i] == Some(positions[i]) {
                    weights[i] = 0.0;
                } else {
                    buffers[i] = Some((i, positions[i]));
                    positions[i] += 1;
                }
            }
        }
        let total: f64 = weights.iter().sum();
        if total == 0.0 {
            break;
        }
        events.push(Event::Draw(step));
        let target = draw(step) * total;
        let mut cumulative = 0.0;
        let index = weights
            .iter()
            .position(|weight| {
                cumulative += weight;
                cumulative > target
            })
            .unwrap();
        output.push(buffers[index].take().unwrap());
    }
    (output, events)
}
#[test]
fn complete_output_and_poll_draw_order_match_independent_buffer_reference() {
    for families in [0, 1, 3, 64] {
        for infinite in [false, true] {
            let counts: Vec<_> = (0..families)
                .map(|i| {
                    if infinite && i % 3 == 0 {
                        None
                    } else {
                        Some(i % 5)
                    }
                })
                .collect();
            let weights: Vec<_> = (0..families)
                .map(|i| [0.25, 0.0, 0.75, 1.25, 2.0][i % 5])
                .collect();
            let (expected, events) = reference(&counts, &weights, 512);
            let actual_events = Rc::new(RefCell::new(vec![]));
            let children = counts
                .iter()
                .enumerate()
                .map(|(i, &count)| {
                    (
                        Child {
                            id: i,
                            next: 0,
                            count,
                            events: actual_events.clone(),
                        },
                        weights[i],
                    )
                })
                .collect();
            let draw_events = actual_events.clone();
            let mut step = 0;
            let mut cursor = WeightedInterleave::new(children, move || {
                draw_events.borrow_mut().push(Event::Draw(step));
                let d = draw(step);
                step += 1;
                d
            })
            .unwrap();
            assert!(actual_events.borrow().is_empty());
            let actual = cursor
                .by_ref()
                .take(512)
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert_eq!(actual, expected, "families={families} infinite={infinite}");
            assert_eq!(*actual_events.borrow(), events);
            if actual.len() < 512 {
                assert_eq!(cursor.next(), None);
                assert_eq!(*actual_events.borrow(), events);
            }
            drop(cursor); // Dropping a partially consumed stream does not refill.
            assert_eq!(*actual_events.borrow(), events);
        }
    }
}
#[test]
fn caught_priming_panic_preserves_prior_retirement_and_buffered_values() {
    let mut polls = [0; 3];
    let children: Vec<(Box<dyn Iterator<Item = usize> + '_>, f64)> = polls
        .iter_mut()
        .enumerate()
        .map(|(i, p)| {
            let cursor = std::iter::from_fn(move || {
                *p += 1;
                if i == 1 && *p == 1 {
                    panic!("reader");
                }
                if i == 0 || *p > 2 { None } else { Some(i) }
            });
            (Box::new(cursor) as Box<dyn Iterator<Item = usize>>, 1.0)
        })
        .collect();
    let mut cursor = WeightedInterleave::new(children, || 0.0).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
    assert_eq!(cursor.next(), Some(Ok(1)));
    assert_eq!(
        cursor.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
        [2, 2]
    );
    drop(cursor);
    assert_eq!(polls, [1, 3, 3]);
}
#[test]
fn caught_selected_refill_and_draw_panics_do_not_lose_buffered_items() {
    let mut polls = 0;
    let source = std::iter::from_fn(|| {
        polls += 1;
        match polls {
            1 => Some(10),
            2 => panic!("refill"),
            3 => Some(20),
            _ => None,
        }
    });
    let mut draws = 0;
    let mut cursor = WeightedInterleave::new(vec![(source, 1.0)], || {
        draws += 1;
        if draws == 1 {
            panic!("draw");
        }
        0.0
    })
    .unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
    assert_eq!(cursor.next(), Some(Ok(10)));
    assert!(catch_unwind(AssertUnwindSafe(|| cursor.next())).is_err());
    assert_eq!(cursor.next(), Some(Ok(20)));
    assert_eq!(cursor.next(), None);
    drop(cursor);
    assert_eq!(polls, 4);
    assert_eq!(draws, 3);
}
