//! Original owned diagnostics, propagation, rollback and range-filter regressions.
use std::{
    error::Error,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 2])
            .with_initial(0)
            .with_range_diagnostics("basic entity", "value", i32::to_string),
    ])
    .unwrap()
}
fn lists() -> ListProblem<i32> {
    ListProblem::new(
        vec![0, 2],
        vec![
            ListEntity::new(vec![0]),
            ListEntity::new(vec![]).with_range_diagnostics("list entity", "values", i32::to_string),
        ],
    )
    .unwrap()
}
fn basic_move() -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(99),
    }
}
fn list_move() -> ListMove<i32> {
    ListMove::Assign {
        value: 99,
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn message<T, E: std::fmt::Display>(r: Result<T, E>) -> String {
    match r {
        Ok(_) => panic!("invalid move accepted"),
        Err(e) => e.to_string(),
    }
}
#[test]
fn custom_phase_propagates_reports_and_restores_all_six_scoring_backends() {
    // Both basic and list errors are exercised through each mixed backend.
    for mode in 0..8 {
        macro_rules! command {
            ($c:ident, $m:expr) => {{
                let before = $c.assignment().clone();
                let error = $c.execute(&$m).unwrap_err();
                assert_eq!($c.assignment(), &before);
                Err(error)
            }};
        }
        let text = if mode < 2 {
            let p = basic();
            if mode == 0 {
                message(custom_phase(
                    &p,
                    &ConstraintNetwork::<_, SimpleScore>::new(1),
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, basic_move()),
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                message(stream_custom_phase(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, basic_move()),
                ))
            }
        } else if mode < 4 {
            let p = lists();
            if mode == 2 {
                message(list_custom_phase(
                    &p,
                    &ListConstraintNetwork::<_, SimpleScore>::new(2, 2),
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, list_move()),
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = ListStreamSources::new(&mut b);
                message(list_stream_custom_phase(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, list_move()),
                ))
            }
        } else {
            let p = MixedProblem::new(basic(), lists());
            let movement = if mode < 6 {
                MixedMove {
                    basic: basic_move(),
                    lists: ListMove::Composite(vec![]),
                }
            } else {
                MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(2),
                    },
                    lists: list_move(),
                }
            };
            if mode % 2 == 0 {
                message(mixed_custom_phase(
                    &p,
                    &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 2, 2),
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, movement),
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = MixedStreamSources::new(&mut b);
                message(mixed_stream_custom_phase(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    Default::default(),
                    |c| command!(c, movement),
                ))
            }
        };
        let label = if matches!(mode, 0 | 1 | 4 | 5) {
            "entity basic entity, variable value"
        } else {
            "entity list entity, variable values"
        };
        assert!(text.contains(label), "{mode}: {text}");
        assert!(
            text.contains("value 99 outside allowed range [0, 2]"),
            "{mode}: {text}"
        );
    }
}
#[test]
fn reports_survive_rollback_preserve_causes_and_coexist_with_borrowed_diagnostics() {
    let p = basic();
    let mut a = p.initial_assignment();
    let e = p.apply(&mut a, &basic_move()).err().unwrap();
    assert_eq!(a, p.initial_assignment());
    assert_eq!(
        e.without_range_report(),
        &ModelError::ValueOutsideDomain { variable: 0 }
    );
    assert!(e.source().unwrap().downcast_ref::<ModelError>().is_some());
    assert_eq!(
        e.range_report().unwrap().location,
        RangeViolationLocation::Basic { variable: 0 }
    );
    let bad = Assignment::new(vec![Some(99)]);
    let borrowed = validate_basic_with_diagnostics(&p, &bad).unwrap_err();
    assert!(std::ptr::eq(
        borrowed.violation.as_ref().unwrap().value,
        bad.values()[0].as_ref().unwrap()
    ));
    let vars = [Variable::new(vec![0, 2])
        .with_initial(99)
        .with_range_diagnostics("e", "v", i32::to_string)];
    assert!(
        build_basic_with_diagnostics(&vars)
            .unwrap_err()
            .violation
            .is_some()
    );
    drop(p);
    assert_eq!(e.range_report().unwrap().allowed, ["0", "2"]);
    let p = ListProblem::new(
        vec![0, 2],
        vec![
            ListEntity::new(vec![0]).with_value_range(vec![0]),
            ListEntity::new(vec![])
                .with_value_range(vec![2])
                .with_range_diagnostics("receiver", "values", i32::to_string),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let m = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    let e = p.apply(&mut a, &m).err().unwrap();
    assert_eq!(a, before);
    assert_eq!(
        e.without_range_report(),
        &ListError::ValueOutsideEntityRange { entity: 1 }
    );
    assert_eq!(e.range_report().unwrap().allowed, ["2"]);
    let bad = ListAssignment::new(vec![vec![], vec![0]]);
    let e = validate_list_with_diagnostics(&p, &bad).unwrap_err();
    assert_eq!(*e.violation.unwrap().value, 0);
    let es = [ListEntity::new(vec![99]).with_range_diagnostics("e", "v", i32::to_string)];
    assert!(
        build_list_with_diagnostics(&[0, 2], &es, true)
            .unwrap_err()
            .violation
            .is_some()
    );
}
#[test]
fn renderers_run_only_on_range_failure_and_panics_restore_the_whole_composite() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 2])
            .with_initial(0)
            .with_range_diagnostics("e", "v", move |v| {
                seen.fetch_add(1, Ordering::SeqCst);
                v.to_string()
            }),
    ])
    .unwrap();
    let mut a = p.initial_assignment();
    p.apply(
        &mut a,
        &Move::Change {
            variable: 0,
            value: Some(2),
        },
    )
    .unwrap()
    .commit();
    let resumed = p.with_initial_assignment(&a).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(resumed.apply(&mut a, &basic_move()).is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 3); // rejected value plus two allowed values
    let p = ListProblem::new(
        vec![0, 2],
        vec![
            ListEntity::new(vec![0]),
            ListEntity::new(vec![]).with_range_diagnostics("e", "v", |_| panic!("renderer")),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let before = a.clone();
    let m = ListMove::Composite(vec![
        ListMove::Assign {
            value: 2,
            destination: ListPosition {
                entity: 0,
                index: 1,
            },
        },
        list_move(),
    ]);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = p.apply(&mut a, &m);
        }))
        .is_err()
    );
    assert_eq!(a, before);
    let invalid_index = ListMove::Assign {
        value: 99,
        destination: ListPosition {
            entity: 8,
            index: 0,
        },
    };
    assert!(matches!(
        p.apply(&mut a, &invalid_index),
        Err(ListError::EntityIndex { .. })
    ));
}
#[test]
fn diagnostics_do_not_change_seeded_range_restricted_kopt_candidates() {
    let make = |diagnostics| {
        ListProblem::new(
            (0..6).collect::<Vec<i32>>(),
            vec![
                ListEntity::new(vec![0, 1, 2]).with_value_range(vec![0, 1, 2, 3]),
                ListEntity::new(vec![3, 4, 5]).with_value_range(vec![2, 3, 4, 5]),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                if diagnostics {
                    e.with_range_diagnostics(i.to_string(), "values", i32::to_string)
                } else {
                    e
                }
            })
            .collect(),
        )
        .unwrap()
    };
    let plain = make(false);
    let reported = make(true);
    let sampling = ListKOptSampling::new(ListKOptConfig::new(2, 5).unwrap(), 17, 200);
    let a = plain.initial_assignment();
    let expected: Vec<_> = sample_list_k_opt_repartitions(&plain, &a, sampling.clone())
        .unwrap()
        .collect();
    let actual: Vec<_> = sample_list_k_opt_repartitions(&reported, &a, sampling)
        .unwrap()
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(actual, expected);
}

#[test]
fn local_search_propagates_provider_errors_through_all_six_backends() {
    let config = LocalSearchConfig::<SimpleScore> {
        max_steps: 1,
        ..Default::default()
    };
    let list_initial = || ListAssignment::new(vec![vec![0], vec![2]]);
    for mode in 0..8 {
        let text = if mode < 2 {
            let p = basic();
            let mut provider =
                |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![basic_move()];
            if mode == 0 {
                message(local_search_from_with_provider(
                    &p,
                    &ConstraintNetwork::<_, SimpleScore>::new(1),
                    p.initial_assignment(),
                    config.clone(),
                    &mut provider,
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                message(stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    source,
                    p.initial_assignment(),
                    config.clone(),
                    &mut provider,
                ))
            }
        } else if mode < 4 {
            let p = lists();
            let mut provider =
                |_: &ListProblem<i32>, _: &ListAssignment<i32>, _: u64| vec![list_move()];
            if mode == 2 {
                message(list_local_search_from_with_provider(
                    &p,
                    &ListConstraintNetwork::<_, SimpleScore>::new(2, 2),
                    list_initial(),
                    config.clone(),
                    &mut provider,
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = ListStreamSources::new(&mut b);
                message(list_stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    source,
                    list_initial(),
                    config.clone(),
                    &mut provider,
                ))
            }
        } else {
            let p = MixedProblem::new(basic(), lists());
            let a = MixedAssignment::new(p.basic().initial_assignment(), list_initial());
            let mut provider =
                |_: &MixedProblem<i32, i32>, _: &MixedAssignment<i32, i32>, _: u64| {
                    vec![if mode < 6 {
                        MixedMove {
                            basic: basic_move(),
                            lists: ListMove::Composite(vec![]),
                        }
                    } else {
                        MixedMove {
                            basic: Move::Change {
                                variable: 0,
                                value: Some(2),
                            },
                            lists: list_move(),
                        }
                    }]
                };
            let config = MixedLocalSearchConfig {
                search: config.clone(),
                ..Default::default()
            };
            if mode % 2 == 0 {
                message(mixed_local_search_from_with_provider(
                    &p,
                    &MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 2, 2),
                    a,
                    config,
                    &mut provider,
                ))
            } else {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = MixedStreamSources::new(&mut b);
                message(mixed_stream_local_search_from_with_provider(
                    &p,
                    b.build(),
                    source,
                    a,
                    config,
                    &mut provider,
                ))
            }
        };
        let label = if matches!(mode, 0 | 1 | 4 | 5) {
            "entity basic entity, variable value"
        } else {
            "entity list entity, variable values"
        };
        assert!(text.contains(label), "{mode}: {text}");
        assert!(
            text.contains("value 99 outside allowed range [0, 2]"),
            "{mode}: {text}"
        );
    }
}
#[test]
fn metadata_does_not_require_debug_display_clone_or_static_values() {
    #[derive(Eq, PartialEq)]
    struct Value<'a>(&'a str);
    let good = String::from("good");
    let bad = String::from("bad");
    let p = FiniteProblem::new(vec![
        Variable::new(vec![Value(&good)]).with_range_diagnostics(
            "entity",
            "value",
            |v: &Value<'_>| v.0.to_string(),
        ),
    ])
    .unwrap();
    let a = Assignment::new(vec![Some(Value(&bad))]);
    let e = p.validate(&a).unwrap_err();
    assert_eq!(e.range_report().unwrap().value, "bad");
    assert_eq!(e.range_report().unwrap().allowed, ["good"]);
}
