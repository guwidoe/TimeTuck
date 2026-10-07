// Adapted from DefaultConstructionHeuristicPhaseTest and sortable testdomain fixtures.
// Apache-2.0; see compat/provenance.json. Configuration alternatives retain source order.
use std::{cell::Cell, cmp::Ordering, collections::BTreeSet};
use timetuck::*;
#[derive(Clone, Copy, Debug)]
enum Order {
    Encounter,
    Asc,
    Desc,
}
impl Order {
    fn compare(self, a: usize, b: usize) -> Ordering {
        match self {
            Self::Encounter => Ordering::Equal,
            Self::Asc => a.cmp(&b),
            Self::Desc => b.cmp(&a),
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Config {
    placement: ConstructionPlacement,
    entities: Order,
    values: Order,
    expected: [usize; 3],
    shuffle: bool,
    early: bool,
    step: bool,
}
fn configurations(list: bool, ranges: bool) -> Vec<Config> {
    use ConstructionPlacement::*;
    use Order::*;
    // Native ordering/placement equivalents; no reflective Java configuration API.
    let mut configs = vec![
        Config {
            placement: Sequential,
            entities: Desc,
            values: Encounter,
            expected: [2, 1, 0],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: Sequential,
            entities: Encounter,
            values: Asc,
            expected: [0, 1, 2],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: Sequential,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: Sequential,
            entities: Encounter,
            values: Desc,
            expected: [2, 1, 0],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: Sequential,
            entities: Desc,
            values: Desc,
            expected: [0, 1, 2],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Desc,
            expected: [0, 1, 2],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Desc,
            expected: [0, 1, 2],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Desc,
            expected: [0, 1, 2],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Desc,
            expected: [0, 1, 2],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Encounter,
            values: Desc,
            expected: [2, 1, 0],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Encounter,
            values: Desc,
            expected: [2, 1, 0],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Encounter,
            values: Asc,
            expected: [0, 1, 2],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: ValueQueue,
            entities: Encounter,
            values: Asc,
            expected: [0, 1, 2],
            shuffle: false,
            early: true,
            step: false,
        },
        Config {
            placement: Pooled,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: false,
            step: false,
        },
        Config {
            placement: Pooled,
            entities: Desc,
            values: Asc,
            expected: [2, 1, 0],
            shuffle: true,
            early: false,
            step: false,
        },
    ];
    if list {
        if ranges {
            configs.extend([
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: true,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: true,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: false,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: false,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: true,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: true,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: false,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: false,
                    early: true,
                    step: true,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: true,
                    early: true,
                    step: true,
                },
            ]);
        } else {
            configs.extend([
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: true,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Desc,
                    expected: [0, 1, 2],
                    shuffle: true,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Encounter,
                    values: Desc,
                    expected: [2, 1, 0],
                    shuffle: false,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Encounter,
                    values: Desc,
                    expected: [2, 1, 0],
                    shuffle: false,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: true,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Desc,
                    values: Asc,
                    expected: [2, 1, 0],
                    shuffle: true,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Encounter,
                    values: Asc,
                    expected: [0, 1, 2],
                    shuffle: false,
                    early: true,
                    step: false,
                },
                Config {
                    placement: ValueQueue,
                    entities: Encounter,
                    values: Asc,
                    expected: [0, 1, 2],
                    shuffle: false,
                    early: true,
                    step: false,
                },
            ]);
        }
    } else {
        configs.extend([
            Config {
                placement: Sequential,
                entities: Desc,
                values: Desc,
                expected: [0, 1, 2],
                shuffle: true,
                early: true,
                step: false,
            },
            Config {
                placement: Sequential,
                entities: Desc,
                values: Desc,
                expected: [0, 1, 2],
                shuffle: true,
                early: true,
                step: false,
            },
            Config {
                placement: Sequential,
                entities: Desc,
                values: Asc,
                expected: [2, 1, 0],
                shuffle: true,
                early: true,
                step: false,
            },
            Config {
                placement: Sequential,
                entities: Desc,
                values: Asc,
                expected: [2, 1, 0],
                shuffle: true,
                early: true,
                step: false,
            },
        ]);
    }
    assert_eq!(
        configs.len(),
        if list {
            if ranges { 28 } else { 27 }
        } else {
            23
        }
    );
    configs
}
// Exact Collections.shuffle(new Random(0)) permutations in each source generator's
// call order. IDs are immutable difficulty/strength ranks, not positions after shuffle.
fn fixture(c: Config, ranges: bool) -> ([usize; 3], Vec<Vec<usize>>) {
    if !c.shuffle {
        return ([0, 1, 2], vec![vec![0, 1, 2]; 3]);
    }
    if ranges {
        let entities = [1, 0, 2];
        let domains = [vec![2, 1, 0], vec![0, 2, 1], vec![1, 0, 2]];
        (
            entities,
            entities.iter().map(|&id| domains[id].clone()).collect(),
        )
    } else {
        ([2, 1, 0], vec![vec![0, 2, 1]; 3])
    }
}
fn phase(c: Config) -> SolverSequence<'static, HardSoftScore> {
    SolverSequence {
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            placement: c.placement,
            assert_scores: true,
            pick_early: if c.early {
                ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard
            } else {
                ConstructionPickEarly::Never
            },
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn basic_score(values: impl Iterator<Item = usize>) -> HardSoftScore {
    let assigned = values.collect::<Vec<_>>();
    let distinct = assigned.iter().collect::<BTreeSet<_>>().len();
    HardSoftScore::new(-((assigned.len() - distinct) as i64), -(distinct as i64))
}
fn list_score(len: usize) -> HardSoftScore {
    if len == 1 {
        HardSoftScore::new(-1, -10)
    } else {
        HardSoftScore::new(-11, 0)
    }
}
fn basic(ranges: bool, factory: bool) {
    for c in configurations(false, ranges) {
        for stream in [false, true] {
            let (ids, domains) = fixture(c, ranges);
            let p = FiniteProblem::new(domains.into_iter().map(Variable::new).collect()).unwrap();
            let initial = p.initial_assignment();
            let calls = Cell::new(0);
            let comparators = || {
                (
                    move |a: usize, b: usize| c.entities.compare(ids[a], ids[b]),
                    move |a: &usize, b: &usize| c.values.compare(*a, *b),
                )
            };
            let mut provider = if factory {
                SortedBasicConstruction::from_factory(
                    &p,
                    &initial,
                    c.placement,
                    OptionalConstruction::Preserve,
                    |model, a| {
                        assert!(std::ptr::eq(model, &p));
                        assert_eq!(a, &initial);
                        calls.set(calls.get() + 1);
                        comparators()
                    },
                )
            } else {
                let (e, v) = comparators();
                SortedBasicConstruction::new(&p, c.placement, OptionalConstruction::Preserve, e, v)
            };
            let r = if stream {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                solve_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<usize>)>();
                        let rows = b.collect_list(source.port(), |(_, v)| *v)?;
                        b.score("unique", &rows, |rows| {
                            basic_score(rows.iter().flatten().copied())
                        })?;
                        Ok((b.build(), source))
                    },
                    phase(c),
                    ext,
                )
                .unwrap()
            } else {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let mut n = ConstraintNetwork::new(3);
                n.add_constraint("unique", vec![0, 1, 2], |rows| {
                    basic_score(rows.iter().filter_map(|v| v.copied()))
                })
                .unwrap();
                solve_with_extensions(&p, &n, phase(c), ext).unwrap()
            };
            for (position, id) in ids.into_iter().enumerate() {
                assert_eq!(
                    r.assignment.values()[position],
                    Some(c.expected[id]),
                    "{c:?}, stream={stream}, entity={id}"
                );
            }
            assert_eq!(r.score, Some(HardSoftScore::new(0, -3)));
            assert_eq!(calls.get(), usize::from(factory));
        }
    }
}
fn list(ranges: bool, factory: bool) {
    for c in configurations(true, ranges) {
        for stream in [false, true] {
            let (ids, domains) = fixture(c, ranges);
            let p = if ranges {
                ListProblem::from_entity_ranges(
                    domains
                        .into_iter()
                        .map(|v| ListEntity::new(vec![]).with_value_range(v))
                        .collect(),
                )
                .unwrap()
            } else {
                ListProblem::new(domains[0].clone(), vec![ListEntity::new(vec![]); 3]).unwrap()
            };
            let initial = p.initial_assignment();
            let calls = Cell::new(0);
            let steps = Cell::new(0);
            let comparators = || {
                (
                    move |a: usize, b: usize| c.entities.compare(ids[a], ids[b]),
                    move |a: &usize, b: &usize| c.values.compare(*a, *b),
                )
            };
            let provider = if factory {
                SortedListConstruction::from_factory(
                    &p,
                    &initial,
                    c.placement,
                    OptionalConstruction::Preserve,
                    |model, a| {
                        assert!(std::ptr::eq(model, &p));
                        assert_eq!(a, &initial);
                        calls.set(calls.get() + 1);
                        comparators()
                    },
                )
            } else {
                let (e, v) = comparators();
                SortedListConstruction::new(&p, c.placement, OptionalConstruction::Preserve, e, v)
            };
            let mut provider: Box<
                dyn MoveProvider<ListProblem<usize>, ListAssignment<usize>, ListMove<usize>>,
            > = if c.step {
                Box::new(provider.with_step_sorting(
                    |model: &ListProblem<usize>, a: &ListAssignment<usize>| {
                        assert!(std::ptr::eq(model, &p));
                        assert_eq!(a.lists().iter().map(Vec::len).sum::<usize>(), steps.get());
                        steps.set(steps.get() + 1);
                        comparators()
                    },
                ))
            } else {
                Box::new(provider)
            };
            let r = if stream {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, provider.as_mut());
                solve_list_stream_with_extensions(
                    &p,
                    || {
                        let mut b = StreamGraphBuilder::new();
                        let sources = ListStreamSources::new(&mut b);
                        b.score("sizes", sources.entities.port(), |(_, v)| {
                            list_score(v.len())
                        })?;
                        Ok((b.build(), sources))
                    },
                    phase(c),
                    ext,
                )
                .unwrap()
            } else {
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, provider.as_mut());
                let mut n = ListConstraintNetwork::new(3, 3);
                for e in 0..3 {
                    n.add_constraint(format!("size{e}"), vec![ListScope::Entity(e)], |rows| {
                        let ListFact::Entity(v) = rows[0] else {
                            panic!()
                        };
                        list_score(v.len())
                    })
                    .unwrap();
                }
                solve_list_with_extensions(&p, &n, phase(c), ext).unwrap()
            };
            for (position, id) in ids.into_iter().enumerate() {
                assert_eq!(
                    r.assignment.list(position).unwrap(),
                    [c.expected[id]],
                    "{c:?}, stream={stream}, entity={id}"
                );
            }
            assert_eq!(r.score, Some(HardSoftScore::new(-3, -30)));
            assert_eq!(calls.get(), usize::from(factory));
            assert_eq!(steps.get(), if c.step { 3 } else { 0 });
        }
    }
}
#[test]
fn solve_basic_variable_queue_comparator() {
    basic(false, false);
}
#[test]
fn solve_basic_variable_queue_factory() {
    basic(false, true);
}
#[test]
fn solve_basic_variable_entity_range_queue_comparator() {
    basic(true, false);
}
#[test]
fn solve_basic_variable_entity_range_queue_factory() {
    basic(true, true);
}
#[test]
fn solve_list_variable_queue_comparator() {
    list(false, false);
}
#[test]
fn solve_list_variable_queue_factory() {
    list(false, true);
}
#[test]
fn solve_list_variable_entity_range_queue_comparator() {
    list(true, false);
}
#[test]
fn solve_list_variable_entity_range_queue_factory() {
    list(true, true);
}
