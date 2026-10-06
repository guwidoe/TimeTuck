// Adapted from pinned Timefold Community DefaultSolverTest and comparator fixtures.
// Apache-2.0; sources and native substitutions: compat/provenance.json.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
#[test]
fn solve_with_multiple_sorter_manners() {
    // Exact Collections.shuffle(Random(0)) fixture encounter order: the same RNG
    // shuffles entities first, then values. Captured through public JDK APIs.
    let entities = [4, 2, 1, 3, 0];
    let values = vec![3, 4, 2, 1, 0];
    let p = FiniteProblem::new(
        entities
            .iter()
            .map(|_| Variable::new(values.clone()))
            .collect(),
    )
    .unwrap();
    let mut n = ConstraintNetwork::new(5);
    n.add_constraint("one distinct value per entity", (0..5).collect(), |v| {
        let assigned = v.iter().flatten().count() as i64;
        let distinct = v
            .iter()
            .filter_map(|v| v.copied())
            .collect::<BTreeSet<_>>()
            .len() as i64;
        HardSoftScore::new(-(assigned - distinct), -distinct)
    })
    .unwrap();
    let ascending = (0..5)
        .map(|i| BasicValueOrder::sorted_by(&p, i, |a, b| a.cmp(&b)).unwrap())
        .collect::<Vec<_>>();
    let descending = (0..5)
        .map(|i| BasicValueOrder::sorted_by(&p, i, |a, b| b.cmp(&a)).unwrap())
        .collect::<Vec<_>>();
    let families = [
        (
            BasicNeighborhood {
                neighborhood: Neighborhood::Change,
                value_orders: &ascending,
                ..Default::default()
            },
            1,
        ),
        (
            BasicNeighborhood {
                neighborhood: Neighborhood::Change,
                value_orders: &descending,
                ..Default::default()
            },
            1,
        ),
    ];
    let a = p.initial_assignment();
    for (index, (c, _)) in families.iter().enumerate() {
        let first = basic_moves_configured(&p, &a, *c)
            .unwrap()
            .take(5)
            .map(|m| {
                let Move::Change { variable, value } = m else {
                    panic!()
                };
                assert_eq!(variable, 0);
                value.unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            first,
            if index == 0 {
                vec![0, 1, 2, 3, 4]
            } else {
                vec![4, 3, 2, 1, 0]
            }
        );
    }
    let calls = Cell::new(0);
    let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64| {
        calls.set(calls.get() + 1);
        basic_moves_configured_union(p, a, &families, UnionOrder::Random { seed: 17 }, step)
            .unwrap()
            .collect::<Vec<_>>()
    };
    let events = Cell::new(0);
    let mut observer = |e: SolverEvent<'_, Assignment<i32>, HardSoftScore>| {
        if matches!(e, SolverEvent::Best(_)) {
            events.set(events.get() + 1);
        }
    };
    let mut extensions = SolverExtensions::default();
    extensions.move_providers.bind(2, &mut provider);
    extensions.events = Some(&mut observer);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_move_evaluations: Some(10),
                    max_steps: 16,
                    assert_scores: true,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        },
        extensions,
    )
    .unwrap();
    assert!(r.solution().is_some() && events.get() > 0 && calls.get() > 0);
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.phases[1].work.move_evaluations, 0);
    assert!(r.phases[2].work.move_evaluations <= 10);
    p.validate(&r.assignment).unwrap();
    let assigned = r.assignment.values().iter().flatten().count() as i64;
    let distinct = r
        .assignment
        .values()
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>()
        .len() as i64;
    assert_eq!(
        r.score,
        Some(HardSoftScore::new(-(assigned - distinct), -distinct))
    );
    assert_eq!(p.variables()[0].domain(), &values);
}
#[test]
fn solve_mixed_model_with_sort_manner() {
    // All five public ValueSorterManner constants, paired with both descending
    // entity modes. This fixture has both comparators, so IF_AVAILABLE sorts too.
    for entity_mode in ["DESCENDING", "DESCENDING_IF_AVAILABLE"] {
        for value_mode in [
            "NONE",
            "ASCENDING",
            "ASCENDING_IF_AVAILABLE",
            "DESCENDING",
            "DESCENDING_IF_AVAILABLE",
        ] {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![2i32, 1]); 4]).unwrap(),
                ListProblem::new(vec![0usize, 1], vec![ListEntity::new(vec![]); 2]).unwrap(),
            );
            let mut n = MixedConstraintNetwork::new(4, 2, 2);
            for i in 0..4 {
                n.add_constraint(format!("basic{i}"), vec![MixedScope::Basic(i)], |f| {
                    let MixedFact::Basic(v) = f[0] else { panic!() };
                    SimpleScore::new(i64::from(v.is_some()))
                })
                .unwrap();
            }
            for i in 0..2 {
                n.add_constraint(format!("list{i}"), vec![MixedScope::ListEntity(i)], |f| {
                    let MixedFact::ListEntity(v) = f[0] else {
                        panic!()
                    };
                    SimpleScore::new(if v.len() == 1 { 2 } else { 1 })
                })
                .unwrap();
            }
            // Source difficulty is entity index; descending entity order selects
            // only first-basic slots2 then0. Second basics retain construction.
            let variables = [2, 0];
            let asc = value_mode.starts_with("ASCENDING");
            let orders = if value_mode == "NONE" {
                vec![]
            } else {
                variables
                    .iter()
                    .map(|&i| {
                        BasicValueOrder::sorted_by(p.basic(), i, |a, b| {
                            if asc { a.cmp(&b) } else { b.cmp(&a) }
                        })
                        .unwrap()
                    })
                    .collect::<Vec<_>>()
            };
            let family = MixedNeighborhood {
                basic: Some(Neighborhood::Change),
                lists: None,
                basic_variables: Some(&variables),
                basic_value_orders: &orders,
                ..Default::default()
            };
            let initial = p.initial_assignment();
            let candidates = mixed_moves_configured(&p, &initial, family)
                .unwrap()
                .map(|m| m.basic)
                .collect::<Vec<_>>();
            let ordered = if asc { [1, 2] } else { [2, 1] };
            let expected = variables
                .into_iter()
                .flat_map(|variable| {
                    ordered.into_iter().map(move |v| Move::Change {
                        variable,
                        value: Some(v),
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(candidates, expected, "{entity_mode}/{value_mode}");
            let calls = Cell::new(0);
            let mut provider =
                |p: &MixedProblem<i32, usize>, a: &MixedAssignment<i32, usize>, _: u64| {
                    calls.set(calls.get() + 1);
                    mixed_moves_configured(p, a, family)
                        .unwrap()
                        .collect::<Vec<_>>()
                };
            let events = Cell::new(0);
            let mut observer = |e: SolverEvent<'_, MixedAssignment<i32, usize>, SimpleScore>| {
                if matches!(e, SolverEvent::Best(_)) {
                    events.set(events.get() + 1);
                }
            };
            let mut extensions = MixedSolverExtensions::default();
            extensions.move_providers.bind(1, &mut provider);
            extensions.events = Some(&mut observer);
            let r = solve_mixed_with_extensions(
                &p,
                &n,
                MixedSolverSequence {
                    phases: vec![
                        MixedSolverPhase::Construction(MixedConstructionConfig {
                            order: MixedConstructionOrder::BasicThenLists,
                            ..Default::default()
                        }),
                        MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                            search: LocalSearchConfig {
                                max_steps: 16,
                                acceptance: Acceptance::HillClimbing,
                                assert_scores: true,
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                },
                extensions,
            )
            .unwrap();
            assert!(calls.get() > 0 && events.get() > 0 && r.solution().is_some());
            assert!(r.assignment.basic().values().iter().all(Option::is_some));
            assert!(r.assignment.lists().lists().iter().all(|l| !l.is_empty()));
            assert_eq!(r.score, Some(SimpleScore::new(8)));
            assert!(r.phases[1].work.completed_steps <= 16);
            p.validate(&r.assignment).unwrap();
        }
    }
}
