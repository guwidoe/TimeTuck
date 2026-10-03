// Adapted from Timefold Community 2.7.0 DependencyValuesShadowVariableTest and
// testdomain/shadow/dependency fixtures, Apache-2.0. See compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::*;
type Key = (&'static str, &'static str);
#[derive(Clone, Debug, Eq, PartialEq)]
struct Times {
    start: Option<i64>,
    end: Option<i64>,
    inconsistent: bool,
}
struct Schedule {
    names: Vec<&'static str>,
    durations: Vec<i64>,
    dependencies: Vec<Vec<usize>>,
    starts: Vec<i64>,
    plan: DependencyGraph<Key>,
}
impl Schedule {
    fn new(
        names: Vec<&'static str>,
        durations: Vec<i64>,
        dependencies: Vec<Vec<usize>>,
        starts: Vec<i64>,
    ) -> Result<Self, DependencyError<Key>> {
        let mut declarations = vec![];
        for (i, &name) in names.iter().enumerate() {
            declarations.push((
                (name, "startTime"),
                dependencies[i]
                    .iter()
                    .map(|&j| (names[j], "endTime"))
                    .collect(),
            ));
            declarations.push(((name, "endTime"), vec![(name, "startTime")]));
        }
        let plan = DependencyGraph::new(declarations)?;
        Ok(Self {
            names,
            durations,
            dependencies,
            starts,
            plan,
        })
    }
    fn project(&self, lists: &[Vec<usize>]) -> Vec<Times> {
        let mut owners = vec![None; self.names.len()];
        let mut previous = vec![None; self.names.len()];
        let mut links = vec![];
        for (e, list) in lists.iter().enumerate() {
            for (i, &v) in list.iter().enumerate() {
                owners[v] = Some(e);
                if i > 0 {
                    let p = list[i - 1];
                    previous[v] = Some(p);
                    links.push(((self.names[v], "startTime"), (self.names[p], "endTime")));
                }
            }
        }
        let ids: BTreeMap<_, _> = self
            .names
            .iter()
            .enumerate()
            .map(|(i, &name)| (name, i))
            .collect();
        let values = self
            .plan
            .evaluate(&links, |&(name, phase), inputs| {
                let i = ids[name];
                let read = |key: Key| inputs.iter().find(|(k, _)| **k == key).unwrap().1.value;
                if phase == "endTime" {
                    return read((name, "startTime"))
                        .map(|v: i64| v.checked_add(self.durations[i]).unwrap());
                }
                let entity = owners[i]?;
                let mut ready = previous[i].map_or(Some(self.starts[entity]), |p| {
                    read((self.names[p], "endTime"))
                });
                for &d in &self.dependencies[i] {
                    let end = read((self.names[d], "endTime"))?;
                    ready = ready.max(Some(end));
                }
                ready
            })
            .unwrap();
        self.names
            .iter()
            .map(|&name| {
                let start = &values[&(name, "startTime")];
                let end = &values[&(name, "endTime")];
                Times {
                    start: start.value,
                    end: end.value,
                    inconsistent: start.inconsistent || end.inconsistent,
                }
            })
            .collect()
    }
    fn score(&self, lists: &[Vec<usize>]) -> HardSoftScore {
        let times = self.project(lists);
        let hard = -(times.iter().filter(|t| t.inconsistent).count() as i64);
        let soft = -lists
            .iter()
            .enumerate()
            .map(|(e, list)| {
                list.iter()
                    .filter_map(|&v| times[v].end.map(|end| end - self.starts[e]))
                    .sum::<i64>()
            })
            .sum::<i64>();
        HardSoftScore::new(hard, soft)
    }
    fn network(&self) -> ListConstraintNetwork<'_, usize, HardSoftScore> {
        let mut n = ListConstraintNetwork::new(self.starts.len(), self.names.len());
        n.add_constraint(
            "schedule",
            (0..self.starts.len()).map(ListScope::Entity).collect(),
            |facts| {
                let lists = facts
                    .iter()
                    .map(|f| {
                        let ListFact::Entity(list) = f else {
                            unreachable!()
                        };
                        list.to_vec()
                    })
                    .collect::<Vec<_>>();
                self.score(&lists)
            },
        )
        .unwrap();
        n
    }
    fn graph(
        &self,
    ) -> Result<(StreamGraph<'_, HardSoftScore>, ListStreamSources<usize>), GraphError> {
        let mut b = StreamGraphBuilder::new();
        let sources = ListStreamSources::new(&mut b);
        let lists = b.group_list(sources.entities.port(), |row| ((), row.clone()))?;
        b.score("schedule", &lists, |(_, snapshot)| {
            let mut lists = vec![vec![]; self.starts.len()];
            for (e, list) in snapshot.iter() {
                lists[*e] = list.clone();
            }
            self.score(&lists)
        })?;
        Ok((b.build(), sources))
    }
}
#[test]
fn test_solve() {
    let s = Schedule::new(
        vec![
            "A", "B", "C", "D", "(A,B)", "(B,C)", "(C,D)", "(A,D)", "(A,B)+C", "(B,C)+D",
            "(C,D)+A", "(A,D)+B",
        ],
        vec![30, 10, 50, 120, 40, 10, 40, 40, 40, 10, 40, 40],
        vec![
            vec![],
            vec![],
            vec![],
            vec![],
            vec![0, 1],
            vec![1, 2],
            vec![2, 3],
            vec![0, 3],
            vec![4, 2],
            vec![5, 3],
            vec![6, 0],
            vec![7, 1],
        ],
        vec![0, 30, 60, 90],
    )
    .unwrap();
    let p = ListProblem::new((0..12).collect(), vec![ListEntity::new(vec![]); 4]).unwrap();
    let config = SolverConfig {
        limits: SolverLimits {
            max_move_evaluations: Some(1000),
            ..Default::default()
        },
        construction: ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            assert_scores: true,
            ..Default::default()
        }],
    };
    let scoped = solve_list(&p, &s.network(), config.clone()).unwrap();
    let streamed = solve_list_stream(&p, || s.graph(), config).unwrap();
    assert_eq!(scoped.assignment, streamed.assignment);
    assert_eq!(scoped.score, streamed.score);
    for result in [scoped, streamed] {
        assert!(result.complete);
        assert!(result.work.move_evaluations <= 1000);
        let times = s.project(result.assignment.lists());
        for (i, deps) in s.dependencies.iter().enumerate() {
            for &d in deps {
                assert!(times[i].start.unwrap() >= times[d].end.unwrap());
            }
        }
    }
}
#[test]
fn test_loop_status_of_entity_is_updated_even_if_no_variables_on_the_entity_changed() {
    let s = Schedule::new(
        vec!["A", "B", "C"],
        vec![300, 360, 420],
        vec![vec![], vec![], vec![0, 1]],
        vec![0, 0, 0],
    )
    .unwrap();
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![1, 0]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let n = s.network();
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let m = ListMove::Assign {
        value: 2,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    };
    {
        let t = d.apply(&m).unwrap();
        let times = s.project(t.assignment().lists());
        assert_eq!(times[2].start, None);
        assert_eq!(times[2].end, None);
        assert!(times[2].inconsistent);
    }
    let times = s.project(d.assignment().lists());
    assert_eq!(times[2].start, None);
    assert_eq!(times[2].end, None);
    assert!(!times[2].inconsistent);
    d.assert_consistent().unwrap();
    let (g, sources) = s.graph().unwrap();
    let mut d = ListStreamScoreDirector::new(&p, g, sources, p.initial_assignment()).unwrap();
    let before = d.score();
    {
        let t = d.apply(&m).unwrap();
        let times = s.project(t.assignment().lists());
        assert_eq!(times[2].start, None);
        assert_eq!(times[2].end, None);
        assert!(times[2].inconsistent);
    }
    let times = s.project(d.assignment().lists());
    assert_eq!(times[2].start, None);
    assert_eq!(times[2].end, None);
    assert!(!times[2].inconsistent);
    assert_eq!(d.score(), before);
    d.assert_consistent().unwrap();
}
#[test]
fn fails_if_two_values_depend_on_each_other() {
    let error = Schedule::new(
        vec!["Value A", "Value B", "Value C"],
        vec![30, 10, 50],
        vec![vec![], vec![2], vec![1]],
        vec![0],
    )
    .err()
    .unwrap();
    assert!(matches!(error, DependencyError::FixedCycle(_)));
    let message = error.to_string();
    for fragment in [
        "There are fixed dependency loops in the graph for variables",
        "startTime",
        "endTime",
        "Value B",
        "Value C",
    ] {
        assert!(message.contains(fragment));
    }
    assert!(!message.contains("Value A"));
}
