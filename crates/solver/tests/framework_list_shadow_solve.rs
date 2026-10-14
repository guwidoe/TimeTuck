// Adapted pinned Timefold Community TEST and declarative/list TEST fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Explicit native dependency bindings and owned Rust cloning replace CDI/Gizmo.
use timetuck::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Owner {
    name: &'static str,
    values: Vec<usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Value {
    name: &'static str,
    dependencies: Vec<usize>,
    previous: Option<usize>,
    start_time: Option<i32>,
    inconsistent: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Solution {
    owners: Vec<Owner>,
    values: Vec<Value>,
    score: Option<SimpleScore>,
}
fn input() -> Solution {
    Solution {
        owners: ["a", "b"]
            .into_iter()
            .map(|name| Owner {
                name,
                values: vec![],
            })
            .collect(),
        values: ["1", "2", "3"]
            .into_iter()
            .enumerate()
            .map(|(id, name)| Value {
                name,
                dependencies: if id == 2 { vec![0, 1] } else { vec![] },
                previous: None,
                start_time: None,
                inconsistent: false,
            })
            .collect(),
        score: None,
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<usize>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![PlanningVariableDefinition::shadow::<i32>(
            "startTime",
        )]),
    ])
    .unwrap()
}
#[derive(Clone, Debug)]
struct World {
    previous: Vec<Option<usize>>,
    assigned: Vec<bool>,
}
fn world(lists: &[Vec<usize>]) -> World {
    let mut world = World {
        previous: vec![None; 3],
        assigned: vec![false; 3],
    };
    for list in lists {
        for (index, &id) in list.iter().enumerate() {
            assert!(
                !world.assigned[id],
                "validated assignments have unique ownership"
            );
            world.assigned[id] = true;
            world.previous[id] = index.checked_sub(1).map(|i| list[i]);
        }
    }
    world
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Projection {
    starts: Vec<Option<i32>>,
    inconsistent: Vec<bool>,
    assigned: Vec<bool>,
}
impl Projection {
    fn score(&self) -> SimpleScore {
        // forEach excludes unassigned/inconsistent values. forEachUnfiltered
        // retains every inconsistent value for the separate penalty.
        SimpleScore::new(
            -(0..3)
                .filter(|&id| self.assigned[id] && !self.inconsistent[id])
                .map(|id| i64::from(self.starts[id].expect("consistent start time")))
                .sum::<i64>()
                - 1000 * self.inconsistent.iter().filter(|&&bad| bad).count() as i64,
        )
    }
}
fn shadow_builder<'s>(
    schema: &'s PlanningSchema,
    values: &[Value],
) -> (
    ShadowSessionBuilder<'s, 's, usize, World>,
    Vec<ShadowNode<i32>>,
) {
    let start = schema
        .entity::<Value>()
        .unwrap()
        .variable("startTime")
        .unwrap();
    // The source supplier returns zero without a predecessor, including for
    // unassigned values. Declared edges still participate in cycle detection.
    let mut builder = ShadowSessionBuilder::new(schema);
    let nodes: Vec<_> = (0..3)
        .map(|id| {
            builder
                .derive(start, id, move |w: &World, inputs| {
                    if w.previous[id].is_none() {
                        return Some(0);
                    }
                    let inputs = inputs.for_field::<i32>(start).unwrap();
                    Some(
                        inputs
                            .iter()
                            .filter_map(|v| v.as_ref().map(|v| **v))
                            .max()?
                            + 1,
                    )
                })
                .unwrap()
        })
        .collect();
    for (id, node) in nodes.iter().enumerate() {
        let all = nodes.clone();
        let dependencies = values[id].dependencies.clone();
        builder
            .bind_dependencies(node, move |w: &World| {
                Ok(w.previous[id]
                    .into_iter()
                    .chain(dependencies.iter().copied())
                    .map(|source| all[source].erase())
                    .collect())
            })
            .unwrap();
    }
    (builder, nodes)
}
fn projection(
    world: &World,
    session: &ShadowSession<'_, '_, usize, World>,
    nodes: &[ShadowNode<i32>],
) -> Result<Projection, ShadowSessionError> {
    let rows = nodes
        .iter()
        .map(|node| session.read(node))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Projection {
        starts: rows.iter().map(|row| row.value.copied()).collect(),
        inconsistent: rows.iter().map(|row| row.inconsistent).collect(),
        assigned: world.assigned.clone(),
    })
}
fn project(schema: &PlanningSchema, values: &[Value], lists: &[Vec<usize>]) -> Projection {
    let (builder, nodes) = shadow_builder(schema, values);
    let mut session = builder.build();
    let world = world(lists);
    session.update(&world).unwrap();
    projection(&world, &session, &nodes).unwrap()
}
fn graph<'s>(
    schema: &'s PlanningSchema,
    values: &[Value],
) -> Result<(StreamGraph<'s, SimpleScore>, ListStreamSources<usize>), GraphError> {
    let (shadows, nodes) = shadow_builder(schema, values);
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::<usize>::new(&mut builder);
    let lists = builder.collect_list(sources.entities.port(), |row| row.clone())?;
    let worlds = builder.map(&lists, |rows| {
        let mut lists = vec![vec![]; 2];
        // Stream sources publish owners separately. A transfer's private
        // intermediate rows can briefly duplicate a value; final layouts are
        // validated and unchanged by this intermediate normalization.
        let mut seen = [false; 3];
        for (owner, values) in rows.iter() {
            lists[*owner] = values
                .iter()
                .copied()
                .filter(|&id| {
                    let fresh = !seen[id];
                    seen[id] = true;
                    fresh
                })
                .collect();
        }
        world(&lists)
    })?;
    let derived = builder.map_shadows(&worlds, shadows, move |world, session| {
        projection(world, session, &nodes)
    })?;
    builder.score(
        "Minimize start time and inconsistent values",
        &derived,
        Projection::score,
    )?;
    Ok((builder.build(), sources))
}
fn solve(problem: &Solution, stream: bool) -> Solution {
    // Native borrowed input / owned output contract; cloning occurs before
    // solving and writes target the clone's concrete lists and shadow fields.
    let mut output = problem.clone();
    let model = ListProblem::new(
        (0..3).collect(),
        output
            .owners
            .iter()
            .map(|owner| ListEntity::new(owner.values.clone()))
            .collect(),
    )
    .unwrap();
    let schema = schema();
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 1000,
            best_score_limit: Some(SimpleScore::new(-1)),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(3),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 32,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    let result = if stream {
        solve_list_stream(&model, || graph(&schema, &output.values), config).unwrap()
    } else {
        let mut network = ListConstraintNetwork::new(2, 3);
        network
            .add_constraint(
                "Declarative list shadows",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |facts| {
                    let lists = facts
                        .iter()
                        .map(|fact| {
                            let ListFact::Entity(list) = fact else {
                                panic!("owner scope")
                            };
                            list.to_vec()
                        })
                        .collect::<Vec<_>>();
                    project(&schema, &output.values, &lists).score()
                },
            )
            .unwrap();
        solve_list(&model, &network, config).unwrap()
    };
    assert!(result.complete);
    assert!(result.solution().is_some()); // Source nonnull successful solve.
    assert!(result.work.evaluations <= 1000);
    model.validate(&result.assignment).unwrap();
    let fields = project(&schema, &output.values, result.assignment.lists());
    assert_eq!(result.score, Some(fields.score()));
    assert_eq!(fields, reference(&output.values, result.assignment.lists()));
    output.score = result.score;
    for (owner, list) in output.owners.iter_mut().zip(result.assignment.lists()) {
        owner.values = list.clone();
    }
    let world = world(result.assignment.lists());
    for (id, value) in output.values.iter_mut().enumerate() {
        value.previous = world.previous[id];
        value.start_time = fields.starts[id];
        value.inconsistent = fields.inconsistent[id];
    }
    output
}
// Original independent transitive-closure/topological reference. No production
// dependency evaluator, scripted score, or expected optimum inside solving.
fn reference(values: &[Value], lists: &[Vec<usize>]) -> Projection {
    let world = world(lists);
    let dependencies: Vec<Vec<usize>> = values
        .iter()
        .enumerate()
        .map(|(id, value)| {
            world.previous[id]
                .into_iter()
                .chain(value.dependencies.iter().copied())
                .collect()
        })
        .collect();
    let mut reach = [[false; 3]; 3];
    for (id, sources) in dependencies.iter().enumerate() {
        for &source in sources {
            reach[id][source] = true;
        }
    }
    for middle in 0..3 {
        for id in 0..3 {
            for source in 0..3 {
                reach[id][source] |= reach[id][middle] && reach[middle][source];
            }
        }
    }
    let inconsistent: Vec<_> = (0..3)
        .map(|id| (0..3).any(|source| reach[source][source] && (id == source || reach[id][source])))
        .collect();
    let mut starts = vec![None; 3];
    for _ in 0..3 {
        for id in 0..3 {
            if inconsistent[id]
                || !dependencies[id]
                    .iter()
                    .all(|&source| starts[source].is_some())
            {
                continue;
            }
            starts[id] = Some(if world.previous[id].is_none() {
                0
            } else {
                dependencies[id]
                    .iter()
                    .map(|&source| starts[source].unwrap())
                    .max()
                    .unwrap()
                    + 1
            });
        }
    }
    Projection {
        starts,
        inconsistent,
        assigned: world.assigned,
    }
}
#[test]
fn declarative_shadow_solution_list_solve() {
    for stream in [false, true] {
        let problem = input();
        let saved = problem.clone();
        let solved = solve(&problem, stream);
        assert!(!std::ptr::eq(&solved, &problem));
        assert_eq!(solved.score, Some(SimpleScore::new(-1)));
        assert!(!std::ptr::eq(&solved.owners[0], &problem.owners[1]));
        for (new, old) in solved.owners.iter().zip(&problem.owners) {
            assert!(!std::ptr::eq(new, old));
        }
        assert_eq!(problem, saved);
        let mut ids: Vec<_> = solved
            .owners
            .iter()
            .flat_map(|owner| owner.values.iter().copied())
            .collect();
        ids.sort();
        assert_eq!(ids, [0, 1, 2]);
        assert!(
            solved
                .values
                .iter()
                .all(|value| !value.inconsistent && value.start_time.is_some())
        );
    }
}
#[test]
fn every_fixture_layout_matches_reference_and_cycles_recover() {
    let problem = input();
    let schema = schema();
    let mut best = SimpleScore::new(i64::MIN);
    let mut cyclic = 0;
    for a in 0..3 {
        for b in 0..3 {
            for c in 0..3 {
                if a == b || a == c || b == c {
                    continue;
                }
                let permutation = [a, b, c];
                for split in 0..=3 {
                    let lists = vec![permutation[..split].to_vec(), permutation[split..].to_vec()];
                    let actual = project(&schema, &problem.values, &lists);
                    assert_eq!(actual, reference(&problem.values, &lists));
                    best = best.max(actual.score());
                    cyclic += usize::from(actual.inconsistent.iter().any(|&bad| bad));
                    let model = ListProblem::new(
                        (0..3).collect(),
                        lists.iter().cloned().map(ListEntity::new).collect(),
                    )
                    .unwrap();
                    let (graph, sources) = graph(&schema, &problem.values).unwrap();
                    let director = ListStreamScoreDirector::new(
                        &model,
                        graph,
                        sources,
                        model.initial_assignment(),
                    )
                    .unwrap();
                    assert_eq!(director.score(), actual.score());
                }
            }
        }
    }
    assert_eq!(best, SimpleScore::new(-1));
    assert!(cyclic > 0);
    let (builder, nodes) = shadow_builder(&schema, &problem.values);
    let mut session = builder.build();
    for lists in [
        vec![vec![2, 0], vec![1]],
        vec![vec![0, 2], vec![1]],
        vec![vec![2, 0], vec![1]],
    ] {
        let world = world(&lists);
        session.update(&world).unwrap();
        assert_eq!(
            projection(&world, &session, &nodes).unwrap(),
            reference(&problem.values, &lists)
        );
    }
}
