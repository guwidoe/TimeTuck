// Adapted from Timefold Community FlattenLastUniNodeTest. Apache-2.0.
// Copyright Timefold BV and contributors. See compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::{StableFlatMapNode, TupleChange};
// Reuse the same references to cover interned duplicate identity explicitly.
static A: &str = "A";
static B: &str = "B";
static C: &str = "C";
static X: &str = "X";
type Node = StableFlatMapNode<'static, usize, Vec<&'static str>, &'static str, &'static str>;
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Event {
    Insert(&'static str),
    Update(&'static str),
    Retract(&'static str),
}
#[derive(Default)]
struct Downstream {
    rows: BTreeMap<(usize, usize), &'static str>,
    events: Vec<Event>,
}
impl Downstream {
    fn propagate(&mut self, node: &mut Node) {
        for change in node.propagate() {
            self.events.push(match change.value {
                Some(v) => {
                    if self.rows.insert(change.id, v).is_some() {
                        Event::Update(v)
                    } else {
                        Event::Insert(v)
                    }
                }
                None => Event::Retract(
                    self.rows
                        .remove(&change.id)
                        .expect("retract existing occurrence"),
                ),
            });
        }
    }
    fn expect(&mut self, mut expected: Vec<Event>) {
        self.events.sort();
        expected.sort();
        assert_eq!(self.events, expected);
        self.events.clear();
    }
}
fn node() -> Node {
    StableFlatMapNode::new(Clone::clone, |v| *v)
}
#[test]
fn insert_and_retract() {
    use Event::*;
    let mut n = node();
    let mut d = Downstream::default();
    n.enqueue(TupleChange::put(0, vec![A, B]));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Insert(A), Insert(B)]);
    n.enqueue(TupleChange::put(1, vec![B, C]));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Insert(B), Insert(C)]);
    n.enqueue(TupleChange::retract(0));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Retract(A), Retract(B)]);
    n.enqueue(TupleChange::retract(1));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Retract(B), Retract(C)]);
    d.propagate(&mut n);
    assert!(d.events.is_empty());
    assert!(d.rows.is_empty());
}
#[test]
fn modify() {
    use Event::*;
    let mut n = node();
    let mut d = Downstream::default();
    n.enqueue(TupleChange::put(0, vec![A, B]));
    n.enqueue(TupleChange::put(1, vec![B, C]));
    d.propagate(&mut n);
    d.events.clear();
    n.enqueue(TupleChange::put(0, vec![B, X, B]));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Retract(A), Update(B), Insert(X), Insert(B)]);
    n.enqueue(TupleChange::put(1, vec![X, C]));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Retract(B), Update(C), Insert(X)]);
    n.enqueue(TupleChange::put(0, vec![X]));
    assert!(d.events.is_empty());
    d.propagate(&mut n);
    d.expect(vec![Retract(B), Retract(B), Update(X)]);
}
