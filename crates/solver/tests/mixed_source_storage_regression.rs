//! Original mixed source lifecycle regressions; no upstream source-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

type Sources = MixedStreamSources<i32, i32>;
type State = MixedAssignment<i32, i32>;
fn assert_rows(
    graph: &StreamGraph<'_, SimpleScore>,
    sources: &Sources,
    problem: &MixedProblem<i32, i32>,
    state: &State,
) {
    let basics = graph.source_rows(&sources.basic).unwrap();
    let owners = graph.source_rows(&sources.lists.entities).unwrap();
    let values = graph.source_rows(&sources.lists.values).unwrap();
    assert_eq!(basics.len(), state.basic().values().len());
    assert_eq!(owners.len(), state.lists().lists().len());
    assert_eq!(values.len(), problem.lists().values().len());
    for (i, value) in state.basic().values().iter().enumerate() {
        assert_eq!(basics[&i], (i, *value));
    }
    for (i, list) in state.lists().lists().iter().enumerate() {
        assert_eq!(&owners[&i], &(i, list.clone()));
    }
    for (i, &value) in problem.lists().values().iter().enumerate() {
        let position = state.lists().location(&value);
        let (previous, next) = position.map_or((None, None), |p| {
            let list = &state.lists().lists()[p.entity];
            (
                p.index.checked_sub(1).map(|j| list[j]),
                list.get(p.index + 1).copied(),
            )
        });
        assert_eq!(
            values[&i],
            ListValueRow {
                range_index: i,
                value,
                position,
                previous,
                next
            }
        );
    }
}
fn joint(variable: usize, value: i32, lists: ListMove<i32>) -> MixedMove<i32, i32> {
    MixedMove {
        basic: Move::Change {
            variable,
            value: Some(value),
        },
        lists,
    }
}
#[test]
fn source_boundary_preserves_all_unobserved_rows_snapshots_and_payload_identity() {
    for count in [256, 257] {
        let variables = (0..count)
            .map(|i| {
                let variable = Variable::new(vec![0, 1]).allowing_unassigned();
                if i == 0 {
                    variable
                } else {
                    variable.with_initial(0)
                }
            })
            .collect();
        let values: Vec<_> = (0..count as i32).collect();
        let entities = values
            .iter()
            .enumerate()
            .map(|(i, &v)| ListEntity::new(if i + 1 == count { vec![] } else { vec![v] }))
            .collect();
        let problem = MixedProblem::new(
            FiniteProblem::new(variables).unwrap(),
            ListProblem::new(values, entities)
                .unwrap()
                .allowing_unassigned(),
        );
        // All planning sources are intentionally unobserved by graph terminals.
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let sources = Sources::new(&mut builder);
        let inspect = sources.clone();
        let mut director = MixedStreamScoreDirector::new(
            &problem,
            builder.build(),
            sources,
            problem.initial_assignment(),
        )
        .unwrap();
        let original = director.assignment().clone();
        let before_basic = director
            .graph()
            .source_rows(&inspect.basic)
            .unwrap()
            .clone();
        let before_owners = director
            .graph()
            .source_rows(&inspect.lists.entities)
            .unwrap()
            .clone();
        let before_values = director
            .graph()
            .source_rows(&inspect.lists.values)
            .unwrap()
            .clone();
        let movement = joint(
            0,
            1,
            ListMove::Composite(vec![
                ListMove::Change {
                    source: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                    destination: ListPosition {
                        entity: 1,
                        index: 1,
                    },
                },
                ListMove::Assign {
                    value: (count - 1) as i32,
                    destination: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                },
            ]),
        );
        let trial = director.apply(&movement).unwrap();
        assert_eq!(trial.constraint_evaluations(), 0);
        assert_rows(trial.graph(), &inspect, &problem, trial.assignment());
        let candidate_values = trial
            .graph()
            .source_rows(&inspect.lists.values)
            .unwrap()
            .clone();
        assert_eq!(candidate_values[&0].previous, Some(1));
        assert!(std::ptr::eq(
            &before_basic[&2],
            &trial.graph().source_rows(&inspect.basic).unwrap()[&2]
        ));
        assert!(std::ptr::eq(
            &before_owners[&2],
            &trial.graph().source_rows(&inspect.lists.entities).unwrap()[&2]
        ));
        assert!(std::ptr::eq(
            &before_values[&2],
            &trial.graph().source_rows(&inspect.lists.values).unwrap()[&2]
        ));
        drop(trial);
        assert_eq!(director.assignment(), &original);
        assert_rows(director.graph(), &inspect, &problem, director.assignment());
        assert_eq!(before_basic[&0], (0, None));
        assert!(before_values[&(count - 1)].position.is_none());
        assert_eq!(
            candidate_values[&0].position,
            Some(ListPosition {
                entity: 1,
                index: 1
            })
        );
        assert!(std::ptr::eq(
            &before_values[&0],
            &director.graph().source_rows(&inspect.lists.values).unwrap()[&0]
        ));
        director.apply(&movement).unwrap().commit();
        assert_rows(director.graph(), &inspect, &problem, director.assignment());
        assert_eq!(before_owners[&0].1, vec![0]);
        assert!(before_values[&(count - 1)].position.is_none());
        director.assert_consistent().unwrap();
    }
}

#[test]
fn joint_callbacks_and_nested_edit_sessions_preserve_exact_work_and_shadow_rows() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![1, 3]).with_initial(1),
            Variable::new(vec![0, 3]).allowing_unassigned(),
        ])
        .unwrap(),
        ListProblem::new(
            vec![10, 20, 30],
            vec![ListEntity::new(vec![10, 20]), ListEntity::new(vec![])],
        )
        .unwrap()
        .allowing_unassigned(),
    );
    let calls = Cell::new(0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = Sources::new(&mut builder);
    let inspect = sources.clone();
    builder
        .score("basic", sources.basic.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(i64::from(v.unwrap_or(0)))
        })
        .unwrap();
    builder
        .score("owners", sources.lists.entities.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(10 * v.len() as i64)
        })
        .unwrap();
    builder
        .score("neighbors", sources.lists.values.port(), |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(if v.position.is_some() {
                1 + 2 * i64::from(v.previous.is_some()) + 4 * i64::from(v.next.is_some())
            } else {
                0
            })
        })
        .unwrap();
    let mut director = MixedStreamScoreDirector::new(
        &problem,
        builder.build(),
        sources,
        problem.initial_assignment(),
    )
    .unwrap();
    assert_eq!(calls.get(), 7);
    assert_eq!(director.score(), SimpleScore::new(29));
    let original = director.assignment().clone();
    let held = director
        .graph()
        .source_rows(&inspect.lists.values)
        .unwrap()
        .clone();
    let first = joint(
        1,
        3,
        ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 1,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    );
    let before = calls.get();
    let trial = director.apply(&first).unwrap();
    assert_eq!(trial.constraint_evaluations(), 5);
    assert_eq!(calls.get() - before, 5);
    assert_eq!(trial.score(), SimpleScore::new(26));
    drop(trial);
    assert_eq!(calls.get() - before, 5);
    assert_eq!(director.assignment(), &original);
    {
        let mut outer = director.edit_session().unwrap();
        outer.apply(&first).unwrap();
        let checkpoint = outer.assignment().clone();
        let before_inner = calls.get();
        {
            let mut inner = outer.nested().unwrap();
            inner
                .apply(&MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Assign {
                        value: 30,
                        destination: ListPosition {
                            entity: 0,
                            index: 1,
                        },
                    },
                })
                .unwrap();
            assert_eq!(calls.get() - before_inner, 3);
            inner.commit();
        }
        assert_ne!(outer.assignment(), &checkpoint);
        assert_rows(
            outer.director().graph(),
            &inspect,
            &problem,
            outer.assignment(),
        );
        // An inner commit is still subject to the outer rollback.
    }
    assert_eq!(director.assignment(), &original);
    assert_eq!(director.score(), SimpleScore::new(29));
    let before = calls.get();
    {
        let mut edits = director.edit_session().unwrap();
        edits.apply(&first).unwrap();
        edits.commit();
    }
    assert_eq!(calls.get() - before, 5);
    assert_eq!(director.score(), SimpleScore::new(26));
    assert_rows(director.graph(), &inspect, &problem, director.assignment());
    assert_eq!(held[&0].next, Some(20));
    assert!(held[&2].position.is_none());
    director.assert_consistent().unwrap();
}

#[test]
fn clone_and_diagnostic_panics_restore_original_payloads_and_match_large_source_fallback() {
    #[derive(Debug, Default)]
    struct Flags {
        armed: Cell<bool>,
        clones: Cell<usize>,
        drops: Cell<usize>,
        at_panic: Cell<usize>,
    }
    #[derive(Debug)]
    struct Value {
        code: i32,
        flags: Rc<Flags>,
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.code == other.code
        }
    }
    impl Eq for Value {}
    impl Clone for Value {
        fn clone(&self) -> Self {
            let n = self.flags.clones.get() + 1;
            self.flags.clones.set(n);
            if self.flags.armed.get() {
                self.flags.at_panic.set(n);
                panic!("user clone after delivered source callback");
            }
            Self {
                code: self.code,
                flags: self.flags.clone(),
            }
        }
    }
    impl Drop for Value {
        fn drop(&mut self) {
            self.flags.drops.set(self.flags.drops.get() + 1);
        }
    }
    let mut observed = Vec::new();
    for count in [2, 257] {
        let flags = Rc::new(Flags::default());
        let value = |code| Value {
            code,
            flags: flags.clone(),
        };
        let variables = (0..count)
            .map(|i| {
                let v = Variable::new(vec![value(0), value(1)]).allowing_unassigned();
                if i < 2 { v.with_initial(value(0)) } else { v }
            })
            .collect();
        let problem = MixedProblem::new(
            FiniteProblem::new(variables).unwrap(),
            ListProblem::<i32>::new(vec![], vec![]).unwrap(),
        );
        let calls = Cell::new(0);
        let diagnostic = Cell::new(false);
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let sources = MixedStreamSources::<Value, i32>::new(&mut builder);
        let inspect = sources.clone();
        builder
            .score("value", sources.basic.port(), |(i, v)| {
                calls.set(calls.get() + 1);
                assert!(!diagnostic.get(), "diagnostic panic");
                if *i == 0 && v.as_ref().is_some_and(|v| v.code == 1) {
                    flags.armed.set(true);
                }
                SimpleScore::new(v.as_ref().map_or(0, |v| i64::from(v.code)))
            })
            .unwrap();
        let mut director = MixedStreamScoreDirector::new(
            &problem,
            builder.build(),
            sources,
            problem.initial_assignment(),
        )
        .unwrap();
        flags.clones.set(0);
        flags.drops.set(0);
        let held = director
            .graph()
            .source_rows(&inspect.basic)
            .unwrap()
            .clone();
        assert_eq!(
            flags.clones.get(),
            0,
            "source inspection and snapshot do not clone user payloads"
        );
        assert_eq!(
            flags.drops.get(),
            0,
            "inspection does not destroy user payloads"
        );
        let movement = MixedMove {
            basic: Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: Some(value(1)),
                },
                Move::Change {
                    variable: 1,
                    value: Some(value(1)),
                },
            ]),
            lists: ListMove::Composite(vec![]),
        };
        let before = calls.get();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = director.apply(&movement);
            }))
            .is_err()
        );
        assert_eq!(
            calls.get() - before,
            1,
            "panic occurs after first actual source callback"
        );
        assert_eq!(
            flags.clones.get(),
            flags.at_panic.get(),
            "rollback must not invoke user Clone"
        );
        let clone_work = flags.clones.get();
        let drop_work = flags.drops.get();
        flags.armed.set(false);
        assert_eq!(director.score(), SimpleScore::ZERO);
        let current = director.graph().source_rows(&inspect.basic).unwrap();
        for i in 0..2 {
            assert_eq!(current[&i].1.as_ref().unwrap().code, 0);
            assert!(std::ptr::eq(&held[&i], &current[&i]));
        }
        assert_eq!(flags.clones.get(), clone_work);
        assert_eq!(flags.drops.get(), drop_work);
        diagnostic.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| director.assert_consistent())).is_err());
        diagnostic.set(false);
        assert_eq!(director.score(), SimpleScore::ZERO);
        let before = calls.get();
        director
            .apply(&MixedMove {
                basic: Move::Change {
                    variable: 1,
                    value: Some(value(1)),
                },
                lists: ListMove::Composite(vec![]),
            })
            .unwrap()
            .commit();
        assert_eq!(calls.get() - before, 1);
        assert_eq!(director.score(), SimpleScore::new(1));
        assert_eq!(held[&1].1.as_ref().unwrap().code, 0);
        director.assert_consistent().unwrap();
        observed.push((clone_work, flags.at_panic.get(), drop_work));
    }
    assert_eq!(
        observed[0], observed[1],
        "fixed and ordinary source routes preserve the exact Clone/Drop/panic prefix"
    );
}

#[test]
fn retained_forager_best_and_inactive_fact_activation_restore_live_source_views() {
    let problem = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap(),
        ListProblem::new(
            vec![10, 20],
            vec![ListEntity::new(vec![10]), ListEntity::new(vec![20])],
        )
        .unwrap(),
    );
    let dead_calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let sources = Sources::new(&mut builder);
    let inspect = sources.clone();
    builder
        .score("basic", sources.basic.port(), |(_, v)| {
            SimpleScore::new(3 * i64::from(v.unwrap_or(0)))
        })
        .unwrap();
    builder
        .score("placement", sources.lists.values.port(), |v| {
            SimpleScore::new(
                if v.value == 20 && v.position.is_some_and(|p| p.entity == 0) {
                    10
                } else {
                    0
                },
            )
        })
        .unwrap();
    let gate = builder.source::<usize, i32>();
    let mapped = builder
        .map(sources.lists.values.port(), |v| {
            dead_calls.set(dead_calls.get() + 1);
            assert!(!fail.get(), "activation panic");
            v.range_index
        })
        .unwrap();
    let joined = builder
        .join_equal(&mapped, gate.port(), |_| (), |_| ())
        .unwrap();
    builder
        .score("gated", &joined, |_| SimpleScore::new(100))
        .unwrap();
    let mut director = MixedStreamScoreDirector::new(
        &problem,
        builder.build(),
        sources,
        problem.initial_assignment(),
    )
    .unwrap();
    let old = director
        .graph()
        .source_rows(&inspect.lists.values)
        .unwrap()
        .clone();
    let inactive = director.graph().activity();
    let result = director
        .run_local_search(MixedLocalSearchConfig {
            joint_moves: true,
            search: LocalSearchConfig {
                max_steps: 4,
                max_evaluations: 500,
                selection: MoveSelection::BestAccepted,
                acceptance: Acceptance::StrictImprovement,
                ..Default::default()
            },
        })
        .unwrap();
    assert_eq!(result.best.as_ref().unwrap().score, SimpleScore::new(16));
    assert_eq!(director.assignment(), &result.best.unwrap().assignment);
    assert_eq!(director.score(), SimpleScore::new(16));
    assert_rows(director.graph(), &inspect, &problem, director.assignment());
    assert_eq!(dead_calls.get(), 0);
    assert!(!director.graph().activity().get(&mapped).unwrap().active);
    assert_eq!(
        old[&1].position,
        Some(ListPosition {
            entity: 1,
            index: 0
        })
    );
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = director.edit_facts();
            tx.apply(&gate, TupleChange::put(0, 1)).unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(director.score(), SimpleScore::new(16));
    assert!(director.graph().source_rows(&gate).unwrap().is_empty());
    assert!(!director.graph().activity().get(&mapped).unwrap().active);
    assert_rows(director.graph(), &inspect, &problem, director.assignment());
    let before = dead_calls.get();
    let mut tx = director.edit_facts();
    tx.apply(&gate, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(dead_calls.get() - before, 2);
    assert_eq!(director.score(), SimpleScore::new(216));
    assert!(!inactive.get(&mapped).unwrap().active);
    assert!(director.graph().activity().get(&mapped).unwrap().active);
    director.assert_consistent().unwrap();
}
