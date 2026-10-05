// Adapted from pinned Timefold Community placement tests and their assertion
// helpers (Apache-2.0). See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use SelectionLifecycleEvent::*;
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
type Events = Rc<RefCell<Vec<SelectionLifecycleEvent>>>;
struct Fixed<T> {
    items: Vec<T>,
    events: Events,
}
impl<T> Fixed<T> {
    fn new(items: Vec<T>) -> Self {
        Self {
            items,
            events: Rc::default(),
        }
    }
}
impl<T: Clone> SelectionSource<(), T> for Fixed<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.borrow_mut().push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(self.items.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        Box::new(self.items.iter().cloned())
    }
}
struct Changes {
    values: Fixed<&'static str>,
    by_value: bool,
}
impl PlacementSource<(), &'static str, String> for Changes {
    fn event(&mut self, e: SelectionLifecycleEvent, c: &()) {
        self.values.event(e, c);
    }
    fn size(&mut self, c: &(), _: &&'static str) -> Option<usize> {
        self.values.size(c)
    }
    fn open<'a>(
        &'a mut self,
        c: &'a (),
        key: &'a &'static str,
    ) -> Box<dyn Iterator<Item = String> + 'a>
    where
        String: 'a,
    {
        let reverse = self.by_value;
        Box::new(self.values.open(c).map(move |v| {
            if reverse {
                format!("{v}->{key}")
            } else {
                format!("{key}->{v}")
            }
        }))
    }
}
struct Joint(SelectionProduct<(), &'static str, Fixed<&'static str>>);
impl PlacementSource<(), &'static str, String> for Joint {
    fn event(&mut self, e: SelectionLifecycleEvent, c: &()) {
        self.0.event(e, c);
    }
    fn size(&mut self, _: &(), _: &&'static str) -> Option<usize> {
        None
    }
    fn open<'a>(
        &'a mut self,
        c: &'a (),
        key: &'a &'static str,
    ) -> Box<dyn Iterator<Item = String> + 'a>
    where
        String: 'a,
    {
        let product = self.0.snapshot(c).unwrap();
        Box::new(
            product
                .iter()
                .map(move |values| format!("{key}->{}+{key}->{}", values[0], values[1])),
        )
    }
}
fn exercise<K, KS: SelectionSource<(), K>, S: PlacementSource<(), K, String>>(
    q: &mut PlacementQueue<(), K, String, KS, S>,
    phases: Vec<Vec<Vec<&str>>>,
    finite_first: bool,
    events: Vec<Events>,
) {
    q.event(SolveStarted, &()).unwrap();
    let mut steps = 0;
    for (phase, pools) in phases.iter().enumerate() {
        q.event(PhaseStarted, &()).unwrap();
        for expected in pools {
            assert!(q.has_next().unwrap());
            q.event(StepStarted, &()).unwrap();
            let placement = q.next_placement(&()).unwrap().unwrap();
            let mut iter = placement.iter();
            for code in expected {
                assert_eq!(iter.next().map(String::as_str), Some(*code));
            }
            assert!(iter.next().is_none());
            q.event(StepEnded, &()).unwrap();
            steps += 1;
        }
        if phase == 0 && finite_first {
            assert!(!q.has_next().unwrap());
            assert!(q.next_placement(&()).unwrap().is_none());
        }
        q.event(PhaseEnded, &()).unwrap();
    }
    q.event(SolveEnded, &()).unwrap();
    for log in events {
        let log = log.borrow();
        for (event, count) in [
            (SolveStarted, 1),
            (SolveEnded, 1),
            (PhaseStarted, phases.len()),
            (PhaseEnded, phases.len()),
            (StepStarted, steps),
            (StepEnded, steps),
        ] {
            assert_eq!(
                log.iter().filter(|&&e| e == event).count(),
                count,
                "{event:?}"
            );
        }
    }
}
#[test]
fn queued_entity_one_move_selector() {
    let keys = Fixed::new(vec!["a", "b", "c"]);
    let values = Fixed::new(vec!["1", "2"]);
    let logs = vec![keys.events.clone(), values.events.clone()];
    let mut q = PlacementQueue::new(
        keys,
        vec![Changes {
            values,
            by_value: false,
        }],
        PlacementOrder::Once,
        3,
        2,
    )
    .unwrap();
    exercise(
        &mut q,
        vec![
            vec![
                vec!["a->1", "a->2"],
                vec!["b->1", "b->2"],
                vec!["c->1", "c->2"],
            ],
            vec![vec!["a->1", "a->2"]],
        ],
        true,
        logs,
    );
}
#[test]
fn queued_entity_multi_queued_move_selector() {
    let keys = Fixed::new(vec!["a", "b"]);
    let primary = Fixed::new(vec!["1", "2", "3"]);
    let secondary = Fixed::new(vec!["8", "9"]);
    let logs = vec![
        keys.events.clone(),
        primary.events.clone(),
        secondary.events.clone(),
    ];
    let mut q = PlacementQueue::new(
        keys,
        vec![
            Changes {
                values: primary,
                by_value: false,
            },
            Changes {
                values: secondary,
                by_value: false,
            },
        ],
        PlacementOrder::Once,
        2,
        3,
    )
    .unwrap();
    exercise(
        &mut q,
        vec![vec![
            vec!["a->1", "a->2", "a->3"],
            vec!["a->8", "a->9"],
            vec!["b->1", "b->2", "b->3"],
            vec!["b->8", "b->9"],
        ]],
        true,
        logs,
    );
}
#[test]
fn queued_entity_cartesian_product_move_selector() {
    let keys = Fixed::new(vec!["a", "b"]);
    let primary = Fixed::new(vec!["1", "2", "3"]);
    let secondary = Fixed::new(vec!["8", "9"]);
    let logs = vec![
        keys.events.clone(),
        primary.events.clone(),
        secondary.events.clone(),
    ];
    let product = Joint(SelectionProduct::new(vec![primary, secondary], true, 3));
    let mut q = PlacementQueue::new(keys, vec![product], PlacementOrder::Once, 2, 6).unwrap();
    exercise(
        &mut q,
        vec![vec![
            vec![
                "a->1+a->8",
                "a->1+a->9",
                "a->2+a->8",
                "a->2+a->9",
                "a->3+a->8",
                "a->3+a->9",
            ],
            vec![
                "b->1+b->8",
                "b->1+b->9",
                "b->2+b->8",
                "b->2+b->9",
                "b->3+b->8",
                "b->3+b->9",
            ],
        ]],
        true,
        logs,
    );
}
#[test]
fn queued_value_one_move_selector() {
    let keys = Fixed::new(vec!["1", "2"]);
    let entities = Fixed::new(vec!["a", "b", "c"]);
    let logs = vec![keys.events.clone(), entities.events.clone()];
    let mut q = PlacementQueue::new(
        keys,
        vec![Changes {
            values: entities,
            by_value: true,
        }],
        PlacementOrder::Cycle,
        2,
        3,
    )
    .unwrap();
    exercise(
        &mut q,
        vec![
            vec![
                vec!["a->1", "b->1", "c->1"],
                vec!["a->2", "b->2", "c->2"],
                vec!["a->1", "b->1", "c->1"],
            ],
            vec![vec!["a->1", "b->1", "c->1"]],
        ],
        false,
        logs,
    );
}
#[test]
fn pooled_one_move_selector() {
    let source = Fixed::new(vec!["a1".to_owned(), "a2".to_owned(), "b1".to_owned()]);
    let logs = vec![source.events.clone()];
    let mut q = pooled_placements(source, 3).unwrap();
    exercise(
        &mut q,
        vec![
            vec![vec!["a1", "a2", "b1"]; 3],
            vec![vec!["a1", "a2", "b1"]],
        ],
        false,
        logs,
    );
}
