//! Original behavioral controls; no upstream method credit.
use std::{cell::Cell, collections::BTreeMap, rc::Rc};
use timetuck::*;
struct Solution;
struct Entity;
fn schema(optional: bool, list: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        if list {
            PlanningVariableDefinition::list::<i64>("value", optional)
        } else {
            PlanningVariableDefinition::basic::<i64>("value", optional)
        },
    ])])
    .unwrap()
}
#[test]
fn shared_range_is_explicit_verified_optional_and_bound_to_selected_rows() {
    let s = schema(true, false);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1i64, 2]).allowing_unassigned(),
        Variable::new(vec![9]).allowing_unassigned(),
        Variable::new(vec![1, 2]).allowing_unassigned(),
    ])
    .unwrap();
    let slots = SchemaVariableSlots {
        variable: s
            .genuine_entity::<Entity>()
            .unwrap()
            .genuine_variable("value")
            .unwrap(),
        slots: vec![2, 0],
    };
    let field = slots.bind_basic(&p).unwrap();
    assert_eq!(
        field.is_value_in_solution_range(Some(&1)),
        Err(FieldBindingError::EntityScopedRange)
    );
    let shared = field.declare_solution_range().unwrap();
    assert!(shared.is_value_in_solution_range(Some(&1)).unwrap());
    assert!(shared.is_value_in_solution_range(None).unwrap());
    assert!(!shared.is_value_in_solution_range(Some(&9)).unwrap());
    let bad = SchemaVariableSlots {
        variable: slots.variable,
        slots: vec![0, 1],
    }
    .bind_basic(&p)
    .unwrap();
    assert!(matches!(
        bad.declare_solution_range(),
        Err(FieldBindingError::DifferentEntityRanges)
    ));
    let empty = SchemaVariableSlots {
        variable: slots.variable,
        slots: vec![],
    }
    .bind_basic(&p)
    .unwrap();
    assert!(matches!(
        empty.declare_solution_range(),
        Err(FieldBindingError::RowOutOfBounds { row: 0, count: 0 })
    ));
}
#[test]
fn explicit_cross_list_edits_and_signed_replace_execute_on_both_scorers_and_rollback() {
    let p = ListProblem::new(
        vec![1i64, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3, 4])],
    )
    .unwrap()
    .allowing_unassigned();
    for operation in 0..3 {
        let mut edit = ListMoveBuilder::new(&p, &p.initial_assignment()).unwrap();
        let expected = match operation {
            0 => {
                edit.relocate_between_lists(
                    ListRange {
                        entity: 0,
                        start: 0,
                        end: 2,
                    },
                    ListPosition {
                        entity: 1,
                        index: 2,
                    },
                    true,
                )
                .unwrap();
                vec![vec![], vec![3, 4, 2, 1]]
            }
            1 => {
                edit.swap_between_lists(
                    ListRange {
                        entity: 0,
                        start: 0,
                        end: 1,
                    },
                    ListRange {
                        entity: 1,
                        start: 0,
                        end: 2,
                    },
                    true,
                )
                .unwrap();
                vec![vec![4, 3, 2], vec![1]]
            }
            _ => {
                assert_eq!(edit.replace_signed(0, 0, 1).unwrap(), 2);
                vec![vec![1], vec![3, 4]]
            }
        };
        assert_eq!(edit.assignment().lists(), expected);
        let prefix = edit.assignment().clone();
        let count = edit.steps().len();
        assert!(matches!(
            edit.replace_signed(0, -1, 0),
            Err(ListEditError::SignedIndex { index: -1, .. })
        ));
        assert!(edit.replace_signed(1, 0, 99).is_err());
        assert!(
            edit.relocate_between_lists(
                ListRange {
                    entity: 1,
                    start: 0,
                    end: 1
                },
                ListPosition {
                    entity: 1,
                    index: 0
                },
                false
            )
            .is_err()
        );
        assert!(
            edit.swap_between_lists(
                ListRange {
                    entity: 1,
                    start: 0,
                    end: 1
                },
                ListRange {
                    entity: 1,
                    start: 1,
                    end: 2
                },
                false
            )
            .is_err()
        );
        assert_eq!(edit.assignment(), &prefix);
        assert_eq!(edit.steps().len(), count);
        let movement = edit.finish();
        let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 4);
        let mut dense = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut b);
        let mut stream =
            ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
        {
            let trial = dense.apply(&movement).unwrap();
            assert_eq!(trial.assignment().lists(), expected);
        }
        {
            let trial = stream.apply(&movement).unwrap();
            assert_eq!(trial.assignment().lists(), expected);
        }
        assert_eq!(dense.assignment(), &p.initial_assignment());
        assert_eq!(stream.assignment(), dense.assignment());
        dense.assert_consistent().unwrap();
        stream.assert_consistent().unwrap();
    }
}
#[test]
fn phase_recording_discovers_receivers_shares_real_buffers_and_rejects_bad_transitions_atomically()
{
    let s = schema(false, true);
    let r = ListChangeRecorder::<usize, i64>::new(
        s.genuine_entity::<Entity>()
            .unwrap()
            .genuine_variable("value")
            .unwrap(),
    )
    .unwrap();
    let a = vec![vec![1], vec![2], vec![3]];
    let b = vec![vec![], vec![2, 1], vec![3]];
    r.record_phase_transition(&a, &b).unwrap();
    let first = r.recorded_before().unwrap();
    assert_eq!(first.len(), 2);
    let second = r.recorded_before().unwrap();
    assert!(Rc::ptr_eq(&first[1].old_values, &second[1].old_values));
    assert_eq!(
        r.record_phase_transition(&a, &b),
        Err(ListRecordingError::StateMismatch)
    );
    assert_eq!(
        r.record_phase_transition(&b, &b[..2]),
        Err(ListRecordingError::StateMismatch)
    );
    assert_eq!(r.recorded_before().unwrap(), first);
    r.before(2, 0, 1, &b[2]).unwrap();
    assert_eq!(
        r.record_phase_transition(&b, &a),
        Err(ListRecordingError::UnclosedBracket)
    );
    r.after(2, 0, 1, &b[2]).unwrap();
    let mut lists: BTreeMap<_, _> = b.into_iter().enumerate().collect();
    r.undo_changes(&mut lists, |_, _| {}).unwrap();
    assert_eq!(lists.values().cloned().collect::<Vec<_>>(), a);
    assert!(r.recorded_before().unwrap().is_empty());
    assert_eq!(first[0].old_values.as_ref(), &[1]);
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        limits: Default::default(),
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        })],
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: Default::default(),
        phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: 1,
                assert_scores: true,
                ..Default::default()
            },
            ..Default::default()
        })],
    }
}
#[test]
fn nested_contexts_count_actual_work_in_all_six_backends_without_closing_parent_or_losing_undo() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1, 2]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![7i64],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(1);
    bn.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    let mut ln = ListConstraintNetwork::new(2, 1);
    ln.add_constraint("owner", vec![ListScope::Value(0)], |v| {
        let ListFact::Value { element, .. } = &v[0] else {
            unreachable!()
        };
        SimpleScore::new(element.as_ref().unwrap().position.entity as i64)
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 2, 1);
    mn.add_constraint(
        "both",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |v| {
            let (MixedFact::Basic(value), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
            else {
                unreachable!()
            };
            SimpleScore::new(*value.unwrap() + element.as_ref().unwrap().position.entity as i64)
        },
    )
    .unwrap();
    let bm = Move::Change {
        variable: 0,
        value: Some(1),
    };
    let lm = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    let mm = MixedMove {
        basic: bm.clone(),
        lists: ListMove::Composite(vec![]),
    };
    macro_rules! check {
        ($director:expr, $movement:expr, $plan:expr) => {{
            let mut d = $director;
            let initial = d.assignment().clone();
            let initial_score = d.score();
            let seen = Cell::new(0u128);
            {
                let mut edits = d.edit_session().unwrap();
                edits.apply(&$movement).unwrap();
                let result = edits
                    .run_solver_in_context($plan, |child| {
                        assert!(child.director().is_some());
                        assert!(child.calculation_count() > 0);
                        seen.set(child.calculation_count());
                    })
                    .unwrap();
                assert_eq!(seen.get(), u128::from(result.work.evaluations));
                assert!(result.complete);
                assert_ne!(edits.assignment(), &initial);
            }
            assert_eq!(d.assignment(), &initial);
            assert_eq!(d.score(), initial_score);
            d.assert_consistent().unwrap();
        }};
    }
    check!(
        ScoreDirector::new(&p, &bn, p.initial_assignment()).unwrap(),
        bm.clone(),
        plan()
    );
    let mut b = StreamGraphBuilder::new();
    let bs = b.source::<usize, (usize, Option<i64>)>();
    b.score("value", bs.port(), |(_, v)| {
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    check!(
        StreamScoreDirector::new(&p, b.build(), bs, p.initial_assignment()).unwrap(),
        bm,
        plan()
    );
    check!(
        ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
        lm.clone(),
        plan()
    );
    let mut b = StreamGraphBuilder::new();
    let ls = ListStreamSources::new(&mut b);
    b.score("owner", ls.values.port(), |v| {
        SimpleScore::new(v.position.unwrap().entity as i64)
    })
    .unwrap();
    check!(
        ListStreamScoreDirector::new(&l, b.build(), ls, l.initial_assignment()).unwrap(),
        lm,
        plan()
    );
    check!(
        MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
        mm.clone(),
        mixed_plan()
    );
    let mut b = StreamGraphBuilder::new();
    let ms = MixedStreamSources::new(&mut b);
    b.score("value", ms.basic.port(), |(_, v)| {
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    b.score("owner", ms.lists.values.port(), |v| {
        SimpleScore::new(v.position.unwrap().entity as i64)
    })
    .unwrap();
    check!(
        MixedStreamScoreDirector::new(&m, b.build(), ms, m.initial_assignment()).unwrap(),
        mm,
        mixed_plan()
    );
}
#[test]
fn nested_error_and_observer_panic_leave_outer_rollback_intact() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    {
        let mut scope = d.edit_session().unwrap();
        scope
            .apply(&Move::Change {
                variable: 0,
                value: Some(1),
            })
            .unwrap();
        let error = scope
            .run_solver_in_context(
                SolverSequence {
                    limits: Default::default(),
                    phases: vec![],
                },
                |view| assert_eq!(view.calculation_count(), 0),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            NestedSolverError::Solver(SolverError::EmptyPlan)
        ));
        assert_eq!(scope.assignment().values(), &[Some(1)]);
    }
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut scope = d.edit_session().unwrap();
        scope
            .apply(&Move::Change {
                variable: 0,
                value: Some(1),
            })
            .unwrap();
        let _ = scope.run_solver_in_context(plan(), |_| panic!("nested observer panic"));
    }));
    assert!(panic.is_err());
    assert_eq!(d.assignment(), &p.initial_assignment());
    d.assert_consistent().unwrap();
}
