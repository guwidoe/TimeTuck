//! Original graph-oracle, atomic refresh and solution-manager integration checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<i64>("derived"),
    ])])
    .unwrap()
}

#[test]
fn all_three_node_graphs_match_independent_reachability_and_do_not_run_callbacks() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("derived")
        .unwrap();
    let calls = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let nodes = (0..3)
        .map(|i| {
            builder
                .derive(field, i, |_, _| {
                    calls.set(calls.get() + 1);
                    Some(1i64)
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut session = builder.build();
    assert!(matches!(
        shadow_consistency(&session),
        Err(ShadowSessionError::NotCurrent)
    ));
    for mask in 0u16..512 {
        let mut reach = [[false; 3]; 3];
        for i in 0..3 {
            let mut deps = vec![];
            for (j, node) in nodes.iter().enumerate() {
                if mask & (1 << (i * 3 + j)) != 0 {
                    reach[i][j] = true;
                    deps.push(node.erase());
                }
            }
            session.set_dependencies(&nodes[i], &deps).unwrap();
        }
        for k in 0..3 {
            for i in 0..3 {
                for j in 0..3 {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        session.update(&()).unwrap();
        let before = calls.get();
        let report = shadow_consistency(&session).unwrap();
        assert_eq!(before, calls.get());
        let expected_affected = (0..3)
            .filter(|&i| (0..3).any(|j| reach[i][j] && reach[j][j]))
            .collect::<Vec<_>>();
        let mut expected_loops = vec![];
        for (i, row) in reach.iter().enumerate() {
            if row[i] && !(0..i).any(|j| reach[i][j] && reach[j][i]) {
                expected_loops.push(
                    (0..3)
                        .filter(|&j| reach[i][j] && reach[j][i])
                        .collect::<Vec<_>>(),
                );
            }
        }
        assert_eq!(
            report.affected.iter().map(|k| k.entity).collect::<Vec<_>>(),
            expected_affected,
            "mask={mask}"
        );
        assert_eq!(
            report
                .loops
                .iter()
                .map(|g| g.iter().map(|k| k.entity).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected_loops,
            "mask={mask}"
        );
        assert_eq!(report.is_consistent(), expected_affected.is_empty());
        for key in report.affected {
            assert_eq!(key.entity_type, PlanningType::of::<Entity>());
            assert_eq!(key.variable, "derived");
        }
    }
}

#[test]
fn strict_refresh_restores_pending_edges_and_accepts_recovery() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("derived")
        .unwrap();
    let mut builder = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let first = builder.derive(field, 0, |_, _| Some(7i64)).unwrap();
    let second = builder.derive(field, 1, |_, _| Some(8i64)).unwrap();
    builder.set_dependencies(&second, &[first.erase()]).unwrap();
    let mut session = builder.build();
    update_consistent_shadows(&mut session, &()).unwrap();
    let saved = session.checkpoint().unwrap();
    session.set_dependencies(&first, &[second.erase()]).unwrap();
    for _ in 0..2 {
        let err = update_consistent_shadows(&mut session, &()).err().unwrap();
        assert!(err.to_string().contains("requires a consistent solution"));
        assert!(err.to_string().contains("derived"));
        let ConsistentShadowUpdateError::Inconsistent(report) = err else {
            panic!("wrong error")
        };
        assert_eq!(report.loops.len(), 1);
        assert_eq!(
            report.loops[0].iter().map(|k| k.entity).collect::<Vec<_>>(),
            [0, 1]
        );
        // Failed refresh leaves the requested edge edit pending, rather than
        // pretending that the rejected cache is the current snapshot.
        assert!(matches!(
            session.read(&first),
            Err(ShadowSessionError::NotCurrent)
        ));
    }
    session.set_dependencies(&first, &[]).unwrap();
    assert_eq!(session.read(&first).unwrap().value, Some(&7));
    assert_eq!(session.read(&second).unwrap().value, Some(&8));
    assert!(
        update_consistent_shadows(&mut session, &())
            .unwrap()
            .is_empty()
    );
    // Old checkpoints still refer to the same session and source world.
    session.restore(&saved, &()).unwrap();
    assert!(shadow_consistency(&session).unwrap().is_consistent());
}

#[test]
fn strict_refresh_handles_initial_cycles_and_panicking_input_or_supplier() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = builder
        .input(entity.variable("input").unwrap(), 0, |world| {
            assert_ne!(*world, 11, "input failed");
            Some(*world)
        })
        .unwrap();
    let from = input.clone();
    let output = builder
        .derive(
            entity.variable("derived").unwrap(),
            0,
            move |world, values| {
                assert_ne!(*world, 12, "supplier failed");
                values.get(&from).unwrap().map(|x| x * 2)
            },
        )
        .unwrap();
    builder.set_dependencies(&output, &[input.erase()]).unwrap();
    let mut session = builder.build();
    session
        .set_dependencies(&output, &[input.erase(), output.erase()])
        .unwrap();
    assert!(matches!(
        update_consistent_shadows(&mut session, &3),
        Err(ConsistentShadowUpdateError::Inconsistent(_))
    ));
    assert!(matches!(
        shadow_consistency(&session),
        Err(ShadowSessionError::NotCurrent)
    ));
    session.set_dependencies(&output, &[input.erase()]).unwrap();
    update_consistent_shadows(&mut session, &3).unwrap();
    for world in [11, 12] {
        assert!(
            catch_unwind(AssertUnwindSafe(|| update_consistent_shadows(
                &mut session,
                &world
            )))
            .is_err()
        );
        assert_eq!(session.read(&input).unwrap().value, Some(&3));
        assert_eq!(session.read(&output).unwrap().value, Some(&6));
        assert!(shadow_consistency(&session).unwrap().is_consistent());
    }
    update_consistent_shadows(&mut session, &5).unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&10));
}

#[test]
fn diagnostic_clone_panic_rolls_back_successfully_evaluated_cache() {
    #[derive(Debug)]
    struct Key<'a>(&'a Cell<bool>);
    impl Clone for Key<'_> {
        fn clone(&self) -> Self {
            assert!(!self.0.get(), "key clone");
            Self(self.0)
        }
    }
    impl PartialEq for Key<'_> {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for Key<'_> {}
    let fail = Cell::new(false);
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("derived")
        .unwrap();
    let mut builder = ShadowSessionBuilder::<Key<'_>, ()>::new(&schema);
    let node = builder
        .derive(field, Key(&fail), |_, _| Some(42i64))
        .unwrap();
    let mut session = builder.build();
    update_consistent_shadows(&mut session, &()).unwrap();
    session.set_dependencies(&node, &[node.erase()]).unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| update_consistent_shadows(
            &mut session,
            &()
        )))
        .is_err()
    );
    fail.set(false);
    session.set_dependencies(&node, &[]).unwrap();
    assert_eq!(session.read(&node).unwrap().value, Some(&42));
    assert!(shadow_consistency(&session).unwrap().is_consistent());
}

#[test]
fn owned_report_distinguishes_entity_types_and_survives_schema() {
    struct Other;
    let report = {
        let schema = PlanningSchema::new::<Solution>(vec![
            PlanningEntityDefinition::of::<Entity>(vec![
                PlanningVariableDefinition::shadow::<i64>("same"),
            ]),
            PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::shadow::<i64>(
                "same",
            )]),
        ])
        .unwrap();
        let mut builder = ShadowSessionBuilder::<usize, ()>::new(&schema);
        let a = builder
            .derive(
                schema.entity::<Entity>().unwrap().variable("same").unwrap(),
                0,
                |_, _| Some(0i64),
            )
            .unwrap();
        let b = builder
            .derive(
                schema.entity::<Other>().unwrap().variable("same").unwrap(),
                0,
                |_, _| Some(0i64),
            )
            .unwrap();
        builder.set_dependencies(&a, &[a.erase()]).unwrap();
        builder.set_dependencies(&b, &[b.erase()]).unwrap();
        let mut session = builder.build();
        session.update(&()).unwrap();
        shadow_consistency(&session).unwrap()
    };
    assert_eq!(report.loops.len(), 2);
    assert_eq!(report.affected.len(), 2);
    assert_ne!(report.affected[0], report.affected[1]);
    assert_eq!(report.affected[0].entity_type, PlanningType::of::<Entity>());
    assert_eq!(report.affected[1].entity_type, PlanningType::of::<Other>());
}

#[test]
fn solution_refresh_rejects_cycles_before_scoring_and_recovers() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("derived")
        .unwrap();
    let mut builder = ShadowSessionBuilder::<usize, bool>::new(&schema);
    let a = builder.derive(field, 0, |_, _| Some(5i64)).unwrap();
    let b = builder.derive(field, 1, |_, _| Some(9i64)).unwrap();
    builder.set_dependencies(&b, &[a.erase()]).unwrap();
    let mut session = builder.build();
    let scores = Cell::new(0);
    let mut solution = ManagedSolution::new(false, vec![]);
    {
        let mut manager = SolutionManager::new(
            |cycle: &bool| {
                session.set_dependencies(&a, &if *cycle { vec![b.erase()] } else { vec![] })?;
                update_consistent_shadows(&mut session, cycle)?;
                Ok::<_, ConsistentShadowUpdateError<usize>>(vec![
                    session.read(&a)?.value.copied(),
                    session.read(&b)?.value.copied(),
                ])
            },
            |_, _| Ok(()),
            |_, shadows: &Vec<Option<i64>>| {
                scores.set(scores.get() + 1);
                Ok(shadows.iter().flatten().sum::<i64>())
            },
        );
        manager.update(&mut solution).unwrap();
        assert_eq!(solution.shadows, [Some(5), Some(9)]);
        assert_eq!(solution.score, Some(14));
        solution.assignment = true;
        let rejected = solution.clone();
        let err = manager.update(&mut solution).unwrap_err();
        let ConsistentShadowUpdateError::Inconsistent(report) = err else {
            panic!("wrong error")
        };
        assert_eq!(report.loops[0].len(), 2);
        assert_eq!(solution, rejected);
        assert_eq!(scores.get(), 1);
        solution.assignment = false;
        manager.update(&mut solution).unwrap();
        assert_eq!(scores.get(), 2);
    }
    assert!(shadow_consistency(&session).unwrap().is_consistent());
}
