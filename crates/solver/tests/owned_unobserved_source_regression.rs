//! Original retained planning-row ownership controls; no upstream credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn movement() -> ListMove<i32> {
    ListMove::Assign {
        value: 1,
        destination: ListPosition {
            entity: 0,
            index: 1,
        },
    }
}
#[test]
fn list_value_rows_survive_unobserved_observed_and_large_source_trials() {
    for count in [3, 257] {
        for observed in [false, true] {
            let p = ListProblem::new((0..count).collect(), vec![ListEntity::new(vec![0])])
                .unwrap()
                .allowing_unassigned();
            let panic = Cell::new(false);
            let calls = Cell::new(0);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = ListStreamSources::new(&mut b);
            let inspect = s.clone();
            b.score("entity", s.entities.port(), |(_, list)| {
                calls.set(calls.get() + 1);
                assert!(!panic.get(), "injected");
                SimpleScore::new(list.len() as i64)
            })
            .unwrap();
            if observed {
                b.score("value", s.values.port(), |row| {
                    SimpleScore::new(i64::from(row.position.is_some()))
                })
                .unwrap();
            }
            let mut d =
                ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
            let held = d.graph().source_rows(&inspect.values).unwrap().clone();
            calls.set(0);
            {
                let trial = d.apply(&movement()).unwrap();
                let rows = trial.graph().source_rows(&inspect.values).unwrap();
                assert_eq!(
                    rows[&1].position,
                    Some(ListPosition {
                        entity: 0,
                        index: 1
                    })
                );
                assert_eq!(rows[&0].next, Some(1));
                assert_eq!(rows[&1].previous, Some(0));
                assert_eq!(
                    trial.score(),
                    SimpleScore::new(if observed { 4 } else { 2 })
                );
            }
            assert_eq!(calls.get(), 1);
            assert_eq!(d.graph().source_rows(&inspect.values).unwrap(), &held);
            assert!(std::ptr::eq(
                &d.graph().source_rows(&inspect.values).unwrap()[&0],
                &held[&0]
            ));
            panic.set(true);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _trial = d.apply(&movement()).unwrap();
                }))
                .is_err()
            );
            panic.set(false);
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.graph().source_rows(&inspect.values).unwrap(), &held);
            d.apply(&movement()).unwrap().commit();
            assert_eq!(
                d.graph().source_rows(&inspect.values).unwrap()[&1].position,
                Some(ListPosition {
                    entity: 0,
                    index: 1
                })
            );
            d.replace_assignment(p.initial_assignment())
                .unwrap()
                .commit();
            assert_eq!(d.graph().source_rows(&inspect.values).unwrap(), &held);
            assert_eq!(held[&1].position, None);
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn mixed_unobserved_values_are_retained_across_atomic_moves_and_full_replacement() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0])])
            .unwrap()
            .allowing_unassigned(),
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = MixedStreamSources::new(&mut b);
    let inspect = s.clone();
    b.score("basic", s.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    b.score("list", s.lists.entities.port(), |(_, v)| {
        SimpleScore::new(v.len() as i64)
    })
    .unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let held = d
        .graph()
        .source_rows(&inspect.lists.values)
        .unwrap()
        .clone();
    let m = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: movement(),
    };
    {
        let trial = d.apply(&m).unwrap();
        assert_eq!(trial.score(), SimpleScore::new(3));
        assert_eq!(
            trial.graph().source_rows(&inspect.lists.values).unwrap()[&1].previous,
            Some(0)
        );
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.graph().source_rows(&inspect.lists.values).unwrap(), &held);
    d.apply(&m).unwrap().commit();
    let committed = d.assignment().clone();
    {
        let _replacement = d.replace_assignment(p.initial_assignment()).unwrap();
    }
    assert_eq!(d.assignment(), &committed);
    assert_eq!(
        d.graph().source_rows(&inspect.lists.values).unwrap()[&0].next,
        Some(1)
    );
    d.replace_assignment(p.initial_assignment())
        .unwrap()
        .commit();
    assert_eq!(d.graph().source_rows(&inspect.lists.values).unwrap(), &held);
    assert_eq!(held[&1].position, None);
    d.assert_consistent().unwrap();
}
