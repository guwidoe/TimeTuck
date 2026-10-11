//! Original recoverable application errors through native shadow transactions.
use std::{
    cell::Cell,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Entity;
#[derive(Debug, Eq, PartialEq)]
struct Amount(i64); // No Clone bound on cached shadow values.
#[derive(Debug, Eq, PartialEq)]
struct DomainError(u8);
impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid domain {}", self.0)
    }
}
impl Error for DomainError {}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::list::<i64>("list", false),
        PlanningVariableDefinition::shadow::<Amount>("amount"),
        PlanningVariableDefinition::shadow::<i64>("total"),
    ])])
    .unwrap()
}
fn failure(
    error: ShadowSessionError,
    stage: ShadowCallbackStage,
    index: usize,
    field: &str,
    code: u8,
) -> ShadowCallbackError {
    assert_eq!(error, error.clone());
    assert!(
        error
            .to_string()
            .contains(&format!("invalid domain {code}"))
    );
    assert!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .is::<DomainError>()
    );
    let ShadowSessionError::Callback(error) = error else {
        panic!("wrong error category")
    };
    assert_eq!(error.stage(), stage);
    assert_eq!(error.registration_index(), index);
    assert_eq!(error.variable(), field);
    assert_eq!(error.entity_type(), std::any::type_name::<Entity>());
    assert_eq!(
        error.cause().downcast_ref::<DomainError>(),
        Some(&DomainError(code))
    );
    error
}
#[test]
fn reader_errors_stop_in_order_and_preserve_retry_restore_and_error_auto_traits() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<ShadowSessionError>();
    send_sync::<ShadowCallbackError>();
    let schema = schema();
    let e = schema.entity::<Entity>().unwrap();
    let fail = &Cell::new(0);
    let counts = &[Cell::new(0), Cell::new(0), Cell::new(0)];
    let mut b = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let a = b
        .try_input(e.variable("input").unwrap(), 0, move |w| {
            counts[0].set(counts[0].get() + 1);
            if fail.get() == 1 {
                Err(DomainError(1))
            } else {
                Ok(Some(*w))
            }
        })
        .unwrap();
    let list = b
        .try_list_input(e.variable("list").unwrap(), 0, move |w| {
            counts[1].set(counts[1].get() + 1);
            if fail.get() == 2 {
                Err(DomainError(2))
            } else {
                Ok(vec![*w])
            }
        })
        .unwrap();
    let read_a = a.clone();
    let read_list = list.clone();
    let total = b
        .derive(e.variable("total").unwrap(), 0, move |_, i| {
            counts[2].set(counts[2].get() + 1);
            Some(i.get(&read_a).unwrap().unwrap() + i.get(&read_list).unwrap().unwrap()[0])
        })
        .unwrap();
    b.set_dependencies(&total, &[a.erase(), list.erase()])
        .unwrap();
    let mut s = b.build();
    fail.set(1);
    let first = failure(
        s.update_notified(&1).err().unwrap(),
        ShadowCallbackStage::Input,
        0,
        "input",
        1,
    );
    assert_eq!(counts.each_ref().map(|c| c.get()), [1, 0, 0]);
    fail.set(0);
    s.update_notified(&1).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&2));
    let saved = s.checkpoint().unwrap();
    fail.set(2);
    s.mark_dirty(&a).unwrap();
    s.mark_dirty(&list).unwrap();
    failure(
        s.update_notified(&3).err().unwrap(),
        ShadowCallbackStage::Input,
        1,
        "list",
        2,
    );
    assert_eq!(counts[2].get(), 1);
    fail.set(0);
    s.update_notified(&3).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&6));
    assert_eq!(counts[2].get(), 2);
    fail.set(1);
    let second = failure(
        s.restore(&saved, &1).err().unwrap(),
        ShadowCallbackStage::Input,
        0,
        "input",
        1,
    );
    assert_ne!(
        first, second,
        "errors identify invocations, not identical messages"
    );
    assert_eq!(s.read(&total).unwrap().value, Some(&6));
    fail.set(0);
    s.restore(&saved, &1).unwrap();
    assert_eq!(counts[2].get(), 2, "restore runs no suppliers");
    assert_eq!(s.read(&total).unwrap().value, Some(&2));
}
#[derive(Clone)]
struct World {
    values: [i64; 2],
    parent: usize,
    cycle: bool,
}
#[test]
fn derived_errors_rollback_rebindings_cycles_and_prefix_values_without_losing_notifications() {
    let schema = schema();
    let e = schema.entity::<Entity>().unwrap();
    let fail = &Cell::new(0);
    let calls = &Cell::new(0);
    let downstream = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let a = b
        .input(e.variable("input").unwrap(), 0, |w| Some(w.values[0]))
        .unwrap();
    let z = b
        .input(e.variable("input").unwrap(), 1, |w| Some(w.values[1]))
        .unwrap();
    let reads = [a.clone(), z.clone()];
    let amount = b
        .try_derive(e.variable("amount").unwrap(), 0, move |w, i| {
            calls.set(calls.get() + 1);
            if fail.get() == 1 {
                return Err(DomainError(7));
            }
            assert_ne!(fail.get(), 2, "panic remains atomic");
            Ok(i.get(&reads[w.parent]).unwrap().map(|v| Amount(v * 2)))
        })
        .unwrap();
    let read = amount.clone();
    let total = b
        .derive(e.variable("total").unwrap(), 0, move |_, i| {
            downstream.set(downstream.get() + 1);
            i.get(&read).unwrap().map(|v| v.0 + 1)
        })
        .unwrap();
    b.set_dependencies(&total, &[amount.erase()]).unwrap();
    let refs = [a.erase(), z.erase()];
    let self_ref = amount.erase();
    b.bind_dependencies(&amount, move |w| {
        Ok(if w.cycle {
            vec![self_ref.clone()]
        } else {
            vec![refs[w.parent].clone()]
        })
    })
    .unwrap();
    let mut s = b.build();
    let original = World {
        values: [2, 10],
        parent: 0,
        cycle: false,
    };
    let mut w = original.clone();
    s.update(&w).unwrap();
    let saved = s.checkpoint().unwrap();
    w.values = [3, 20];
    w.parent = 1;
    fail.set(1);
    for h in [&a, &z] {
        s.mark_dirty(h).unwrap();
    }
    s.mark_dirty(&amount).unwrap();
    failure(
        s.update_notified(&w).err().unwrap(),
        ShadowCallbackStage::Derived,
        2,
        "amount",
        7,
    );
    assert_eq!((calls.get(), downstream.get()), (2, 1));
    assert!(matches!(
        s.read(&total),
        Err(ShadowSessionError::NotCurrent)
    ));
    fail.set(0);
    s.update_notified(&w).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&41));
    assert_eq!((calls.get(), downstream.get()), (3, 2));
    // Old source must no longer reach the supplier after the successful rewire.
    w.values[0] = 99;
    s.mark_dirty(&a).unwrap();
    s.update_notified(&w).unwrap();
    assert_eq!(calls.get(), 3);
    w.cycle = true;
    fail.set(1);
    s.mark_dirty(&amount).unwrap();
    s.update_notified(&w).unwrap();
    assert_eq!(calls.get(), 3);
    assert!(s.read(&total).unwrap().inconsistent);
    w.cycle = false;
    s.mark_dirty(&amount).unwrap();
    failure(
        s.update_notified(&w).err().unwrap(),
        ShadowCallbackStage::Derived,
        2,
        "amount",
        7,
    );
    fail.set(2);
    assert!(catch_unwind(AssertUnwindSafe(|| s.update_notified(&w))).is_err());
    fail.set(0);
    s.update_notified(&w).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&41));
    let before = calls.get();
    s.restore(&saved, &original).unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(s.read(&total).unwrap().value, Some(&5));
    // Manual topology edits also stay pending across a supplier error.
    s.set_dependencies(&total, &[a.erase(), amount.erase()])
        .unwrap();
    fail.set(1);
    s.mark_dirty(&amount).unwrap();
    failure(
        s.update_notified(&original).err().unwrap(),
        ShadowCallbackStage::Derived,
        2,
        "amount",
        7,
    );
    // Restore discards that invalid manual request without calling the total supplier.
    fail.set(0);
    s.restore(&saved, &original).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&5));
}
fn program<'a, 's>(
    schema: &'s PlanningSchema,
    fail: &'a Cell<u8>,
    calls: &'a Cell<usize>,
) -> (
    ShadowSessionBuilder<'a, 's, usize, i64>,
    ShadowNode<i64>,
    ShadowNode<Amount>,
) {
    let e = schema.entity::<Entity>().unwrap();
    let mut b = ShadowSessionBuilder::new(schema);
    let input = b
        .try_input(e.variable("input").unwrap(), 0, move |w: &i64| {
            if fail.get() == 1 {
                Err(DomainError(1))
            } else {
                Ok(Some(*w))
            }
        })
        .unwrap();
    let read = input.clone();
    let amount = b
        .try_derive(e.variable("amount").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            if fail.get() == 2 {
                return Err(DomainError(2));
            }
            assert_ne!(fail.get(), 3, "fallible callback may still panic");
            Ok(i.get(&read).unwrap().map(|v| Amount(v * 2)))
        })
        .unwrap();
    b.set_dependencies(&amount, &[input.erase()]).unwrap();
    (b, input, amount)
}
#[test]
fn manager_publishes_neither_shadows_nor_score_on_input_or_supplier_errors() {
    for initialized in [false, true] {
        for stage in [1, 2, 3] {
            let schema = schema();
            let fail = Cell::new(0);
            let calls = Cell::new(0);
            let (b, input, amount) = program(&schema, &fail, &calls);
            let output = amount.clone();
            let mut m = ShadowSolutionManager::new(
                b.build(),
                move |_, s| Ok::<_, DomainError>(Amount(s.read(&output).unwrap().value.unwrap().0)),
                |_, _| Ok(()),
                |_, v| Ok(SimpleScore::new(v.0)),
            );
            let mut solution = ManagedSolution::new(3i64, Amount(99));
            solution.score = Some(SimpleScore::new(100));
            if initialized {
                m.update(&mut solution).unwrap();
            }
            solution.assignment = 7;
            fail.set(stage);
            let result = catch_unwind(AssertUnwindSafe(|| m.update(&mut solution).map(|_| ())));
            if stage == 3 {
                assert!(result.is_err());
            } else {
                let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::Session(error))) =
                    result.unwrap()
                else {
                    panic!("wrong manager error")
                };
                failure(
                    error,
                    if stage == 1 {
                        ShadowCallbackStage::Input
                    } else {
                        ShadowCallbackStage::Derived
                    },
                    usize::from(stage - 1),
                    if stage == 1 { "input" } else { "amount" },
                    stage,
                );
            }
            assert_eq!(solution.assignment, 7);
            assert_eq!(solution.shadows.0, if initialized { 6 } else { 99 });
            assert_eq!(
                solution.score,
                Some(SimpleScore::new(if initialized { 6 } else { 100 }))
            );
            if initialized {
                assert_eq!(m.session().read(&input).unwrap().value, Some(&3));
            } else {
                assert!(matches!(
                    m.session().read(&input),
                    Err(ShadowSessionError::NotCurrent)
                ));
            }
            fail.set(0);
            m.update(&mut solution).unwrap();
            assert_eq!(solution.shadows.0, 14);
            assert_eq!(solution.score, Some(SimpleScore::new(14)));
            let before = calls.get();
            m.update(&mut solution).unwrap();
            assert_eq!(calls.get(), before);
        }
    }
}
#[test]
fn polling_and_notified_streams_rollback_failed_scores_and_keep_each_world_cache() {
    for notified in [false, true] {
        let schema = schema();
        let fail = Cell::new(0);
        let calls = Cell::new(0);
        let (shadows, input, amount) = program(&schema, &fail, &calls);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        let project = move |_: &i64, s: &ShadowSession<'_, '_, usize, i64>| {
            Ok(s.read(&amount)?.value.unwrap().0)
        };
        let mapped = if notified {
            b.map_shadows_notified(
                source.port(),
                shadows,
                move |before, after, s| {
                    if before != after {
                        s.mark_dirty(&input)?;
                    }
                    Ok(())
                },
                project,
            )
            .unwrap()
        } else {
            b.map_shadows(source.port(), shadows, project).unwrap()
        };
        b.score("amount", &mapped, |v| SimpleScore::new(*v))
            .unwrap();
        let mut g = b.build();
        {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(0, 3)).unwrap();
            t.apply(&source, TupleChange::put(1, 10)).unwrap();
            t.commit().unwrap();
        }
        assert_eq!(g.score(), SimpleScore::new(26));
        for stage in [1, 2, 3] {
            fail.set(stage);
            let result = catch_unwind(AssertUnwindSafe(|| {
                let mut t = g.transaction();
                let error = t.apply(&source, TupleChange::put(0, 7));
                assert!(matches!(t.score(), Err(GraphError::FailedTransaction)));
                error
            }));
            if stage == 3 {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().unwrap_err();
                assert!(matches!(error, GraphError::Dependency(_)));
                assert!(
                    error
                        .to_string()
                        .contains(&format!("invalid domain {stage}"))
                );
            }
            fail.set(0);
            assert_eq!(g.score(), SimpleScore::new(26));
            assert_eq!(g.source_rows(&source).unwrap()[&0], 3);
            let before = calls.get();
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(1, 10)).unwrap();
            t.apply(&source, TupleChange::put(0, 3)).unwrap();
            t.commit().unwrap();
            assert_eq!(calls.get(), before);
        }
        {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::put(0, 7)).unwrap();
            t.commit().unwrap();
        }
        assert_eq!(g.score(), SimpleScore::new(34));
        g.assert_consistent().unwrap();
    }
}
#[test]
fn successful_none_is_not_failure_and_registration_validation_calls_nothing() {
    let schema = schema();
    let e = schema.entity::<Entity>().unwrap();
    let calls = &Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, bool>::new(&schema);
    assert!(matches!(
        b.try_input(e.variable("list").unwrap(), 0, |_| Ok::<_, DomainError>(
            Some(1i64)
        )),
        Err(ShadowSessionError::WrongType)
    ));
    assert!(matches!(
        b.try_list_input(e.variable("input").unwrap(), 0, |_| Ok::<_, DomainError>(
            vec![1i64]
        )),
        Err(ShadowSessionError::WrongType)
    ));
    assert!(matches!(
        b.try_derive(
            e.variable("input").unwrap(),
            0,
            |_, _| Ok::<_, DomainError>(Some(1i64))
        ),
        Err(ShadowSessionError::WrongType)
    ));
    let a = b
        .try_input(e.variable("input").unwrap(), 0, |present| {
            Ok::<_, DomainError>(present.then_some(2i64))
        })
        .unwrap();
    let read = a.clone();
    let total = b
        .try_derive(e.variable("total").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            Ok::<_, DomainError>(i.get(&read).unwrap().copied())
        })
        .unwrap();
    b.set_dependencies(&total, &[a.erase()]).unwrap();
    assert_eq!(calls.get(), 0);
    let mut s = b.build();
    s.update(&false).unwrap();
    assert_eq!(s.read(&total).unwrap().value, None);
    assert!(!s.read(&total).unwrap().inconsistent);
    assert!(s.update(&false).unwrap().is_empty());
    assert_eq!(calls.get(), 1);
    s.update(&true).unwrap();
    assert_eq!(s.read(&total).unwrap().value, Some(&2));
    assert_eq!(calls.get(), 2);
}
