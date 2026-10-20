//! Original bounded mixed route/depot workload; no upstream method credit.
use std::error::Error;
use timetuck::*;

/// Explicit application graph policy; ordinary declarations remain the default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum StreamGraphPolicy {
    #[default]
    OrdinaryJoin,
    OwnerLinks,
}
impl StreamGraphPolicy {
    // This support module also serves oracle/test clients without JSON output.
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::OrdinaryJoin => "ordinary-join",
            Self::OwnerLinks => "owner-links-shared-fixed-terminal-v1",
        }
    }
}
pub type OwnerPair = ((usize, Option<usize>), (usize, Vec<usize>));
pub type OwnerLinkId = ((usize, usize), usize);
/// Both branches retain complete inspectable payload/score matches.
#[allow(dead_code)]
pub enum OwnerRouteConstraint {
    Ordinary(StreamConstraint<(usize, usize), OwnerPair>),
    OwnerLinks(StreamConstraint<OwnerLinkId, OwnerPair>),
}
pub fn declare_owner_routes<'a>(
    graph: &mut StreamGraphBuilder<'a, HardSoftScore>,
    basic: &StreamPort<usize, (usize, Option<usize>)>,
    entities: &StreamPort<usize, (usize, Vec<usize>)>,
    owners: usize,
    policy: StreamGraphPolicy,
    evaluate: impl Fn(&OwnerPair) -> HardSoftScore + 'a,
) -> Result<OwnerRouteConstraint, GraphError> {
    match policy {
        StreamGraphPolicy::OrdinaryJoin => {
            let joined = graph.join_equal(basic, entities, |(id, _)| *id, |(id, _)| *id)?;
            graph
                .score("coupled depot routes", &joined, evaluate)
                .map(OwnerRouteConstraint::Ordinary)
        }
        StreamGraphPolicy::OwnerLinks => {
            // Director source IDs and payload owner IDs agree for every real row.
            let joined = graph.join_fixed_links_batched_shared(
                basic,
                entities,
                (0..owners).map(|owner| (owner, owner, owner)),
                |_, _, _| true,
                |_, left, right| (*left, right.clone()),
            )?;
            graph
                .score_fixed(
                    "coupled depot routes",
                    &joined,
                    (0..owners).map(|owner| ((owner, owner), owner)),
                    evaluate,
                )
                .map(OwnerRouteConstraint::OwnerLinks)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Case {
    pub id: String,
    pub stream_graph_policy: StreamGraphPolicy,
    pub points: Vec<[i64; 2]>,
    pub depots: Vec<[i64; 2]>,
    pub capacities: Vec<usize>,
    pub initial_depots: Vec<usize>,
    pub initial_routes: Vec<Vec<usize>>,
    pub pins: Vec<bool>,
    pub prefixes: Vec<usize>,
    pub steps: u64,
    pub evaluations: u64,
    pub quality_floor: Option<HardSoftScore>,
    pub problem: MixedProblem<usize, usize>,
}
fn numbers(s: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    Ok(s.split(',').map(str::parse).collect::<Result<_, _>>()?)
}
fn parse_points(s: &str) -> Result<Vec<[i64; 2]>, Box<dyn Error>> {
    s.split(',')
        .map(|p| {
            let v = p
                .split(':')
                .map(str::parse)
                .collect::<Result<Vec<i64>, _>>()?;
            if v.len() != 2 || v.iter().any(|v| !(-100_000..=100_000).contains(v)) {
                return Err("point bounds".into());
            }
            Ok([v[0], v[1]])
        })
        .collect()
}
impl Case {
    pub fn parse(line: &str) -> Result<Self, Box<dyn Error>> {
        let f: Vec<_> = line.split('\t').collect();
        if ![10, 11].contains(&f.len())
            || f[0].is_empty()
            || !f[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_')
        {
            return Err("ten fields plus optional quality floor required".into());
        }
        let quality_floor = f
            .get(10)
            .map(|text| -> Result<_, Box<dyn Error>> {
                let (hard, soft) = text.split_once(':').ok_or("quality floor hard:soft")?;
                Ok(HardSoftScore::new(hard.parse()?, soft.parse()?))
            })
            .transpose()?;
        let points = parse_points(f[1])?;
        let depots = parse_points(f[2])?;
        let capacities = numbers(f[3])?;
        let initial_depots = numbers(f[4])?;
        let initial_routes: Vec<_> = f[5]
            .split(';')
            .map(|r| if r == "-" { Ok(vec![]) } else { numbers(r) })
            .collect::<Result<_, _>>()?;
        let pins = f[6]
            .bytes()
            .map(|v| match v {
                b'0' => Ok(false),
                b'1' => Ok(true),
                _ => Err("pin"),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let prefixes = numbers(f[7])?;
        let steps = f[8].parse()?;
        let evaluations = f[9].parse()?;
        let owners = capacities.len();
        if owners == 0
            || owners > 8
            || points.len() > 48
            || depots.is_empty()
            || depots.len() > 8
            || steps == 0
            || steps > 128
            || evaluations == 0
            || evaluations > 2_000_000
            || [
                initial_depots.len(),
                initial_routes.len(),
                pins.len(),
                prefixes.len(),
            ]
            .iter()
            .any(|n| *n != owners)
            || initial_depots.iter().any(|id| *id >= depots.len())
            || prefixes
                .iter()
                .zip(&initial_routes)
                .any(|(p, r)| *p > r.len())
            || pins.iter().zip(&prefixes).any(|(pin, p)| *pin && *p != 0)
        {
            return Err("shape or limit".into());
        }
        let basic = FiniteProblem::new(
            (0..owners)
                .map(|i| {
                    Variable::new((0..depots.len()).collect())
                        .with_initial(initial_depots[i])
                        .with_pinned(pins[i])
                })
                .collect(),
        )?;
        let lists = ListProblem::new(
            (0..points.len()).collect(),
            (0..owners)
                .map(|i| {
                    ListEntity::new(initial_routes[i].clone())
                        .with_pinned(pins[i])
                        .with_pinned_prefix(prefixes[i])
                })
                .collect(),
        )?;
        let problem = MixedProblem::new(basic, lists);
        if !problem.is_complete(&problem.initial_assignment()) {
            return Err("complete initial input required".into());
        }
        let case = Self {
            id: f[0].into(),
            stream_graph_policy: StreamGraphPolicy::OrdinaryJoin,
            points,
            depots,
            capacities,
            initial_depots,
            initial_routes,
            pins,
            prefixes,
            steps,
            evaluations,
            quality_floor,
            problem,
        };
        if case.score(&case.problem.initial_assignment()).hard() != 0 {
            return Err("feasible warm start required".into());
        }
        Ok(case)
    }
    pub fn score(&self, a: &MixedAssignment<usize, usize>) -> HardSoftScore {
        a.lists()
            .lists()
            .iter()
            .enumerate()
            .fold(HardSoftScore::ZERO, |s, (id, r)| {
                s + self.route_score(a.basic().values()[id].unwrap(), r, id)
            })
    }
    pub fn route_score(&self, depot: usize, route: &[usize], owner: usize) -> HardSoftScore {
        let home = self.depots[depot];
        let mut previous = home;
        let mut distance = 0;
        for &id in route {
            let point = self.points[id];
            distance += (point[0] - previous[0]).abs() + (point[1] - previous[1]).abs();
            previous = point;
        }
        distance += (home[0] - previous[0]).abs() + (home[1] - previous[1]).abs();
        HardSoftScore::new(
            -(route.len().saturating_sub(self.capacities[owner]) as i64),
            -distance,
        )
    }
    pub fn validate(
        &self,
        a: &MixedAssignment<usize, usize>,
        s: HardSoftScore,
    ) -> Result<(), Box<dyn Error>> {
        self.problem.validate(a)?;
        if !self.problem.is_complete(a)
            || self.score(a) != s
            || s < self.score(&self.problem.initial_assignment())
            || s.hard() != 0
            || self.quality_floor.is_some_and(|floor| s < floor)
        {
            return Err("result completeness, score or quality".into());
        }
        for owner in 0..self.capacities.len() {
            let after = &a.lists().lists()[owner];
            let before = &self.initial_routes[owner];
            if self.pins[owner]
                && (after != before
                    || a.basic().values()[owner] != Some(self.initial_depots[owner]))
            {
                return Err("whole owner pin".into());
            }
            if after[..self.prefixes[owner]] != before[..self.prefixes[owner]] {
                return Err("prefix pin".into());
            }
        }
        Ok(())
    }
}

pub fn run(
    case: &Case,
    stream: bool,
) -> Result<MixedLocalSearchResult<usize, usize, HardSoftScore>, Box<dyn Error>> {
    let config = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_steps: case.steps,
            max_evaluations: case.evaluations,
            max_move_evaluations: Some(case.evaluations - 1),
            selection: MoveSelection::BestAccepted,
            move_order: MoveOrder::Original,
            ..Default::default()
        },
        joint_moves: true,
    };
    let initial = case.problem.initial_assignment();
    let result = if stream {
        let mut graph = StreamGraphBuilder::new();
        let source = MixedStreamSources::new(&mut graph);
        declare_owner_routes(
            &mut graph,
            source.basic.port(),
            source.lists.entities.port(),
            case.capacities.len(),
            case.stream_graph_policy,
            |((owner, depot), (_, route))| case.route_score(depot.unwrap(), route, *owner),
        )?;
        mixed_stream_local_search_from(&case.problem, graph.build(), source, initial, config)?
    } else {
        let owners = case.capacities.len();
        let mut network = MixedConstraintNetwork::new(owners, owners, case.points.len());
        for owner in 0..owners {
            network.add_constraint(
                format!("route {owner}"),
                vec![MixedScope::Basic(owner), MixedScope::ListEntity(owner)],
                move |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    case.route_score(*depot.unwrap(), route, owner)
                },
            )?;
        }
        mixed_local_search_from(&case.problem, &network, initial, config)?
    };
    Ok(result)
}

/// Discard mutable warm-start choices, retaining only whole/prefix pins.
pub fn cold_problem(case: &Case) -> Result<MixedProblem<usize, usize>, Box<dyn Error>> {
    let basic = FiniteProblem::new(
        (0..case.capacities.len())
            .map(|i| {
                let variable = Variable::new((0..case.depots.len()).collect());
                if case.pins[i] {
                    variable
                        .with_initial(case.initial_depots[i])
                        .with_pinned(true)
                } else {
                    variable
                }
            })
            .collect(),
    )?;
    let lists = ListProblem::new(
        (0..case.points.len()).collect(),
        (0..case.capacities.len())
            .map(|i| {
                let kept = if case.pins[i] {
                    case.initial_routes[i].len()
                } else {
                    case.prefixes[i]
                };
                ListEntity::new(case.initial_routes[i][..kept].to_vec())
                    .with_pinned(case.pins[i])
                    .with_pinned_prefix(case.prefixes[i])
            })
            .collect(),
    )?;
    Ok(MixedProblem::new(basic, lists))
}

fn partial_route_score(
    case: &Case,
    depot: Option<usize>,
    route: &[usize],
    owner: usize,
) -> HardSoftScore {
    depot.map_or_else(
        || {
            HardSoftScore::new(
                -1 - route.len().saturating_sub(case.capacities[owner]) as i64,
                0,
            )
        },
        |depot| case.route_score(depot, route, owner),
    )
}

/// Construct both variable kinds and improve them under one solve-wide budget.
pub fn run_cold(
    case: &Case,
    stream: bool,
) -> Result<SolverResult<MixedAssignment<usize, usize>, HardSoftScore>, Box<dyn Error>> {
    let problem = cold_problem(case)?;
    let plan = MixedSolverSequence {
        limits: SolverLimits {
            max_evaluations: case.evaluations,
            ..Default::default()
        },
        phases: vec![
            MixedSolverPhase::Construction(MixedConstructionConfig::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: case.steps,
                    max_evaluations: case.evaluations,
                    selection: MoveSelection::BestAccepted,
                    move_order: MoveOrder::Original,
                    ..Default::default()
                },
                joint_moves: true,
            }),
        ],
    };
    let result = if stream {
        solve_mixed_stream(
            &problem,
            || {
                let mut graph = StreamGraphBuilder::new();
                let source = MixedStreamSources::new(&mut graph);
                declare_owner_routes(
                    &mut graph,
                    source.basic.port(),
                    source.lists.entities.port(),
                    case.capacities.len(),
                    case.stream_graph_policy,
                    |((owner, depot), (_, route))| partial_route_score(case, *depot, route, *owner),
                )?;
                graph.score(
                    "unassigned customers",
                    source.lists.values.port(),
                    |value| HardSoftScore::new(-i64::from(value.position.is_none()), 0),
                )?;
                Ok((graph.build(), source))
            },
            plan,
        )?
    } else {
        let owners = case.capacities.len();
        let mut network = MixedConstraintNetwork::new(owners, owners, case.points.len());
        for owner in 0..owners {
            network.add_constraint(
                format!("route {owner}"),
                vec![MixedScope::Basic(owner), MixedScope::ListEntity(owner)],
                move |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    partial_route_score(case, depot.copied(), route, owner)
                },
            )?;
        }
        for value in 0..case.points.len() {
            network.add_constraint(
                format!("unassigned customer {value}"),
                vec![MixedScope::ListValue(value)],
                |facts| {
                    let MixedFact::ListValue { element, .. } = &facts[0] else {
                        unreachable!()
                    };
                    HardSoftScore::new(-i64::from(element.is_none()), 0)
                },
            )?;
        }
        solve_mixed(&problem, &network, plan)?
    };
    Ok(result)
}

/// Explicit component-first search followed by joint refinement under shared budgets.
pub fn run_component_refinement(
    case: &Case,
    stream: bool,
    cold: bool,
) -> Result<SolverResult<MixedAssignment<usize, usize>, HardSoftScore>, Box<dyn Error>> {
    let problem = if cold {
        cold_problem(case)?
    } else {
        case.problem.clone()
    };
    let global_steps = SearchTermination::Condition(SearchLimit::Steps(case.steps));
    let phase = |joint_moves| {
        MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: LocalSearchConfig {
                max_steps: case.steps,
                max_evaluations: case.evaluations,
                selection: MoveSelection::BestAccepted,
                move_order: MoveOrder::Original,
                ..Default::default()
            },
            joint_moves,
        })
    };
    let mut phases = Vec::new();
    if cold {
        phases.push(MixedSolverPhase::Construction(
            MixedConstructionConfig::default(),
        ));
    }
    phases.push(phase(false));
    phases.push(phase(true));
    let plan = MixedSolverSequence {
        limits: SolverLimits {
            max_evaluations: case.evaluations,
            termination: Some(&global_steps),
            ..Default::default()
        },
        phases,
    };
    let result = if stream {
        solve_mixed_stream(
            &problem,
            || {
                let mut graph = StreamGraphBuilder::new();
                let source = MixedStreamSources::new(&mut graph);
                declare_owner_routes(
                    &mut graph,
                    source.basic.port(),
                    source.lists.entities.port(),
                    case.capacities.len(),
                    case.stream_graph_policy,
                    |((owner, depot), (_, route))| {
                        if cold {
                            cold_route_cardinality_score(case, *depot, route, *owner)
                        } else {
                            case.route_score(depot.unwrap(), route, *owner)
                        }
                    },
                )?;
                Ok((graph.build(), source))
            },
            plan,
        )?
    } else {
        let owners = case.capacities.len();
        let mut network = MixedConstraintNetwork::new(owners, owners, case.points.len());
        for owner in 0..owners {
            network.add_constraint(
                format!("route {owner}"),
                vec![MixedScope::Basic(owner), MixedScope::ListEntity(owner)],
                move |facts| {
                    let (MixedFact::Basic(depot), MixedFact::ListEntity(route)) =
                        (&facts[0], &facts[1])
                    else {
                        unreachable!()
                    };
                    if cold {
                        cold_route_cardinality_score(case, depot.copied(), route, owner)
                    } else {
                        case.route_score(*depot.unwrap(), route, owner)
                    }
                },
            )?;
        }
        solve_mixed(&problem, &network, plan)?
    };
    Ok(result)
}

// The registered customer set is fixed and assignments forbid duplicates.
// Missing customers = registered count - sum of route lengths. Charge the
// constant once to owner0, including when that owner's depot is unassigned.
fn cold_route_cardinality_score(
    case: &Case,
    depot: Option<usize>,
    route: &[usize],
    owner: usize,
) -> HardSoftScore {
    let base = partial_route_score(case, depot, route, owner);
    let registered = if owner == 0 { case.points.len() } else { 0 };
    HardSoftScore::new(
        base.hard() - registered as i64 + route.len() as i64,
        base.soft(),
    )
}
