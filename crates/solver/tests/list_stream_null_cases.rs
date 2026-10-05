// Adapted from pinned Community BavetRegressionTest list-unassignment regressions.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
type Row = Vec<ListValueRow<usize>>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Value(usize),
    Entity(usize),
}
#[derive(Clone, Copy, Debug)]
enum Shape {
    Pair,
    UnindexedPair,
    GatedPair,
    Exists,
    UnindexedExists,
    Quad,
    QuadPlain,
    QuadExists,
    QuadAbsent,
    DeepRightExists,
    DeepTri,
    DeepMapped,
    Map(bool),
    Flatten(bool),
    Group(bool),
    LargeGroup(bool),
}
#[derive(Clone, Copy)]
enum Changes {
    Cycle,
    DropReadd,
    Drop,
    Reorder,
    DropAll,
}
fn owner(row: &Row) -> usize {
    row[0].position.expect("assigned input").entity
}
fn guard(row: &Row) -> bool {
    assert!(row.iter().all(|r| r.position.is_some()));
    true
}
fn join<I: Ord + Clone + 'static, J: Ord + Clone + 'static>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    left: &StreamPort<I, Row>,
    right: &StreamPort<J, Row>,
    identity: bool,
    indexed: bool,
) -> StreamPort<(I, J), Row> {
    let pairs = b
        .join_equal(
            left,
            right,
            move |r| {
                if identity {
                    r[0].value
                } else if indexed {
                    owner(r)
                } else {
                    0
                }
            },
            move |r| {
                if identity {
                    r[0].value
                } else if indexed {
                    owner(r)
                } else {
                    0
                }
            },
        )
        .unwrap();
    b.map(&pairs, |(a, c)| a.iter().chain(c).cloned().collect::<Row>())
        .unwrap()
}
fn exists<I: Ord + Clone + 'static, J: Ord + Clone + 'static>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    left: &StreamPort<I, Row>,
    right: &StreamPort<J, Row>,
    indexed: bool,
    mode: ExistenceMode,
) -> StreamPort<I, Row> {
    b.existence(
        left,
        right,
        mode,
        move |r| if indexed { owner(r) } else { 0 },
        move |r| if indexed { owner(r) } else { 0 },
        |a, c| guard(a) && guard(c),
    )
    .unwrap()
}
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Vec<Fact>>;
struct Network {
    graph: StreamGraph<'static, SimpleScore>,
    sources: ListStreamSources<usize>,
    read: Box<Reader>,
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: ListStreamSources<usize>,
    out: StreamPort<I, Vec<Fact>>,
) -> Network {
    let data = b.dataset(&out).unwrap();
    b.score("null-conflict", &out, |_| SimpleScore::new(-1))
        .unwrap();
    Network {
        graph: b.build(),
        sources,
        read: Box::new(move |g| {
            let mut r: Vec<_> = g
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(_, r)| r.clone())
                .collect();
            r.sort();
            r
        }),
    }
}
fn finish_values<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: ListStreamSources<usize>,
    out: StreamPort<I, Row>,
) -> Network {
    let fields = b
        .map(&out, |r| {
            r.iter().map(|v| Fact::Value(v.value)).collect::<Vec<_>>()
        })
        .unwrap();
    finish(b, sources, fields)
}
fn build(shape: Shape, pruning: bool) -> Network {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    if pruning {
        b = b.with_activity_pruning();
    }
    let sources = ListStreamSources::new(&mut b);
    let assigned = b
        .filter(sources.values.port(), |r| r.position.is_some())
        .unwrap();
    let base = b.map(&assigned, |r| vec![r.clone()]).unwrap();
    let mapped = b.map(&base, Clone::clone).unwrap();
    match shape {
        Shape::Pair | Shape::UnindexedPair | Shape::GatedPair => {
            let right = if matches!(shape, Shape::GatedPair) {
                b.existence_equal(
                    &base,
                    sources.entities.port(),
                    ExistenceMode::Exists,
                    |_| (),
                    |_| (),
                )
                .unwrap()
            } else {
                mapped
            };
            let pair = join(
                &mut b,
                &base,
                &right,
                false,
                !matches!(shape, Shape::UnindexedPair),
            );
            let out = b.filter(&pair, guard).unwrap();
            finish_values(b, sources, out)
        }
        Shape::Exists | Shape::UnindexedExists => {
            let out = exists(
                &mut b,
                &base,
                &mapped,
                !matches!(shape, Shape::UnindexedExists),
                ExistenceMode::Exists,
            );
            finish_values(b, sources, out)
        }
        Shape::DeepTri | Shape::DeepMapped => {
            let twice = b.map(&mapped, Clone::clone).unwrap();
            let pairs = join(&mut b, &base, &twice, false, true);
            let pairs = b.filter(&pairs, guard).unwrap();
            if matches!(shape, Shape::DeepMapped) {
                let left = b.map(&pairs, |r| vec![r[0].clone()]).unwrap();
                let thrice = b.map(&twice, Clone::clone).unwrap();
                let joined = join(&mut b, &left, &thrice, false, true);
                let out = b.filter(&joined, guard).unwrap();
                finish_values(b, sources, out)
            } else {
                let joined = join(&mut b, &pairs, &twice, false, true);
                let out = b.filter(&joined, guard).unwrap();
                finish_values(b, sources, out)
            }
        }
        Shape::Quad
        | Shape::QuadPlain
        | Shape::QuadExists
        | Shape::QuadAbsent
        | Shape::DeepRightExists => {
            let pair = join(&mut b, &base, &mapped, false, true);
            let tri = join(&mut b, &pair, &mapped, false, true);
            if matches!(shape, Shape::DeepRightExists) {
                let right = b.map(&tri, |r| vec![r[0].clone()]).unwrap();
                let out = exists(&mut b, &base, &right, true, ExistenceMode::Exists);
                return finish_values(b, sources, out);
            }
            let quad = join(&mut b, &tri, &mapped, false, true);
            let out = match shape {
                Shape::Quad => b.filter(&quad, guard).unwrap(),
                Shape::QuadPlain => quad,
                _ => exists(
                    &mut b,
                    &quad,
                    &base,
                    true,
                    if matches!(shape, Shape::QuadAbsent) {
                        ExistenceMode::NotExists
                    } else {
                        ExistenceMode::Exists
                    },
                ),
            };
            finish_values(b, sources, out)
        }
        Shape::Map(semi) | Shape::Group(semi) => {
            let pairs = join(&mut b, &base, &mapped, true, true);
            if matches!(shape, Shape::Group(_)) {
                let groups = b
                    .group_list(&pairs, |r| (r[0].value, r[0].clone()))
                    .unwrap();
                let left = b
                    .map(&groups, |(_, members)| vec![members[0].clone()])
                    .unwrap();
                finish_relation(b, sources, left, mapped, semi)
            } else {
                let left = b.map(&pairs, |r| vec![r[0].clone()]).unwrap();
                finish_relation(b, sources, left, mapped, semi)
            }
        }
        Shape::Flatten(semi) => {
            let pairs = join(&mut b, &base, &mapped, true, true);
            let left = b
                .flat_map(&pairs, |r| {
                    let mut row = r.clone();
                    row.push(r[0].clone());
                    vec![row]
                })
                .unwrap();
            finish_relation(b, sources, left, mapped, semi)
        }
        Shape::LargeGroup(semi) => {
            let pairs = join(&mut b, &base, &mapped, true, true);
            if semi {
                let groups = b.group_list(&pairs, |r| (owner(r), r[0].clone())).unwrap();
                let visible = b
                    .existence(
                        &groups,
                        &mapped,
                        ExistenceMode::Exists,
                        |(entity, _)| *entity,
                        owner,
                        |(_, members), r| {
                            assert!(members.iter().all(|v| v.position.is_some()));
                            guard(r)
                        },
                    )
                    .unwrap();
                let fields = b
                    .map(&visible, |(entity, _)| vec![Fact::Entity(*entity)])
                    .unwrap();
                finish(b, sources, fields)
            } else {
                let groups = b
                    .group_collect_with::<_, _, _, _, CountCollector>(&pairs, |r| (owner(r), ()))
                    .unwrap();
                let joined = b
                    .join_equal(&groups, &mapped, |(entity, _)| *entity, owner)
                    .unwrap();
                let guarded = b.filter(&joined, |(_, r)| guard(r)).unwrap();
                let fields = b
                    .map(&guarded, |((entity, _), r)| {
                        vec![Fact::Entity(*entity), Fact::Value(r[0].value)]
                    })
                    .unwrap();
                finish(b, sources, fields)
            }
        }
    }
}
fn finish_relation<I: Ord + Clone + 'static, J: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: ListStreamSources<usize>,
    left: StreamPort<I, Row>,
    right: StreamPort<J, Row>,
    semi: bool,
) -> Network {
    if semi {
        let out = exists(&mut b, &left, &right, true, ExistenceMode::Exists);
        finish_values(b, sources, out)
    } else {
        let joined = join(&mut b, &left, &right, false, true);
        let out = b.filter(&joined, guard).unwrap();
        finish_values(b, sources, out)
    }
}
fn expected(shape: Shape, lists: &[Vec<usize>]) -> Vec<Vec<Fact>> {
    let all: Vec<_> = lists.iter().flatten().copied().collect();
    let mut out = Vec::new();
    let cartesian = |values: &[usize], arity: usize| {
        let mut out = vec![vec![]];
        for _ in 0..arity {
            out = out
                .into_iter()
                .flat_map(|r| {
                    values.iter().map(move |v| {
                        let mut r = r.clone();
                        r.push(Fact::Value(*v));
                        r
                    })
                })
                .collect();
        }
        out
    };
    for (entity, values) in lists.iter().enumerate() {
        match shape {
            Shape::QuadAbsent => {}
            Shape::LargeGroup(true) => {
                if !values.is_empty() {
                    out.push(vec![Fact::Entity(entity)]);
                }
            }
            Shape::LargeGroup(false) => {
                for v in values {
                    out.push(vec![Fact::Entity(entity), Fact::Value(*v)]);
                }
            }
            Shape::Exists
            | Shape::UnindexedExists
            | Shape::DeepRightExists
            | Shape::Map(true)
            | Shape::Group(true) => out.extend(cartesian(values, 1)),
            Shape::Quad | Shape::QuadPlain | Shape::QuadExists => out.extend(cartesian(values, 4)),
            Shape::DeepTri => out.extend(cartesian(values, 3)),
            Shape::DeepMapped => {
                for a in values {
                    for _ in values {
                        for c in values {
                            out.push(vec![Fact::Value(*a), Fact::Value(*c)]);
                        }
                    }
                }
            }
            Shape::Flatten(semi) => {
                for a in values {
                    if semi {
                        out.push(vec![Fact::Value(*a); 3]);
                    } else {
                        for c in values {
                            out.push(vec![
                                Fact::Value(*a),
                                Fact::Value(*a),
                                Fact::Value(*a),
                                Fact::Value(*c),
                            ]);
                        }
                    }
                }
            }
            Shape::UnindexedPair => {
                for a in values {
                    for c in &all {
                        out.push(vec![Fact::Value(*a), Fact::Value(*c)]);
                    }
                }
            }
            _ => out.extend(cartesian(values, 2)),
        }
    }
    out.sort();
    out
}
fn assert_observed(
    assignment: &ListAssignment<usize>,
    graph: &StreamGraph<'_, SimpleScore>,
    score: SimpleScore,
    read: &Reader,
    shape: Shape,
    sources: &ListStreamSources<usize>,
    model: &[Vec<usize>],
) {
    assert_eq!(assignment.lists(), model);
    let wanted = expected(shape, model);
    assert_eq!(read(graph), wanted, "{shape:?}");
    assert_eq!(score, SimpleScore::new(-(wanted.len() as i64)));
    for (_, row) in graph.source_rows(&sources.values).unwrap().iter() {
        let position = model.iter().enumerate().find_map(|(entity, list)| {
            list.iter()
                .position(|v| *v == row.value)
                .map(|index| ListPosition { entity, index })
        });
        assert_eq!(row.position, position);
        assert_eq!(
            row.previous,
            position.and_then(|p| p.index.checked_sub(1).map(|i| model[p.entity][i]))
        );
        assert_eq!(
            row.next,
            position.and_then(|p| model[p.entity].get(p.index + 1).copied())
        );
    }
}
fn assert_state(
    d: &ListStreamScoreDirector<'_, '_, usize, SimpleScore>,
    read: &Reader,
    shape: Shape,
    sources: &ListStreamSources<usize>,
    model: &[Vec<usize>],
) {
    assert_observed(
        d.assignment(),
        d.graph(),
        d.score(),
        read,
        shape,
        sources,
        model,
    );
}
fn assign(values: impl IntoIterator<Item = usize>, entity: usize, start: usize) -> ListMove<usize> {
    ListMove::Composite(
        values
            .into_iter()
            .enumerate()
            .map(|(i, value)| ListMove::Assign {
                value,
                destination: ListPosition {
                    entity,
                    index: start + i,
                },
            })
            .collect(),
    )
}
fn run(shape: Shape, changes: Changes) {
    let count = if matches!(shape, Shape::LargeGroup(_)) {
        10
    } else {
        2
    };
    let entity_count = if matches!(shape, Shape::LargeGroup(true)) {
        2
    } else {
        1
    };
    let problem = ListProblem::new(
        (0..count).collect(),
        (0..entity_count).map(|_| ListEntity::new(vec![])).collect(),
    )
    .unwrap()
    .allowing_unassigned();
    let Network {
        graph,
        sources,
        read,
    } = build(shape, false);
    let source_view = sources.clone();
    let mut model = vec![vec![]; entity_count];
    let mut d =
        ListStreamScoreDirector::new(&problem, graph, sources, problem.initial_assignment())
            .unwrap();
    assert_state(&d, read.as_ref(), shape, &source_view, &model);
    if matches!(changes, Changes::Cycle) {
        d.apply(&assign([0], 0, 0)).unwrap().commit();
        model[0] = vec![0];
        assert_state(&d, read.as_ref(), shape, &source_view, &model);
        for value in [1, 0] {
            d.apply(&ListMove::Composite(vec![
                ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 1,
                },
                assign([value], 0, 0),
            ]))
            .unwrap()
            .commit();
            model[0] = vec![value];
            assert_state(&d, read.as_ref(), shape, &source_view, &model);
        }
    } else {
        for (entity, target) in model.iter_mut().enumerate() {
            let size = count / entity_count;
            *target = (entity * size..(entity + 1) * size).collect();
            d.apply(&assign(entity * size..(entity + 1) * size, entity, 0))
                .unwrap()
                .commit();
        }
        assert_state(&d, read.as_ref(), shape, &source_view, &model);
        let change = if matches!(changes, Changes::Reorder) {
            ListMove::Swap {
                left: ListPosition {
                    entity: 0,
                    index: 0,
                },
                right: ListPosition {
                    entity: 0,
                    index: 1,
                },
            }
        } else {
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: if matches!(changes, Changes::DropAll) {
                    count
                } else {
                    1
                },
            }
        };
        d.apply(&change).unwrap().commit();
        match changes {
            Changes::Reorder => model[0].swap(0, 1),
            Changes::DropAll => model[0].clear(),
            _ => {
                model[0].remove(0);
            }
        }
        assert_state(&d, read.as_ref(), shape, &source_view, &model);
        if matches!(changes, Changes::DropReadd) {
            d.apply(&assign([0], 0, 1)).unwrap().commit();
            model[0].push(0);
            assert_state(&d, read.as_ref(), shape, &source_view, &model);
        }
    }
    d.assert_consistent().unwrap();
}

#[test]
fn filtering_join_null_conflict_right() {
    run(Shape::Pair, Changes::Cycle);
}

#[test]
fn filtering_join_null_conflict_right_unindexed() {
    run(Shape::UnindexedPair, Changes::Cycle);
}

#[test]
fn filtering_join_null_conflict_right_via_if_exists() {
    run(Shape::GatedPair, Changes::Cycle);
}

#[test]
fn filtering_join_null_conflict_right_unassign_one() {
    run(Shape::Pair, Changes::DropReadd);
}

#[test]
fn filtering_join_null_conflict_right_quad_join_unassign_one() {
    run(Shape::Quad, Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_behind_mapped_deferring_join_unassign_one() {
    run(Shape::DeepMapped, Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_behind_deferring_join_unassign_one() {
    run(Shape::DeepTri, Changes::Drop);
}

#[test]
fn join_null_conflict_right_quad_join_unassign_one_non_filtering() {
    run(Shape::QuadPlain, Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_quad_join_reorder() {
    run(Shape::Quad, Changes::Reorder);
}

#[test]
fn filtering_if_exists_null_conflict_right() {
    run(Shape::Exists, Changes::Cycle);
}

#[test]
fn filtering_if_exists_null_conflict_right_unindexed() {
    run(Shape::UnindexedExists, Changes::Cycle);
}

#[test]
fn filtering_if_exists_null_conflict_deep_left_unassign_one() {
    run(Shape::QuadExists, Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_deep_right_unassign_one() {
    run(Shape::DeepRightExists, Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_through_quad_join_reorder() {
    run(Shape::QuadExists, Changes::Reorder);
}

#[test]
fn filtering_if_not_exists_null_conflict_deep_left_unassign_one() {
    run(Shape::QuadAbsent, Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_map_unassign_one() {
    run(Shape::Map(false), Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_through_map_unassign_one() {
    run(Shape::Map(true), Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_flatten_unassign_one() {
    run(Shape::Flatten(false), Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_through_flatten_unassign_one() {
    run(Shape::Flatten(true), Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_group_by_unassign_one() {
    run(Shape::Group(false), Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_through_group_by_unassign_one() {
    run(Shape::Group(true), Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_large_group_unassign_one() {
    run(Shape::LargeGroup(false), Changes::Drop);
}

#[test]
fn filtering_if_exists_null_conflict_through_large_group_unassign_one() {
    run(Shape::LargeGroup(true), Changes::Drop);
}

#[test]
fn filtering_join_null_conflict_through_large_group_unassign_all() {
    run(Shape::LargeGroup(false), Changes::DropAll);
}

// Original model-level regression: owners, indices, neighbors and complete matches.
#[test]
fn cross_owner_moves_and_rollback_match_independent_list_enumeration() {
    let shapes = [
        Shape::Pair,
        Shape::UnindexedPair,
        Shape::GatedPair,
        Shape::Exists,
        Shape::UnindexedExists,
        Shape::Quad,
        Shape::QuadPlain,
        Shape::QuadExists,
        Shape::QuadAbsent,
        Shape::DeepRightExists,
        Shape::DeepTri,
        Shape::DeepMapped,
        Shape::Map(false),
        Shape::Map(true),
        Shape::Flatten(false),
        Shape::Flatten(true),
        Shape::Group(false),
        Shape::Group(true),
        Shape::LargeGroup(false),
        Shape::LargeGroup(true),
    ];
    for pruning in [false, true] {
        for shape in shapes {
            let problem = ListProblem::new(
                (0..5).collect(),
                vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
            )
            .unwrap()
            .allowing_unassigned();
            let Network {
                graph,
                sources,
                read,
            } = build(shape, pruning);
            let view = sources.clone();
            let mut d = ListStreamScoreDirector::new(
                &problem,
                graph,
                sources,
                problem.initial_assignment(),
            )
            .unwrap();
            let mut model = vec![vec![], vec![]];
            for step in 0..60 {
                let value = (step * 7) % 5;
                let destination = (step / 7) % 2;
                let mut next = model.clone();
                let position = next.iter().enumerate().find_map(|(entity, list)| {
                    list.iter()
                        .position(|v| *v == value)
                        .map(|index| ListPosition { entity, index })
                });
                let change = if let Some(source) = position {
                    next[source.entity].remove(source.index);
                    if step % 3 == 0 {
                        ListMove::Unassign {
                            entity: source.entity,
                            start: source.index,
                            end: source.index + 1,
                        }
                    } else {
                        let index = (step * 3) % (next[destination].len() + 1);
                        next[destination].insert(index, value);
                        ListMove::Change {
                            source,
                            destination: ListPosition {
                                entity: destination,
                                index,
                            },
                        }
                    }
                } else {
                    let index = (step / 2) % (next[destination].len() + 1);
                    next[destination].insert(index, value);
                    ListMove::Assign {
                        value,
                        destination: ListPosition {
                            entity: destination,
                            index,
                        },
                    }
                };
                let trial = d.apply(&change).unwrap();
                assert_observed(
                    trial.assignment(),
                    trial.graph(),
                    trial.score(),
                    read.as_ref(),
                    shape,
                    &view,
                    &next,
                );
                if step % 5 == 0 {
                    drop(trial);
                } else {
                    trial.commit();
                    model = next;
                }
                assert_state(&d, read.as_ref(), shape, &view, &model);
            }
        }
    }
}
