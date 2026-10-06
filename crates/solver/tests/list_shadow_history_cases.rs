// Adapted from ListVariableShadowVariablesTest and its shadowhistory fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native list moves and typed publications replace Java moves and mutable setters.
use std::collections::BTreeMap;
use timetuck::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Value(&'static str);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Owner(&'static str);
struct Solution;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "valueList",
            false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Owner>("entity"),
            PlanningVariableDefinition::shadow::<usize>("index"),
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<Value>("next"),
        ]),
    ])
    .unwrap()
}
#[derive(Default)]
struct History {
    fields: ListBuiltinValues<Owner, Value>,
    entity: Vec<Option<Owner>>,
    index: Vec<Option<usize>>,
    previous: Vec<Option<Value>>,
    next: Vec<Option<Value>>,
}
struct Fixture<'s> {
    problem: ListProblem<Value>,
    assignment: ListAssignment<Value>,
    owners: Vec<Owner>,
    registry: VariableStateRegistry<'s>,
    supply: ListStateSupply<'s, Owner, Value>,
    history: BTreeMap<Value, History>,
}
impl<'s> Fixture<'s> {
    fn new(s: &'s PlanningSchema, rows: Vec<(Owner, Vec<Value>)>, extra: Vec<Value>) -> Self {
        let values = rows
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .chain(extra)
            .collect::<Vec<_>>();
        let problem = ListProblem::new(
            values.clone(),
            rows.iter()
                .map(|(_, v)| ListEntity::new(v.clone()))
                .collect(),
        )
        .unwrap();
        let assignment = problem.initial_assignment();
        let field = s.entity::<Owner>().unwrap().variable("valueList").unwrap();
        let mut registry = VariableStateRegistry::new(s);
        registry.reset_lists(field, rows.clone()).unwrap();
        let mut supply = ListStateSupply::new(
            registry.list::<Owner, Value>(field).unwrap(),
            values.iter().copied(),
        )
        .unwrap();
        let entity = s.entity::<Value>().unwrap();
        for (property, name) in [
            (ListShadowProperty::Owner, "entity"),
            (ListShadowProperty::Index, "index"),
            (ListShadowProperty::Previous, "previous"),
            (ListShadowProperty::Next, "next"),
        ] {
            supply
                .externalize(property, entity.variable(name).unwrap())
                .unwrap();
        }
        let mut fixture = Self {
            problem,
            assignment,
            owners: rows.iter().map(|(e, _)| *e).collect(),
            registry,
            supply,
            history: values
                .into_iter()
                .map(|v| (v, History::default()))
                .collect(),
        };
        fixture.refresh();
        fixture
    }
    fn refresh(&mut self) {
        let update = self.supply.refresh_all().unwrap();
        assert!(update.notifications.is_empty());
        for change in update.shadows {
            let h = self.history.get_mut(&change.value).unwrap();
            assert_eq!(h.fields.get(change.after.property()), change.before);
            h.fields.apply(&change.after);
            match change.after {
                ListBuiltinValue::Owner(v) => h.entity.push(v),
                ListBuiltinValue::Index(v) => h.index.push(v),
                ListBuiltinValue::Previous(v) => h.previous.push(v),
                ListBuiltinValue::Next(v) => h.next.push(v),
            }
        }
    }
    fn slot(&self, owner: Owner) -> usize {
        self.owners.iter().position(|&e| e == owner).unwrap()
    }
    fn pos(&self, owner: Owner, index: usize) -> ListPosition {
        ListPosition {
            entity: self.slot(owner),
            index,
        }
    }
    fn apply(&mut self, movement: ListMove<Value>) {
        self.problem
            .apply(&mut self.assignment, &movement)
            .unwrap()
            .commit();
        let bindings = self
            .owners
            .iter()
            .enumerate()
            .map(|(i, &e)| (e, i))
            .collect::<Vec<_>>();
        self.registry
            .synchronize_lists(self.supply.source(), &self.assignment, &bindings)
            .unwrap();
        self.refresh();
    }
    fn change(&mut self, source: Owner, index: usize, destination: Owner, to: usize) {
        self.apply(ListMove::Change {
            source: self.pos(source, index),
            destination: self.pos(destination, to),
        });
    }
    fn swap(&mut self, left: Owner, index: usize, right: Owner, other: usize) {
        self.apply(ListMove::Swap {
            left: self.pos(left, index),
            right: self.pos(right, other),
        });
    }
    fn sub_change(
        &mut self,
        source: Owner,
        span: std::ops::Range<usize>,
        destination: Owner,
        to: usize,
        reverse: bool,
    ) {
        self.apply(ListMove::SubListChange {
            entity: self.slot(source),
            start: span.start,
            end: span.end,
            destination: self.pos(destination, to),
            reverse,
        });
    }
    fn sub_swap(
        &mut self,
        left: Owner,
        l: std::ops::Range<usize>,
        right: Owner,
        r: std::ops::Range<usize>,
        reverse: bool,
    ) {
        self.apply(ListMove::SubListSwap {
            left: ListRange {
                entity: self.slot(left),
                start: l.start,
                end: l.end,
            },
            right: ListRange {
                entity: self.slot(right),
                start: r.start,
                end: r.end,
            },
            reverse,
        });
    }
    fn list(&self, owner: Owner) -> &[Value] {
        self.assignment.list(self.slot(owner)).unwrap()
    }
    fn entity(&self, value: Value, expected: &[Option<Owner>]) {
        assert_eq!(self.history[&value].entity, expected);
        assert_eq!(
            self.history[&value].fields.owner,
            expected.last().copied().flatten()
        );
    }
    fn index(&self, value: Value, expected: &[Option<usize>]) {
        assert_eq!(self.history[&value].index, expected);
        assert_eq!(
            self.history[&value].fields.index,
            expected.last().copied().flatten()
        );
    }
    fn previous(&self, value: Value, expected: &[Option<Value>]) {
        assert_eq!(self.history[&value].previous, expected);
        assert_eq!(
            self.history[&value].fields.previous,
            expected.last().copied().flatten()
        );
    }
    fn next(&self, value: Value, expected: &[Option<Value>]) {
        assert_eq!(self.history[&value].next, expected);
        assert_eq!(
            self.history[&value].fields.next,
            expected.last().copied().flatten()
        );
    }
    fn remove_owners(&mut self) {
        self.registry
            .reset_lists(self.supply.source(), [] as [(Owner, Vec<Value>); 0])
            .unwrap();
        self.refresh();
    }
    fn score(&self) -> SimpleScore {
        ListConstraintNetwork::<Value, SimpleScore>::new(
            self.owners.len(),
            self.problem.values().len(),
        )
        .calculate(&self.problem, &self.assignment)
        .unwrap()
    }
}

#[test]
fn add_and_remove_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.refresh();
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a)]);
    context.previous(c, &[Some(b)]);
    context.next(a, &[Some(b)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[]);
    context.remove_owners();
    context.refresh();
    context.entity(a, &[Some(ann), None]);
    context.entity(b, &[Some(ann), None]);
    context.entity(c, &[Some(ann), None]);
    context.index(a, &[Some(0), None]);
    context.index(b, &[Some(1), None]);
    context.index(c, &[Some(2), None]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), None]);
    context.previous(c, &[Some(b), None]);
    context.next(a, &[Some(b), None]);
    context.next(b, &[Some(c), None]);
    context.next(c, &[]);
}

#[test]
fn add_and_remove_element() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let mut context = Fixture::new(&s, rows, vec![x]);
    assert_eq!(context.score(), SimpleScore::ZERO.with_structural(-1));
    assert_eq!(context.supply.unassigned_count(), 1);
    context.apply(ListMove::Assign {
        value: x,
        destination: context.pos(ann, 2),
    });
    assert_eq!(context.list(ann), [a, b, x, c]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(x, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1)]);
    context.index(x, &[Some(2)]);
    context.index(c, &[Some(2), Some(3)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a)]);
    context.previous(x, &[Some(b)]);
    context.previous(c, &[Some(b), Some(x)]);
    context.next(a, &[Some(b)]);
    context.next(b, &[Some(c), Some(x)]);
    context.next(x, &[Some(c)]);
    context.next(c, &[]);
    context.apply(ListMove::Unassign {
        entity: context.slot(ann),
        start: 1,
        end: 2,
    });
    assert_eq!(context.list(ann), [a, x, c]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann), None]);
    context.entity(x, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), None]);
    context.index(x, &[Some(2), Some(1)]);
    context.index(c, &[Some(2), Some(3), Some(2)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), None]);
    context.previous(x, &[Some(b), Some(a)]);
    context.previous(c, &[Some(b), Some(x)]);
    context.next(a, &[Some(b), Some(x)]);
    context.next(b, &[Some(c), Some(x), None]);
    context.next(x, &[Some(c)]);
    context.next(c, &[]);
}

#[test]
fn move_element_to_lower_index_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 3, ann, 1);
    assert_eq!(context.list(ann), [a, d, b, c, e]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(2)]);
    context.index(c, &[Some(2), Some(3)]);
    context.index(d, &[Some(3), Some(1)]);
    context.index(e, &[Some(4)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(d)]);
    context.previous(c, &[Some(b)]);
    context.previous(d, &[Some(c), Some(a)]);
    context.previous(e, &[Some(d), Some(c)]);
    context.next(a, &[Some(b), Some(d)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[Some(d), Some(e)]);
    context.next(d, &[Some(e), Some(b)]);
    context.next(e, &[]);
}

#[test]
fn move_element_to_higher_index_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 0, ann, 2);
    assert_eq!(context.list(ann), [b, c, a, d, e]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0), Some(2)]);
    context.index(b, &[Some(1), Some(0)]);
    context.index(c, &[Some(2), Some(1)]);
    context.index(d, &[Some(3)]);
    context.index(e, &[Some(4)]);
    context.previous(a, &[Some(c)]);
    context.previous(b, &[Some(a), None]);
    context.previous(c, &[Some(b)]);
    context.previous(d, &[Some(c), Some(a)]);
    context.previous(e, &[Some(d)]);
    context.next(a, &[Some(b), Some(d)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[Some(d), Some(a)]);
    context.next(d, &[Some(e)]);
    context.next(e, &[]);
}

#[test]
fn move_element_to_another_entity_change_index() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let y = Value("Y");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x, y]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 0, bob, 1);
    assert_eq!(context.list(ann), [b, c]);
    assert_eq!(context.list(bob), [x, a, y]);
    context.entity(a, &[Some(ann), Some(bob)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(x, &[Some(bob)]);
    context.entity(y, &[Some(bob)]);
    context.index(a, &[Some(0), Some(1)]);
    context.index(b, &[Some(1), Some(0)]);
    context.index(c, &[Some(2), Some(1)]);
    context.index(x, &[Some(0)]);
    context.index(y, &[Some(1), Some(2)]);
    context.previous(a, &[Some(x)]);
    context.previous(b, &[Some(a), None]);
    context.previous(c, &[Some(b)]);
    context.previous(x, &[]);
    context.previous(y, &[Some(x), Some(a)]);
    context.next(a, &[Some(b), Some(y)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[]);
    context.next(x, &[Some(y), Some(a)]);
    context.next(y, &[]);
}

#[test]
fn move_element_to_another_entity_same_index() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let y = Value("Y");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x, y]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 1, bob, 1);
    assert_eq!(context.list(ann), [a, c]);
    assert_eq!(context.list(bob), [x, b, y]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann), Some(bob)]);
    context.entity(c, &[Some(ann)]);
    context.entity(x, &[Some(bob)]);
    context.entity(y, &[Some(bob)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2), Some(1)]);
    context.index(x, &[Some(0)]);
    context.index(y, &[Some(1), Some(2)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(x)]);
    context.previous(c, &[Some(b), Some(a)]);
    context.previous(x, &[]);
    context.previous(y, &[Some(x), Some(b)]);
    context.next(a, &[Some(b), Some(c)]);
    context.next(b, &[Some(c), Some(y)]);
    context.next(c, &[]);
    context.next(x, &[Some(y), Some(b)]);
    context.next(y, &[]);
}

#[test]
fn move_one_up_to_end() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 1, ann, 2);
    assert_eq!(context.list(ann), [a, c, b]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(2)]);
    context.index(c, &[Some(2), Some(1)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(c)]);
    context.previous(c, &[Some(b), Some(a)]);
    context.next(a, &[Some(b), Some(c)]);
    context.next(b, &[Some(c), None]);
    context.next(c, &[Some(b)]);
}

#[test]
fn move_one_down_to_start() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.change(ann, 1, ann, 0);
    assert_eq!(context.list(ann), [b, a, c]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.index(a, &[Some(0), Some(1)]);
    context.index(b, &[Some(1), Some(0)]);
    context.index(c, &[Some(2)]);
    context.previous(a, &[Some(b)]);
    context.previous(b, &[Some(a), None]);
    context.previous(c, &[Some(b), Some(a)]);
    context.next(a, &[Some(b), Some(c)]);
    context.next(b, &[Some(c), Some(a)]);
    context.next(c, &[]);
}

#[test]
fn swap_elements_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.swap(ann, 1, ann, 3);
    assert_eq!(context.list(ann), [a, d, c, b, e]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(3)]);
    context.index(c, &[Some(2)]);
    context.index(d, &[Some(3), Some(1)]);
    context.index(e, &[Some(4)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(c)]);
    context.previous(c, &[Some(b), Some(d)]);
    context.previous(d, &[Some(c), Some(a)]);
    context.previous(e, &[Some(d), Some(b)]);
    context.next(a, &[Some(b), Some(d)]);
    context.next(b, &[Some(c), Some(e)]);
    context.next(c, &[Some(d), Some(b)]);
    context.next(d, &[Some(e), Some(c)]);
    context.next(e, &[]);
}

#[test]
fn swap_elements_another_entity_change_index() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let y = Value("Y");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x, y]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.swap(ann, 0, bob, 1);
    assert_eq!(context.list(ann), [y, b, c]);
    assert_eq!(context.list(bob), [x, a]);
    context.entity(a, &[Some(ann), Some(bob)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(x, &[Some(bob)]);
    context.entity(y, &[Some(bob), Some(ann)]);
    context.index(a, &[Some(0), Some(1)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2)]);
    context.index(x, &[Some(0)]);
    context.index(y, &[Some(1), Some(0)]);
    context.previous(a, &[Some(x)]);
    context.previous(b, &[Some(a), Some(y)]);
    context.previous(c, &[Some(b)]);
    context.previous(x, &[]);
    context.previous(y, &[Some(x), None]);
    context.next(a, &[Some(b), None]);
    context.next(b, &[Some(c)]);
    context.next(c, &[]);
    context.next(x, &[Some(y), Some(a)]);
    context.next(y, &[Some(b)]);
}

#[test]
fn swap_elements_another_entity_same_index() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let y = Value("Y");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x, y]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.swap(ann, 1, bob, 1);
    assert_eq!(context.list(ann), [a, y, c]);
    assert_eq!(context.list(bob), [x, b]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann), Some(bob)]);
    context.entity(c, &[Some(ann)]);
    context.entity(x, &[Some(bob)]);
    context.entity(y, &[Some(bob), Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2)]);
    context.index(x, &[Some(0)]);
    context.index(y, &[Some(1)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(x)]);
    context.previous(c, &[Some(b), Some(y)]);
    context.previous(x, &[]);
    context.previous(y, &[Some(x), Some(a)]);
    context.next(a, &[Some(b), Some(y)]);
    context.next(b, &[Some(c), None]);
    context.next(c, &[]);
    context.next(x, &[Some(y), Some(b)]);
    context.next(y, &[Some(c)]);
}

#[test]
fn swap_neighbors() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.swap(ann, 1, ann, 2);
    assert_eq!(context.list(ann), [a, c, b, d]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(2)]);
    context.index(c, &[Some(2), Some(1)]);
    context.index(d, &[Some(3)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(c)]);
    context.previous(c, &[Some(b), Some(a)]);
    context.previous(d, &[Some(c), Some(b)]);
    context.next(a, &[Some(b), Some(c)]);
    context.next(b, &[Some(c), Some(d)]);
    context.next(c, &[Some(d), Some(b)]);
    context.next(d, &[]);
}

#[test]
fn move_sub_list_to_higher_index_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_change(ann, 1..3, ann, 3, false);
    assert_eq!(context.list(ann), [a, d, e, b, c]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(3)]);
    context.index(c, &[Some(2), Some(4)]);
    context.index(d, &[Some(3), Some(1)]);
    context.index(e, &[Some(4), Some(2)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(e)]);
    context.previous(c, &[Some(b)]);
    context.previous(d, &[Some(c), Some(a)]);
    context.previous(e, &[Some(d)]);
    context.next(a, &[Some(b), Some(d)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[Some(d), None]);
    context.next(d, &[Some(e)]);
    context.next(e, &[Some(b)]);
}

#[test]
fn move_sub_list_to_lower_index_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_change(ann, 1..4, ann, 0, false);
    assert_eq!(context.list(ann), [b, c, d, a, e]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0), Some(3)]);
    context.index(b, &[Some(1), Some(0)]);
    context.index(c, &[Some(2), Some(1)]);
    context.index(d, &[Some(3), Some(2)]);
    context.index(e, &[Some(4)]);
    context.previous(a, &[Some(d)]);
    context.previous(b, &[Some(a), None]);
    context.previous(c, &[Some(b)]);
    context.previous(d, &[Some(c)]);
    context.previous(e, &[Some(d), Some(a)]);
    context.next(a, &[Some(b), Some(e)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[Some(d)]);
    context.next(d, &[Some(e), Some(a)]);
    context.next(e, &[]);
}

#[test]
fn move_sub_list_to_another_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let x = Value("X");
    let y = Value("Y");
    let z = Value("Z");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x, y, z]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_change(ann, 1..4, bob, 2, false);
    assert_eq!(context.list(ann), [a, e]);
    assert_eq!(context.list(bob), [x, y, b, c, d, z]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann), Some(bob)]);
    context.entity(c, &[Some(ann), Some(bob)]);
    context.entity(d, &[Some(ann), Some(bob)]);
    context.entity(e, &[Some(ann)]);
    context.entity(x, &[Some(bob)]);
    context.entity(y, &[Some(bob)]);
    context.entity(z, &[Some(bob)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1), Some(2)]);
    context.index(c, &[Some(2), Some(3)]);
    context.index(d, &[Some(3), Some(4)]);
    context.index(e, &[Some(4), Some(1)]);
    context.index(x, &[Some(0)]);
    context.index(y, &[Some(1)]);
    context.index(z, &[Some(2), Some(5)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(y)]);
    context.previous(c, &[Some(b)]);
    context.previous(d, &[Some(c)]);
    context.previous(e, &[Some(d), Some(a)]);
    context.previous(x, &[]);
    context.previous(y, &[Some(x)]);
    context.previous(z, &[Some(y), Some(d)]);
    context.next(a, &[Some(b), Some(e)]);
    context.next(b, &[Some(c)]);
    context.next(c, &[Some(d)]);
    context.next(d, &[Some(e), Some(z)]);
    context.next(e, &[]);
    context.next(x, &[Some(y)]);
    context.next(y, &[Some(z), Some(b)]);
    context.next(z, &[]);
}

#[test]
fn move_tail_sub_list() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let x = Value("X");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![x]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_change(ann, 1..3, bob, 1, false);
    assert_eq!(context.list(ann), [a]);
    assert_eq!(context.list(bob), [x, b, c]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann), Some(bob)]);
    context.entity(c, &[Some(ann), Some(bob)]);
    context.index(a, &[Some(0)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2)]);
    context.previous(a, &[]);
    context.previous(b, &[Some(a), Some(x)]);
    context.previous(c, &[Some(b)]);
    context.previous(x, &[]);
    context.next(a, &[Some(b), None]);
    context.next(b, &[Some(c)]);
    context.next(c, &[]);
    context.next(x, &[Some(b)]);
}

#[test]
fn move_whole_list_reversing() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c]));
    let bob = Owner("Bob");
    rows.push((bob, vec![]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_change(ann, 0..3, bob, 0, true);
    assert!(context.list(ann).is_empty());
    assert_eq!(context.list(bob), [c, b, a]);
    context.entity(a, &[Some(ann), Some(bob)]);
    context.entity(b, &[Some(ann), Some(bob)]);
    context.entity(c, &[Some(ann), Some(bob)]);
    context.index(a, &[Some(0), Some(2)]);
    context.index(b, &[Some(1)]);
    context.index(c, &[Some(2), Some(0)]);
    context.previous(a, &[Some(b)]);
    context.previous(b, &[Some(a), Some(c)]);
    context.previous(c, &[Some(b), None]);
    context.next(a, &[Some(b), None]);
    context.next(b, &[Some(c), Some(a)]);
    context.next(c, &[Some(b)]);
}

#[test]
fn swap_sub_lists_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let f = Value("F");
    let g = Value("G");
    let h = Value("H");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e, f, g, h]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_swap(ann, 0..2, ann, 4..7, false);
    assert_eq!(context.list(ann), [e, f, g, c, d, a, b, h]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.entity(f, &[Some(ann)]);
    context.entity(g, &[Some(ann)]);
    context.entity(h, &[Some(ann)]);
    context.index(a, &[Some(0), Some(5)]);
    context.index(b, &[Some(1), Some(6)]);
    context.index(c, &[Some(2), Some(3)]);
    context.index(d, &[Some(3), Some(4)]);
    context.index(e, &[Some(4), Some(0)]);
    context.index(f, &[Some(5), Some(1)]);
    context.index(g, &[Some(6), Some(2)]);
    context.index(h, &[Some(7)]);
    context.previous(a, &[Some(d)]);
    context.previous(b, &[Some(a)]);
    context.previous(c, &[Some(b), Some(g)]);
    context.previous(d, &[Some(c)]);
    context.previous(e, &[Some(d), None]);
    context.previous(f, &[Some(e)]);
    context.previous(g, &[Some(f)]);
    context.previous(h, &[Some(g), Some(b)]);
    context.next(a, &[Some(b)]);
    context.next(b, &[Some(c), Some(h)]);
    context.next(c, &[Some(d)]);
    context.next(d, &[Some(e), Some(a)]);
    context.next(e, &[Some(f)]);
    context.next(f, &[Some(g)]);
    context.next(g, &[Some(h), Some(c)]);
    context.next(h, &[]);
}

#[test]
fn swap_sub_lists_same_size_same_entity() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let f = Value("F");
    let g = Value("G");
    let h = Value("H");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e, f, g, h]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_swap(ann, 0..2, ann, 4..6, false);
    assert_eq!(context.list(ann), [e, f, c, d, a, b, g, h]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.entity(f, &[Some(ann)]);
    context.entity(g, &[Some(ann)]);
    context.entity(h, &[Some(ann)]);
    context.index(a, &[Some(0), Some(4)]);
    context.index(b, &[Some(1), Some(5)]);
    context.index(c, &[Some(2)]);
    context.index(d, &[Some(3)]);
    context.index(e, &[Some(4), Some(0)]);
    context.index(f, &[Some(5), Some(1)]);
    context.index(g, &[Some(6)]);
    context.index(h, &[Some(7)]);
    context.previous(a, &[Some(d)]);
    context.previous(b, &[Some(a)]);
    context.previous(c, &[Some(b), Some(f)]);
    context.previous(d, &[Some(c)]);
    context.previous(e, &[Some(d), None]);
    context.previous(f, &[Some(e)]);
    context.previous(g, &[Some(f), Some(b)]);
    context.previous(h, &[Some(g)]);
    context.next(a, &[Some(b)]);
    context.next(b, &[Some(c), Some(g)]);
    context.next(c, &[Some(d)]);
    context.next(d, &[Some(e), Some(a)]);
    context.next(e, &[Some(f)]);
    context.next(f, &[Some(g), Some(c)]);
    context.next(g, &[Some(h)]);
    context.next(h, &[]);
}

#[test]
fn swap_sub_lists_same_entity_reversing() {
    let s = schema();
    let mut rows = vec![];
    let a = Value("A");
    let b = Value("B");
    let c = Value("C");
    let d = Value("D");
    let e = Value("E");
    let ann = Owner("Ann");
    rows.push((ann, vec![a, b, c, d, e]));
    let mut context = Fixture::new(&s, rows, vec![]);
    context.sub_swap(ann, 3..5, ann, 0..2, true);
    assert_eq!(context.list(ann), [e, d, c, b, a]);
    context.entity(a, &[Some(ann)]);
    context.entity(b, &[Some(ann)]);
    context.entity(c, &[Some(ann)]);
    context.entity(d, &[Some(ann)]);
    context.entity(e, &[Some(ann)]);
    context.index(a, &[Some(0), Some(4)]);
    context.index(b, &[Some(1), Some(3)]);
    context.index(c, &[Some(2)]);
    context.index(d, &[Some(3), Some(1)]);
    context.index(e, &[Some(4), Some(0)]);
    context.previous(a, &[Some(b)]);
    context.previous(b, &[Some(a), Some(c)]);
    context.previous(c, &[Some(b), Some(d)]);
    context.previous(d, &[Some(c), Some(e)]);
    context.previous(e, &[Some(d), None]);
    context.next(a, &[Some(b), None]);
    context.next(b, &[Some(c), Some(a)]);
    context.next(c, &[Some(d), Some(b)]);
    context.next(d, &[Some(e), Some(c)]);
    context.next(e, &[Some(d)]);
}
