//! Original basic source lifecycle regressions; no upstream source-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn change(variable: usize, value: Option<i32>) -> Move<i32> {
    Move::Change { variable, value }
}
#[test]
fn complete_and_optional_rows_survive_trials_nested_sessions_and_owned_snapshots() {
    for count in [2, 256, 257] {
        for partial in [false, true] {
            let problem = FiniteProblem::new(
                (0..count)
                    .map(|i| {
                        let variable = Variable::new(vec![0, 1]).allowing_unassigned();
                        if i == 0 && partial {
                            variable
                        } else {
                            variable.with_initial(0)
                        }
                    })
                    .collect(),
            )
            .unwrap();
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("weighted", source.port(), |(i, v)| {
                SimpleScore::new((*i as i64 + 1) * i64::from(v.unwrap_or(0)))
            })
            .unwrap();
            let inspect = source.clone();
            let original = problem.initial_assignment();
            let mut d =
                StreamScoreDirector::new(&problem, b.build(), source, original.clone()).unwrap();
            let before = d.graph().source_rows(&inspect).unwrap().clone();
            assert_eq!(before.len(), count);
            for (i, v) in original.values().iter().enumerate() {
                assert_eq!(before[&i], (i, *v));
            }
            {
                let trial = d.apply(&change(0, Some(1))).unwrap();
                assert_eq!(trial.score(), SimpleScore::new(1));
                assert_eq!(
                    trial.graph().source_rows(&inspect).unwrap()[&0],
                    (0, Some(1))
                );
            }
            assert_eq!(d.assignment(), &original);
            assert_eq!(d.graph().source_rows(&inspect).unwrap(), &before);
            d.apply(&change(0, Some(1))).unwrap().commit();
            let committed = d.assignment().clone();
            let inner_snapshot;
            {
                let mut outer = d.edit_session().unwrap();
                outer.apply(&Move::Swap { left: 0, right: 1 }).unwrap();
                assert_eq!(outer.score(), SimpleScore::new(2));
                {
                    let mut inner = outer.nested().unwrap();
                    inner.apply(&change(1, None)).unwrap();
                    assert_eq!(inner.score(), SimpleScore::ZERO);
                    inner_snapshot = inner
                        .director()
                        .graph()
                        .source_rows(&inspect)
                        .unwrap()
                        .clone();
                    inner.commit();
                }
                outer.assert_consistent().unwrap();
                outer.replace_assignment(original.clone()).unwrap();
                assert_eq!(outer.score(), SimpleScore::ZERO);
            }
            assert_eq!(d.assignment(), &committed);
            assert_eq!(d.score(), SimpleScore::new(1));
            assert_eq!(inner_snapshot[&0], (0, Some(0)));
            assert_eq!(inner_snapshot[&1], (1, None));
            assert_eq!(before[&0], (0, if partial { None } else { Some(0) }));
            d.replace_assignment(original.clone()).unwrap().commit();
            assert_eq!(d.assignment(), &original);
            assert_eq!(d.graph().source_rows(&inspect).unwrap(), &before);
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn planning_fact_edits_reject_atomically_but_external_fact_ids_remain_mutable() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).allowing_unassigned(),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let inspect = source.clone();
    let facts = b.source::<i32, (i32, i64)>();
    let joined = b
        .join_equal(source.port(), facts.port(), |(_, v)| *v, |(v, _)| Some(*v))
        .unwrap();
    b.score("cost", &joined, |(_, (_, cost))| SimpleScore::new(-cost))
        .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    {
        let mut tx = d.edit_facts();
        tx.apply(&facts, TupleChange::put(0, (0, 3))).unwrap();
        tx.apply(&facts, TupleChange::put(1, (1, 7))).unwrap();
        tx.commit().unwrap();
    }
    let before = d.graph().source_rows(&inspect).unwrap().clone();
    let old_facts = d.graph().source_rows(&facts).unwrap().clone();
    {
        let trial = d.apply(&change(0, Some(1))).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(-7));
    }
    assert_eq!(d.score(), SimpleScore::new(-3));
    for batch in [false, true] {
        let mut tx = d.edit_facts();
        tx.apply(&facts, TupleChange::put(1, (1, 11))).unwrap();
        let error = if batch {
            let mut changes = StreamBatch::new();
            changes.push(&inspect, TupleChange::retract(0)).unwrap();
            tx.apply_batch(changes)
        } else {
            tx.apply(&inspect, TupleChange::put(0, (0, Some(1))))
        };
        assert_eq!(error, Err(GraphError::ProtectedPlanningSource));
        assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
        assert_eq!(d.graph().source_rows(&facts).unwrap(), &old_facts);
        assert_eq!(d.graph().source_rows(&inspect).unwrap(), &before);
    }
    {
        let mut tx = d.edit_facts();
        tx.apply(&facts, TupleChange::put(0, (0, 5))).unwrap();
        tx.apply(&facts, TupleChange::put(1000, (1000, 99)))
            .unwrap();
        tx.assert_consistent().unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(d.score(), SimpleScore::new(-5));
    assert_eq!(old_facts[&0], (0, 3));
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let other = foreign.source::<i32, (i32, i64)>();
    let mut tx = d.edit_facts();
    assert_eq!(
        tx.apply(&other, TupleChange::put(0, (0, 9))),
        Err(GraphError::ForeignPort)
    );
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(d.score(), SimpleScore::new(-5));
    d.assert_consistent().unwrap();
}
#[test]
fn callback_unwind_restores_an_earlier_source_write_and_retry_is_fresh() {
    let fail = Cell::new(false);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let inspect = source.clone();
    b.score("values", source.port(), |(id, v)| {
        assert!(
            !(fail.get() && *id == 1 && *v == Some(1)),
            "injected callback"
        );
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let old = d.graph().source_rows(&inspect).unwrap().clone();
    let assignment = d.assignment().clone();
    let movement = Move::Composite(vec![change(0, Some(1)), change(1, Some(1))]);
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&movement);
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(d.assignment(), &assignment);
    assert_eq!(d.graph().source_rows(&inspect).unwrap(), &old);
    assert_eq!(d.score(), SimpleScore::ZERO);
    d.assert_consistent().unwrap();
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(2));
    assert_eq!(old[&0], (0, Some(0)));
    assert_eq!(old[&1], (1, Some(0)));
    d.assert_consistent().unwrap();
}

#[derive(Debug)]
struct Value {
    id: i32,
    remaining: Rc<Cell<Option<usize>>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        if let Some(left) = self.remaining.get() {
            assert!(left > 0, "injected clone");
            self.remaining.set(Some(left - 1));
        }
        Self {
            id: self.id,
            remaining: self.remaining.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn clone_failures_leave_source_snapshots_assignment_and_director_reusable() {
    let mut failures = 0;
    for fail_at in 0..20 {
        let remaining = Rc::new(Cell::new(None));
        let value = |id| Value {
            id,
            remaining: remaining.clone(),
        };
        let p = FiniteProblem::new(vec![
            Variable::new(vec![value(0), value(1)])
                .with_initial(value(0));
            2
        ])
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<Value>)>();
        let inspect = source.clone();
        b.score("values", source.port(), |(_, v)| {
            SimpleScore::new(i64::from(v.as_ref().unwrap().id))
        })
        .unwrap();
        let mut d =
            StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
        let old = d.graph().source_rows(&inspect).unwrap().clone();
        let assignment = d.assignment().clone();
        let movement = Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(value(1)),
            },
            Move::Change {
                variable: 1,
                value: Some(value(1)),
            },
        ]);
        remaining.set(Some(fail_at));
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            drop(d.apply(&movement).unwrap());
        }));
        remaining.set(None);
        failures += usize::from(outcome.is_err());
        assert_eq!(d.assignment(), &assignment);
        assert_eq!(d.graph().source_rows(&inspect).unwrap(), &old);
        assert_eq!(d.score(), SimpleScore::ZERO);
        d.assert_consistent().unwrap();
        d.apply(&movement).unwrap().commit();
        assert_eq!(d.score(), SimpleScore::new(2));
        assert_eq!(old[&0].1.as_ref().unwrap().id, 0);
    }
    assert!(failures > 0);
}
