//! Original planning-filter storage contracts; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn problem(count: usize) -> FiniteProblem<i64> {
    FiniteProblem::new(
        (0..count)
            .map(|i| {
                let v = Variable::new(vec![1, 2, 3, 4]).allowing_unassigned();
                if i == 0 { v.with_initial(1) } else { v }
            })
            .collect(),
    )
    .unwrap()
}
fn change(variable: usize, value: Option<i64>) -> Move<i64> {
    Move::Change { variable, value }
}
fn expected(a: &Assignment<i64>) -> BTreeMap<usize, (usize, i64)> {
    a.values()
        .iter()
        .enumerate()
        .filter_map(|(id, v)| v.filter(|v| v % 2 == 0).map(|v| (id, (id, v))))
        .collect()
}
fn check(
    g: &StreamGraph<'_, SimpleScore>,
    data: &StreamDataset<usize, (usize, i64)>,
    a: &Assignment<i64>,
) {
    let want = expected(a);
    assert_eq!(g.dataset_rows(data).unwrap(), &want);
    assert_eq!(
        g.score(),
        SimpleScore::new(want.values().map(|(id, v)| (*id as i64 + 1) * v).sum())
    );
}

#[test]
fn sparse_downstream_filters_accept_unseen_ids_and_restore_nested_saved_views() {
    for count in [2, 256, 257] {
        let p = problem(count);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let mapped = b.map(&assigned, |&(id, v)| (id, v.unwrap())).unwrap();
        let even = b.filter(&mapped, |(_, v)| v % 2 == 0).unwrap();
        let data = b.dataset(&even).unwrap();
        b.score("even", &even, |&(id, v)| {
            SimpleScore::new((id as i64 + 1) * v)
        })
        .unwrap();
        let original = p.initial_assignment();
        let mut d = StreamScoreDirector::new(&p, b.build(), source, original.clone()).unwrap();
        check(d.graph(), &data, d.assignment());
        let empty = d.graph().dataset_rows(&data).unwrap().clone();
        let movement = change(count - 1, Some(2));
        {
            let trial = d.apply(&movement).unwrap();
            check(trial.graph(), &data, trial.assignment());
            assert_eq!(
                trial.graph().dataset_rows(&data).unwrap()[&(count - 1)],
                (count - 1, 2)
            );
        }
        assert_eq!(d.assignment(), &original);
        d.apply(&movement).unwrap().commit();
        let committed = d.assignment().clone();
        let held = d.graph().dataset_rows(&data).unwrap().clone();
        {
            let mut outer = d.edit_session().unwrap();
            outer.apply(&change(0, Some(4))).unwrap();
            {
                let mut inner = outer.nested().unwrap();
                inner.apply(&change(count - 1, None)).unwrap();
                check(inner.director().graph(), &data, inner.assignment());
                inner.commit();
            }
            check(outer.director().graph(), &data, outer.assignment());
        }
        assert_eq!(d.assignment(), &committed);
        check(d.graph(), &data, d.assignment());
        {
            let replace = d.replace_assignment(original.clone()).unwrap();
            check(replace.graph(), &data, replace.assignment());
        }
        assert_eq!(d.assignment(), &committed);
        d.replace_assignment(original.clone()).unwrap().commit();
        check(d.graph(), &data, d.assignment());
        assert!(empty.is_empty());
        assert_eq!(held[&(count - 1)], (count - 1, 2));
        d.assert_consistent().unwrap();
    }
}

#[test]
fn group_key_namespaces_and_arbitrary_fact_ids_never_inherit_planning_bounds() {
    let p = problem(2);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
    let grouped = b
        .group_collect_batched(&assigned, |&(_, v)| {
            let v = v.unwrap();
            (1000 + v as usize, v)
        })
        .unwrap();
    let group_map = b.map(&grouped, |&(id, summary)| (id, summary.sum)).unwrap();
    let group_filter = b.filter(&group_map, |(_, sum)| *sum > 0).unwrap();
    let groups = b.dataset(&group_filter).unwrap();
    b.score("groups", &group_filter, |(_, sum)| SimpleScore::new(*sum))
        .unwrap();
    let facts = b.source::<usize, i64>();
    let positive = b.filter(facts.port(), |v| *v > 0).unwrap();
    let fact_map = b.map(&positive, |v| *v * 2).unwrap();
    let fact_filter = b.filter(&fact_map, |v| *v >= 10).unwrap();
    let fact_rows = b.dataset(&fact_filter).unwrap();
    b.score("facts", &fact_filter, |v| SimpleScore::new(*v))
        .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.graph().dataset_rows(&groups).unwrap()[&1001], (1001, 1));
    d.apply(&change(1, Some(4))).unwrap().commit();
    assert_eq!(d.graph().dataset_rows(&groups).unwrap()[&1004], (1004, 4));
    {
        let mut edit = d.edit_facts();
        edit.apply(&facts, TupleChange::put(1_000_000, 7)).unwrap();
        edit.apply(&facts, TupleChange::put(3_000_000, -3)).unwrap();
        assert_eq!(edit.score(), Ok(SimpleScore::new(19)));
        edit.commit().unwrap();
    }
    let held = d.graph().dataset_rows(&fact_rows).unwrap().clone();
    {
        let mut edit = d.edit_facts();
        edit.apply(&facts, TupleChange::put(3_000_000, 9)).unwrap();
        edit.apply(&facts, TupleChange::retract(1_000_000)).unwrap();
        assert_eq!(edit.score(), Ok(SimpleScore::new(23)));
    }
    assert_eq!(d.graph().dataset_rows(&fact_rows).unwrap(), &held);
    assert_eq!(d.score(), SimpleScore::new(19));
    d.assert_consistent().unwrap();
}

#[derive(Debug)]
struct Value {
    code: i32,
    fail_clone: Rc<Cell<bool>>,
    clone_panics: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        if self.fail_clone.replace(false) {
            self.clone_panics.set(self.clone_panics.get() + 1);
            panic!("clone after accepting predicate");
        }
        Self {
            code: self.code,
            fail_clone: self.fail_clone.clone(),
            clone_panics: self.clone_panics.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, rhs: &Self) -> bool {
        self.code == rhs.code
    }
}
impl Eq for Value {}

#[test]
fn accepting_predicate_and_subsequent_payload_clone_panics_restore_membership_and_retry() {
    for count in [2, 256, 257] {
        let mode = Cell::new(0);
        let fail_clone = Rc::new(Cell::new(false));
        let clone_panics = Rc::new(Cell::new(0));
        let value = |code| Value {
            code,
            fail_clone: fail_clone.clone(),
            clone_panics: clone_panics.clone(),
        };
        let p = FiniteProblem::new(
            (0..count)
                .map(|i| {
                    let v = Variable::new(vec![value(0), value(1)]).allowing_unassigned();
                    if i == 0 { v.with_initial(value(0)) } else { v }
                })
                .collect(),
        )
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<Value>)>();
        let accepted = b
            .filter(source.port(), |(_, v)| {
                if v.as_ref().is_some_and(|v| v.code == 1) {
                    assert_ne!(mode.get(), 1, "predicate failure");
                    if mode.get() == 2 {
                        fail_clone.set(true);
                    }
                }
                v.is_some()
            })
            .unwrap();
        let mapped = b
            .map(&accepted, |(id, v)| (*id, v.as_ref().unwrap().code))
            .unwrap();
        let positive = b.filter(&mapped, |(_, v)| *v > 0).unwrap();
        let data = b.dataset(&positive).unwrap();
        b.score("positive", &positive, |(_, v)| {
            SimpleScore::new(i64::from(*v))
        })
        .unwrap();
        let original = p.initial_assignment();
        let mut d = StreamScoreDirector::new(&p, b.build(), source, original.clone()).unwrap();
        let held = d.graph().dataset_rows(&data).unwrap().clone();
        let movement = Move::Change {
            variable: count - 1,
            value: Some(value(1)),
        };
        for fail in [1, 2] {
            mode.set(fail);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = d.apply(&movement).unwrap();
                }))
                .is_err()
            );
            mode.set(0);
            fail_clone.set(false);
            assert_eq!(d.assignment(), &original);
            assert_eq!(d.score(), SimpleScore::ZERO);
            assert_eq!(d.graph().dataset_rows(&data).unwrap(), &held);
            {
                let retry = d.apply(&movement).unwrap();
                assert_eq!(retry.score(), SimpleScore::new(1));
                assert_eq!(
                    retry.graph().dataset_rows(&data).unwrap()[&(count - 1)],
                    (count - 1, 1)
                );
            }
            assert_eq!(d.assignment(), &original);
            d.assert_consistent().unwrap();
        }
        assert_eq!(clone_panics.get(), 1);
        d.apply(&movement).unwrap().commit();
        assert_eq!(d.score(), SimpleScore::new(1));
        assert!(held.is_empty());
        d.assert_consistent().unwrap();
    }
}
