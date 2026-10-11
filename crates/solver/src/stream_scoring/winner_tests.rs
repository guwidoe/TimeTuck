//! Original execution/ownership tests; no upstream source-method credit.
use crate::*;
use crate::{
    list_search_scoring::ListSearchDirector, mixed_search_scoring::MixedSearchDirector,
    search_scoring::SearchDirector,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
#[derive(Default, Debug)]
struct Control {
    armed: Cell<bool>,
    calls: Cell<usize>,
    fail: Cell<Option<usize>>,
}
struct Value {
    id: usize,
    payload: i64,
    control: Rc<Control>,
}
impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.id, self.payload)
    }
}
impl Value {
    fn new(id: usize, payload: i64, c: &Rc<Control>) -> Self {
        Self {
            id,
            payload,
            control: c.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, r: &Self) -> bool {
        self.id == r.id
    }
}
impl Eq for Value {}
impl Clone for Value {
    fn clone(&self) -> Self {
        if self.control.armed.get() {
            let n = self.control.calls.get();
            self.control.calls.set(n + 1);
            assert_ne!(self.control.fail.get(), Some(n), "injected clone panic");
        }
        Self::new(self.id, self.payload, &self.control)
    }
}
fn basic(c: &Rc<Control>) -> FiniteProblem<Value> {
    FiniteProblem::new(
        (0..512)
            .map(|i| {
                Variable::new(vec![Value::new(0, 0, c), Value::new(1, 1, c)])
                    .with_initial(Value::new(0, i as i64 + 100, c))
            })
            .collect(),
    )
    .unwrap()
}
fn lists(c: &Rc<Control>) -> ListProblem<Value> {
    ListProblem::new(
        (0..512).map(|i| Value::new(i, 0, c)).collect(),
        (0..512)
            .map(|i| ListEntity::new(vec![Value::new(i, i as i64 + 100, c)]))
            .collect(),
    )
    .unwrap()
}
fn basic_move(c: &Rc<Control>) -> Move<Value> {
    Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(Value::new(1, -1, c)),
        },
        Move::Change {
            variable: 0,
            value: Some(Value::new(1, 700, c)),
        },
    ])
}
fn list_move() -> ListMove<Value> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
macro_rules! verify {
    ($c:ident,$new:expr,$movement:expr,$check:expr) => {{
        let c = &$c;
        let mut count = None;
        // First successful execution establishes the finite Clone failure sites.
        for failure in std::iter::once(None).chain((0..100).map(Some)) {
            if failure.is_some_and(|i| i >= count.unwrap()) {
                break;
            }
            c.armed.set(false);
            c.fail.set(None);
            c.calls.set(0);
            let mut d = $new;
            let movement = $movement;
            let original = format!("{:?}", d.assignment());
            let score = d.score();
            // Independent full-copy expected payloads before capture/install.
            let candidate = d.apply(&movement).unwrap();
            let expected = format!("{:?}", candidate.assignment());
            let expected_score = candidate.score();
            let expected_work = candidate.constraint_evaluations();
            drop(candidate);
            c.calls.set(0);
            c.fail.set(failure);
            c.armed.set(true);
            let result = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [movement],
                    1,
                    ForagerConfig::<SimpleScore>::default(),
                    |_, _| true,
                    |_| false,
                    false,
                )
            }));
            let calls = c.calls.get();
            c.armed.set(false);
            c.fail.set(None);
            if failure.is_none() {
                let selected = result.unwrap().unwrap();
                assert!(selected.selected.is_some());
                assert_eq!(
                    selected.constraint_evaluations, expected_work,
                    "install must not replay callbacks"
                );
                assert_eq!(format!("{:?}", d.assignment()), expected);
                assert_eq!(d.score(), expected_score);
                ($check)(&d);
                assert!(
                    calls < 100,
                    "sparse selection cloned {calls} payloads in a512-row world"
                );
                count = Some(calls);
            } else {
                assert!(result.is_err(), "clone site {failure:?} did not panic");
                assert_eq!(format!("{:?}", d.assignment()), original);
                assert_eq!(d.score(), score);
            }
            d.assert_consistent().unwrap();
        }
        count.unwrap()
    }};
}
#[test]
fn sparse_capture_clone_work_is_bounded_and_every_clone_panic_rolls_back() {
    let c = Rc::new(Control::default());
    let p = basic(&c);
    let lp = lists(&c);
    let mp = MixedProblem::new(p.clone(), lp.clone());
    let basic_count = verify!(
        c,
        {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = b.source::<usize, (usize, Option<Value>)>();
            b.score("payload", s.port(), |(_, v)| {
                SimpleScore::new(v.as_ref().unwrap().payload)
            })
            .unwrap();
            StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap()
        },
        basic_move(&c),
        |d: &StreamScoreDirector<'_, '_, Value, SimpleScore>| {
            assert_eq!(d.assignment().values()[0].as_ref().unwrap().payload, 700);
        }
    );
    let list_count = verify!(
        c,
        {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = ListStreamSources::<Value>::new(&mut b);
            b.score("payload", s.entities.port(), |(i, r)| {
                SimpleScore::new(if *i == 0 { r[0].payload } else { 0 })
            })
            .unwrap();
            ListStreamScoreDirector::new(&lp, b.build(), s, lp.initial_assignment()).unwrap()
        },
        list_move(),
        |d: &ListStreamScoreDirector<'_, '_, Value, SimpleScore>| {
            assert_eq!(d.assignment().lists()[0][0].payload, 101);
        }
    );
    let mixed_count = verify!(
        c,
        {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = MixedStreamSources::<Value, Value>::new(&mut b);
            b.score("payload", s.basic.port(), |(_, v)| {
                SimpleScore::new(v.as_ref().unwrap().payload)
            })
            .unwrap();
            MixedStreamScoreDirector::new(&mp, b.build(), s, mp.initial_assignment()).unwrap()
        },
        MixedMove {
            basic: basic_move(&c),
            lists: list_move()
        },
        |d: &MixedStreamScoreDirector<'_, '_, Value, Value, SimpleScore>| {
            assert_eq!(
                d.assignment().basic().values()[0].as_ref().unwrap().payload,
                700
            );
            assert_eq!(d.assignment().lists().lists()[0][0].payload, 101);
        }
    );
    eprintln!(
        "sparse512-world complete selection payload Clone counts: basic={basic_count}, list={list_count}, mixed={mixed_count}"
    );
}
