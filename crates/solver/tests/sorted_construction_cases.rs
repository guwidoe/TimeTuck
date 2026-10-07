// Adapted from DefaultConstructionHeuristicPhaseTest factory-sorting methods,
// their configuration generators, nested score calculators and testdomain fixtures.
// Apache-2.0; see compat/provenance.json.
use std::{cell::Cell, cmp::Ordering, collections::BTreeSet};
use timetuck::*;
fn descending(a: usize, b: usize) -> Ordering {
    b.cmp(&a)
}
fn same(_: usize, _: usize) -> Ordering {
    Ordering::Equal
}
fn value_descending(a: &String, b: &String) -> Ordering {
    let code = |s: &String| s.rsplit(' ').next().unwrap().parse::<usize>().unwrap();
    code(b).cmp(&code(a))
}
fn value_same(_: &String, _: &String) -> Ordering {
    Ordering::Equal
}
fn values() -> Vec<String> {
    (0..3).map(|i| format!("Generated Value {i}")).collect()
}
fn phase() -> SolverSequence<'static, SimpleScore> {
    SolverSequence {
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        })],
        ..Default::default()
    }
}
#[test]
fn solve_entity_factory_sorting() {
    // The generator contains four entries: two factory and two comparator configurations.
    for factory in [true, true, false, false] {
        for stream in [false, true] {
            let p = ListProblem::new(values(), vec![ListEntity::new(vec![]); 3]).unwrap();
            let initial = p.initial_assignment();
            let calls = Cell::new(0);
            let mut provider = if factory {
                SortedListConstruction::from_factory(
                    &p,
                    &initial,
                    ConstructionPlacement::ValueQueue,
                    OptionalConstruction::Preserve,
                    |model, a| {
                        assert!(std::ptr::eq(model, &p));
                        assert_eq!(a, &initial);
                        calls.set(calls.get() + 1);
                        (
                            descending as fn(usize, usize) -> Ordering,
                            value_same as fn(&String, &String) -> Ordering,
                        )
                    },
                )
            } else {
                SortedListConstruction::new(
                    &p,
                    ConstructionPlacement::ValueQueue,
                    OptionalConstruction::Preserve,
                    descending,
                    value_same,
                )
            };
            let result = if stream {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                solve_list_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let sources = ListStreamSources::new(&mut b);
                        b.score("size", sources.entities.port(), |(_, list)| {
                            SimpleScore::new(if list.len() <= 1 { -2 } else { -11 })
                        })?;
                        Ok((b.build(), sources))
                    },
                    phase(),
                    ext,
                )
                .unwrap()
            } else {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let mut n = ListConstraintNetwork::new(3, 3);
                for e in 0..3 {
                    n.add_constraint(format!("size{e}"), vec![ListScope::Entity(e)], |f| {
                        let ListFact::Entity(list) = f[0] else {
                            panic!()
                        };
                        SimpleScore::new(if list.len() <= 1 { -2 } else { -11 })
                    })
                    .unwrap();
                }
                solve_list_with_extensions(&p, &n, phase(), ext).unwrap()
            };
            let best = result;
            for (e, v) in [2, 1, 0].into_iter().enumerate() {
                assert_eq!(
                    best.assignment.list(e).unwrap(),
                    [format!("Generated Value {v}")]
                );
            }
            assert_eq!(best.score, Some(SimpleScore::new(-6)));
            assert_eq!(calls.get(), usize::from(factory));
        }
    }
}
#[test]
fn solve_value_factory_sorting() {
    for factory in [true, true, false, false] {
        for stream in [false, true] {
            let p = FiniteProblem::new(vec![Variable::new(values()); 3]).unwrap();
            let initial = p.initial_assignment();
            let calls = Cell::new(0);
            let mut provider = if factory {
                SortedBasicConstruction::from_factory(
                    &p,
                    &initial,
                    ConstructionPlacement::Sequential,
                    OptionalConstruction::Preserve,
                    |model, a| {
                        assert!(std::ptr::eq(model, &p));
                        assert_eq!(a, &initial);
                        calls.set(calls.get() + 1);
                        (
                            same as fn(usize, usize) -> Ordering,
                            value_descending as fn(&String, &String) -> Ordering,
                        )
                    },
                )
            } else {
                SortedBasicConstruction::new(
                    &p,
                    ConstructionPlacement::Sequential,
                    OptionalConstruction::Preserve,
                    same,
                    value_descending,
                )
            };
            let result = if stream {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                solve_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<String>)>();
                        let all = b.collect_list(source.port(), |(_, v)| v.clone())?;
                        b.score("repeated", &all, |rows| {
                            let assigned = rows.iter().flatten().collect::<Vec<_>>();
                            SimpleScore::new(
                                -((assigned.len()
                                    - assigned.iter().copied().collect::<BTreeSet<_>>().len())
                                    as i64),
                            )
                        })?;
                        Ok((b.build(), source))
                    },
                    phase(),
                    ext,
                )
                .unwrap()
            } else {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let mut n = ConstraintNetwork::new(3);
                n.add_constraint("repeated", vec![0, 1, 2], |rows| {
                    let assigned = rows.iter().flatten().collect::<Vec<_>>();
                    SimpleScore::new(
                        -((assigned.len() - assigned.iter().copied().collect::<BTreeSet<_>>().len())
                            as i64),
                    )
                })
                .unwrap();
                solve_with_extensions(&p, &n, phase(), ext).unwrap()
            };
            let best = result;
            assert_eq!(
                best.assignment.values(),
                &[
                    Some("Generated Value 2".into()),
                    Some("Generated Value 1".into()),
                    Some("Generated Value 0".into())
                ]
            );
            assert_eq!(best.score, Some(SimpleScore::ZERO));
            assert_eq!(calls.get(), usize::from(factory));
        }
    }
}
