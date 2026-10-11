//! Original notified-refresh callback, differential, and rollback contracts.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Task;
#[derive(Clone)]
struct World {
    values: Vec<i64>,
    links: Vec<usize>,
    fail: Option<(&'static str, usize)>,
}
type Events = Rc<RefCell<Vec<(&'static str, usize)>>>;
type Fixture<'s> = (
    ShadowSession<'s, 's, usize, World>,
    Vec<ShadowNode<i64>>,
    Vec<ShadowNode<i64>>,
);
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("input", true),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap()
}
fn world(n: usize) -> World {
    World {
        values: (0..n as i64).collect(),
        links: (0..n).collect(),
        fail: None,
    }
}
fn fixture<'s>(schema: &'s PlanningSchema, n: usize, log: &Events) -> Fixture<'s> {
    let fields = schema.entity::<Task>().unwrap();
    let input_field = fields.variable("input").unwrap();
    let output_field = fields.variable("output").unwrap();
    let mut builder = ShadowSessionBuilder::<usize, World>::new(schema);
    let inputs: Vec<_> = (0..n)
        .map(|i| {
            let log = log.clone();
            builder
                .input(input_field, i, move |w| {
                    log.borrow_mut().push(("read", i));
                    assert_ne!(w.fail, Some(("read", i)), "reader panic");
                    Some(w.values[i])
                })
                .unwrap()
        })
        .collect();
    let outputs: Vec<_> = (0..n)
        .map(|i| {
            let log = log.clone();
            builder
                .derive(output_field, i, move |w, values| {
                    log.borrow_mut().push(("supply", i));
                    assert_ne!(w.fail, Some(("supply", i)), "supplier panic");
                    let total: i64 = values
                        .for_field::<i64>(input_field)
                        .unwrap()
                        .into_iter()
                        .chain(values.for_field::<i64>(output_field).unwrap())
                        .flatten()
                        .sum();
                    Some(total + 1)
                })
                .unwrap()
        })
        .collect();
    let nodes: Rc<[_]> = inputs
        .iter()
        .chain(&outputs)
        .map(ShadowNode::erase)
        .collect();
    for (i, output) in outputs.iter().enumerate() {
        let nodes = nodes.clone();
        let log = log.clone();
        builder
            .bind_dependencies(output, move |w| {
                log.borrow_mut().push(("resolve", i));
                if w.fail == Some(("resolve", i)) {
                    return Err(ShadowSessionError::UnregisteredSource);
                }
                if w.fail == Some(("resolve_panic", i)) {
                    panic!("resolver panic");
                }
                Ok(vec![nodes[w.links[i]].clone()])
            })
            .unwrap();
    }
    (builder.build(), inputs, outputs)
}
fn counts(log: &Events) -> [usize; 3] {
    ["resolve", "read", "supply"]
        .map(|kind| log.borrow().iter().filter(|(k, _)| *k == kind).count())
}
fn output_values(
    session: &ShadowSession<'_, '_, usize, World>,
    nodes: &[ShadowNode<i64>],
) -> Vec<(Option<i64>, bool)> {
    nodes
        .iter()
        .map(|n| {
            let read = session.read(n).unwrap();
            (read.value.copied(), read.inconsistent)
        })
        .collect()
}
fn changed_owners(session: &ShadowSession<'_, '_, usize, World>) -> Vec<usize> {
    session
        .changed_entities()
        .unwrap()
        .iter()
        .map(|g| *session.entity(&g[0]).unwrap())
        .collect()
}
#[test]
fn sparse_refreshes_bound_callbacks_and_ordinary_updates_still_poll_everything() {
    let n = 1024;
    let schema = schema();
    let log = Events::default();
    let (mut session, inputs, outputs) = fixture(&schema, n, &log);
    let mut w = world(n);
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [n, n, n]);
    assert_eq!(
        &log.borrow()[..n],
        &(0..n).map(|i| ("resolve", i)).collect::<Vec<_>>()
    );
    assert_eq!(
        &log.borrow()[n..2 * n],
        &(0..n).map(|i| ("read", i)).collect::<Vec<_>>()
    );
    log.borrow_mut().clear();
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [0, 0, 0]);
    assert!(session.changed_entities().unwrap().is_empty());
    w.values[511] += 7;
    session.mark_dirty(&inputs[511]).unwrap();
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [0, 1, 1]);
    assert_eq!(session.read(&outputs[511]).unwrap().value, Some(&519));
    assert_eq!(changed_owners(&session), [511]);
    log.borrow_mut().clear();
    w.links[512] = n + 511;
    session.mark_dirty(&outputs[512]).unwrap();
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [1, 0, 1]);
    assert_eq!(session.read(&outputs[512]).unwrap().value, Some(&520));
    log.borrow_mut().clear();
    session.mark_dirty(&inputs[511]).unwrap();
    w.values[511] += 1;
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [0, 1, 2]);
    assert_eq!(changed_owners(&session), [511, 512]);
    log.borrow_mut().clear();
    session
        .mark_entities_dirty(&[inputs[511].erase(), outputs[511].erase()])
        .unwrap();
    session.update_notified(&w).unwrap();
    assert_eq!(counts(&log), [1, 1, 1]);
    assert!(session.changed_entities().unwrap().is_empty());
    log.borrow_mut().clear();
    session.update(&w).unwrap();
    assert_eq!(counts(&log), [n, n, 0]);
    log.borrow_mut().clear();
    assert!(missing_shadow_sources(&session, &w).unwrap().is_empty());
    assert_eq!(counts(&log), [n, 0, 0]);
}
#[test]
fn all_small_dynamic_graphs_match_full_refresh_values_flags_and_reports() {
    let schema = schema();
    let (mut full, _, full_outputs) = fixture(&schema, 3, &Events::default());
    let (mut notified, inputs, outputs) = fixture(&schema, 3, &Events::default());
    let mut w = world(3);
    full.update(&w).unwrap();
    notified.update_notified(&w).unwrap();
    // Every one-source graph over three inputs and three outputs, including
    // self loops, larger cycles, cycle descendants, rewires and recovery.
    for code in 0..216usize {
        let mut digits = code;
        for (i, output) in outputs.iter().enumerate() {
            let next = digits % 6;
            digits /= 6;
            if w.links[i] != next {
                notified.mark_dirty(output).unwrap();
            }
            w.links[i] = next;
        }
        let changed_input = code % 3;
        w.values[changed_input] += 1;
        notified.mark_dirty(&inputs[changed_input]).unwrap();
        full.update(&w).unwrap();
        notified.update_notified(&w).unwrap();
        assert_eq!(
            output_values(&notified, &outputs),
            output_values(&full, &full_outputs),
            "graph {code}"
        );
        assert_eq!(
            changed_owners(&notified),
            changed_owners(&full),
            "report {code}"
        );
    }
}
#[test]
fn manual_rewires_and_cycle_recovery_keep_unnotified_cached_inputs() {
    let schema = schema();
    let fields = schema.entity::<Task>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = builder
        .input(fields.variable("input").unwrap(), 0, |w| {
            assert!(*w >= 0, "unnotified reader must not run");
            Some(*w)
        })
        .unwrap();
    let input_field = fields.variable("input").unwrap();
    let output_field = fields.variable("output").unwrap();
    let mut derive = |key| {
        builder
            .derive(output_field, key, move |_, values| {
                Some(
                    values
                        .for_field::<i64>(input_field)
                        .unwrap()
                        .into_iter()
                        .chain(values.for_field::<i64>(output_field).unwrap())
                        .flatten()
                        .sum::<i64>()
                        + 1,
                )
            })
            .unwrap()
    };
    let a = derive(0);
    let b = derive(1);
    builder.set_dependencies(&a, &[input.erase()]).unwrap();
    builder.set_dependencies(&b, &[a.erase()]).unwrap();
    let mut session = builder.build();
    session.update_notified(&3).unwrap();
    session.set_dependencies(&a, &[b.erase()]).unwrap();
    session.update_notified(&-1).unwrap();
    assert!(session.read(&b).unwrap().inconsistent);
    session.set_dependencies(&a, &[input.erase()]).unwrap();
    session.update_notified(&-1).unwrap();
    assert_eq!(session.read(&input).unwrap().value, Some(&3));
    assert_eq!(session.read(&a).unwrap().value, Some(&4));
    assert_eq!(session.read(&b).unwrap().value, Some(&5));
    // Checked restore deliberately samples the world; it does not adopt the
    // notification contract or silently accept a mismatching input snapshot.
    let saved = session.checkpoint().unwrap();
    assert_eq!(
        session.restore(&saved, &9),
        Err(ShadowSessionError::SnapshotMismatch)
    );
    assert_eq!(session.read(&b).unwrap().value, Some(&5));
}
#[test]
fn resolver_reader_and_supplier_failures_preserve_pending_work_for_retry() {
    for kind in ["resolve", "resolve_panic", "read", "supply"] {
        let schema = schema();
        let log = Events::default();
        let (mut session, inputs, outputs) = fixture(&schema, 3, &log);
        let mut w = world(3);
        session.update_notified(&w).unwrap();
        let saved = session.checkpoint().unwrap();
        w.values[0] = 50;
        w.links[1] = 0;
        session.mark_dirty(&inputs[0]).unwrap();
        session.mark_dirty(&outputs[1]).unwrap();
        w.fail = Some((kind, if kind == "read" { 0 } else { 1 }));
        log.borrow_mut().clear();
        let result = catch_unwind(AssertUnwindSafe(|| session.update_notified(&w)));
        if kind == "resolve" {
            assert!(result.unwrap().is_err());
        } else {
            assert!(result.is_err());
        }
        assert!(matches!(
            session.read(&inputs[0]),
            Err(ShadowSessionError::NotCurrent)
        ));
        if kind.starts_with("resolve") {
            assert_eq!(counts(&log), [1, 0, 0]);
        }
        w.fail = None;
        session.update_notified(&w).unwrap();
        assert_eq!(session.read(&outputs[0]).unwrap().value, Some(&51));
        assert_eq!(session.read(&outputs[1]).unwrap().value, Some(&51));
        session.restore(&saved, &world(3)).unwrap();
        assert_eq!(
            output_values(&session, &outputs),
            vec![(Some(1), false), (Some(2), false), (Some(3), false)]
        );
    }
}
#[test]
fn manager_policies_and_every_publication_stage_preserve_notified_transactions() {
    let schema = schema();
    let log = Events::default();
    let (session, inputs, outputs) = fixture(&schema, 3, &log);
    let projected = outputs.clone();
    let mut manager = ShadowSolutionManager::new(
        session,
        move |w, session| {
            if w.fail == Some(("project", 0)) {
                return Err("project");
            }
            if w.fail == Some(("project_panic", 0)) {
                panic!("project");
            }
            Ok(output_values(session, &projected))
        },
        |w, _| {
            if w.fail == Some(("validate", 0)) {
                return Err("validate");
            }
            if w.fail == Some(("validate_panic", 0)) {
                panic!("validate");
            }
            Ok(())
        },
        |w, values| {
            if w.fail == Some(("score", 0)) {
                return Err("score");
            }
            if w.fail == Some(("score_panic", 0)) {
                panic!("score");
            }
            Ok(values
                .iter()
                .map(|(value, inconsistent)| value.unwrap_or(0) - i64::from(*inconsistent) * 100)
                .sum::<i64>())
        },
    )
    .with_preflight(|w| {
        if w.fail == Some(("preflight", 0)) {
            return Err("preflight");
        }
        if w.fail == Some(("preflight_panic", 0)) {
            panic!("preflight");
        }
        Ok(())
    });
    let mut solution = ManagedSolution::new(world(3), vec![]);
    // First failure must leave all initial notifications pending.
    solution.assignment.fail = Some(("resolve", 1));
    assert!(matches!(
        manager.update_notified(&mut solution),
        Err(ShadowSolutionError::Shadows(
            ConsistentShadowUpdateError::Session(ShadowSessionError::UnregisteredSource)
        ))
    ));
    assert!(solution.shadows.is_empty());
    assert_eq!(solution.score, None);
    solution.assignment.fail = None;
    log.borrow_mut().clear();
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(counts(&log), [3, 3, 3]);
    assert_eq!(solution.score, Some(6));
    solution.assignment.values[0] = 10;
    manager.session_mut().mark_dirty(&inputs[0]).unwrap();
    log.borrow_mut().clear();
    manager
        .update_notified_with_policy(&mut solution, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(counts(&log), [0, 0, 0]);
    assert!(matches!(
        manager.session().read(&inputs[0]),
        Err(ShadowSessionError::NotCurrent)
    ));
    assert_eq!(solution.score, Some(6));
    manager
        .update_notified_with_policy(&mut solution, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(counts(&log), [0, 1, 1]);
    assert_eq!(solution.shadows[0], (Some(11), false));
    assert_eq!(solution.score, Some(6));
    log.borrow_mut().clear();
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(counts(&log), [0, 0, 0]);
    assert_eq!(solution.score, Some(16));
    for stage in [
        "preflight",
        "preflight_panic",
        "project",
        "project_panic",
        "validate",
        "validate_panic",
        "score",
        "score_panic",
    ] {
        let before = solution.shadows.clone();
        let score = solution.score;
        solution.assignment.values[0] += 1;
        manager
            .session_mut()
            .mark_entities_dirty(&[outputs[0].erase()])
            .unwrap();
        solution.assignment.fail = Some((stage, 0));
        log.borrow_mut().clear();
        let result = catch_unwind(AssertUnwindSafe(|| {
            manager
                .update_notified(&mut solution)
                .map(|score| score.copied())
        }));
        if stage.ends_with("panic") {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        if stage.starts_with("preflight") {
            assert_eq!(counts(&log), [0, 0, 0]);
        }
        assert_eq!(solution.shadows, before);
        assert_eq!(solution.score, score);
        assert!(matches!(
            manager.session().read(&outputs[0]),
            Err(ShadowSessionError::NotCurrent)
        ));
        solution.assignment.fail = None;
        log.borrow_mut().clear();
        manager.update_notified(&mut solution).unwrap();
        assert_eq!(counts(&log), [1, 1, 1]);
        assert_eq!(
            solution.shadows[0],
            (Some(solution.assignment.values[0] + 1), false)
        );
        assert_eq!(changed_owners(manager.session()), [0]);
    }
}
#[test]
fn notified_manager_rejects_cycles_atomically_or_publishes_them_by_policy() {
    let schema = schema();
    let log = Events::default();
    let (session, inputs, outputs) = fixture(&schema, 3, &log);
    let projected = outputs.clone();
    let mut manager = ShadowSolutionManager::new(
        session,
        move |_, session| Ok::<_, ()>(output_values(session, &projected)),
        |_, _| Ok(()),
        |_, values| Ok(values.iter().filter(|(_, bad)| *bad).count()),
    );
    let mut solution = ManagedSolution::new(world(3), vec![]);
    manager.update_notified(&mut solution).unwrap();
    let before = solution.shadows.clone();
    solution.assignment.links[0] = 4;
    solution.assignment.links[1] = 3;
    manager.session_mut().mark_dirty(&outputs[0]).unwrap();
    manager.session_mut().mark_dirty(&outputs[1]).unwrap();
    assert!(matches!(
        manager.update_notified(&mut solution),
        Err(ShadowSolutionError::Shadows(
            ConsistentShadowUpdateError::Inconsistent(_)
        ))
    ));
    assert_eq!(solution.shadows, before);
    assert_eq!(solution.score, Some(0));
    manager = manager.with_consistency_policy(ShadowConsistencyPolicy::AllowInconsistent);
    log.borrow_mut().clear();
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(counts(&log), [2, 0, 0]);
    assert_eq!(
        solution.shadows,
        [(None, true), (None, true), (Some(3), false)]
    );
    assert_eq!(solution.score, Some(2));
    solution.assignment.links[0] = 0;
    solution.assignment.links[1] = 1;
    solution.assignment.values[0] = 30;
    for output in &outputs[..2] {
        manager.session_mut().mark_dirty(output).unwrap();
    }
    manager.session_mut().mark_dirty(&inputs[0]).unwrap();
    log.borrow_mut().clear();
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(counts(&log), [2, 1, 2]);
    assert_eq!(
        solution.shadows,
        [(Some(31), false), (Some(2), false), (Some(3), false)]
    );
    assert_eq!(solution.score, Some(0));
}
#[test]
fn notified_missing_source_diagnostics_cover_selected_targets_and_retry_atomically() {
    struct Linked {
        id: usize,
        source: Option<Box<Linked>>,
    }
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Linked>(vec![
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap();
    let field = schema
        .entity::<Linked>()
        .unwrap()
        .variable("output")
        .unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one("source", SourceMemberRole::Fact, |root: &Linked| {
            root.source.as_deref()
        })
        .unwrap();
    paths
        .one(
            "output",
            SourceMemberRole::Declarative,
            |_: &Linked| -> Option<&i64> { panic!("terminal getter") },
        )
        .unwrap();
    let path = paths.compile::<Linked>("output", "source.output").unwrap();
    let suppliers = std::cell::Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<Linked>>::new(&schema);
    let mut handles = Vec::new();
    for i in 0..2 {
        let handle = builder
            .derive(field, i, |_, _| {
                suppliers.set(suppliers.get() + 1);
                Some(1i64)
            })
            .unwrap();
        builder
            .bind_source_paths(
                &handle,
                vec![&path],
                move |world| &world[i],
                |value| value.downcast_ref::<Linked>().map(|node| node.id),
            )
            .unwrap();
        handles.push(handle);
    }
    let projected = handles.clone();
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            Ok::<_, ()>(
                projected
                    .iter()
                    .map(|node| session.read(node).unwrap().value.copied())
                    .collect::<Vec<_>>(),
            )
        },
        |_, _| Ok(()),
        |_, values| Ok(values.iter().flatten().sum::<i64>()),
    );
    let mut solution = ManagedSolution::new(
        vec![
            Linked {
                id: 0,
                source: None,
            },
            Linked {
                id: 1,
                source: None,
            },
        ],
        vec![],
    );
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(suppliers.get(), 2);
    solution.assignment[0].source = Some(Box::new(Linked {
        id: 7,
        source: None,
    }));
    solution.assignment[1].source = Some(Box::new(Linked {
        id: 8,
        source: None,
    }));
    // Deliberately incomplete notifications demonstrate diagnostic scope; callers
    // must notify both targets before any successful refresh of this world.
    manager.session_mut().mark_dirty(&handles[0]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing source expected");
    };
    assert_eq!(
        missing
            .iter()
            .map(|r| (r.target.entity, r.source.entity))
            .collect::<Vec<_>>(),
        [(0, 7)]
    );
    let all = missing_shadow_sources(manager.session(), &solution.assignment).unwrap();
    assert_eq!(
        all.iter()
            .map(|r| (r.target.entity, r.source.entity))
            .collect::<Vec<_>>(),
        [(0, 7), (1, 8)]
    );
    manager.session_mut().mark_dirty(&handles[1]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing sources expected");
    };
    assert_eq!(missing, all);
    assert_eq!(suppliers.get(), 2);
    assert_eq!(solution.shadows, [Some(1), Some(1)]);
    assert_eq!(solution.score, Some(2));
    for root in &mut solution.assignment {
        root.source = None;
    }
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(suppliers.get(), 4);
    assert!(manager.session().changed_entities().unwrap().is_empty());
}
