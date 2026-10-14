//! Original application workflow; no adapted upstream test or source-method credit.
#![allow(dead_code)]
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
use timetuck::*;
type AppResult<T> = Result<T, String>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Employee {
    pub id: String,
    pub skills: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shift {
    pub id: String,
    pub start: i64,
    pub end: i64,
    pub skill: String,
    pub preferred: usize,
    pub employee: Option<usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Roster {
    pub employees: Vec<Employee>,
    pub shifts: Vec<Shift>,
    pub score: Option<HardSoftScore>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Configuration {
    pub steps: i64,
    pub preference_weight: i64,
}
impl Default for Configuration {
    fn default() -> Self {
        Self {
            steps: 40,
            preference_weight: 2,
        }
    }
}
pub fn configuration_binding() -> ModelConfigBinding<'static, Configuration> {
    let mut b = ModelConfigBinding::new("RosterConfiguration", Configuration::default).unwrap();
    b.member(
        "steps",
        Default::default(),
        |c| &c.steps,
        |c, v| c.steps = v,
    )
    .unwrap();
    b.member(
        "preference_weight",
        ModelConfigMemberOptions {
            kind: Some(ModelConfigParameterKind::Weight),
            constraint_name_ref: Some("preferences".into()),
            ..Default::default()
        },
        |c| &c.preference_weight,
        |c, v| c.preference_weight = v,
    )
    .unwrap();
    b
}
fn validate(roster: &Roster) -> AppResult<()> {
    if roster.employees.is_empty() || roster.shifts.is_empty() {
        return Err("employees and shifts must be nonempty".into());
    }
    let mut ids = BTreeSet::new();
    for e in &roster.employees {
        if e.id.is_empty()
            || !ids.insert(&e.id)
            || e.skills.is_empty()
            || e.skills.iter().any(String::is_empty)
        {
            return Err("employee IDs and skills must be valid and IDs unique".into());
        }
    }
    ids.clear();
    for s in &roster.shifts {
        if s.id.is_empty()
            || !ids.insert(&s.id)
            || s.start >= s.end
            || s.skill.is_empty()
            || s.preferred >= roster.employees.len()
            || s.employee.is_some_and(|e| e >= roster.employees.len())
        {
            return Err("invalid shift interval, identity or employee reference".into());
        }
    }
    Ok(())
}
pub struct RosterCodec;
fn array<'a>(v: &'a Value, name: &str) -> AppResult<&'a [Value]> {
    v[name]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing {name} array"))
}
fn text(v: &Value, name: &str) -> AppResult<String> {
    v[name]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("missing {name} text"))
}
fn number(v: &Value, name: &str) -> AppResult<i64> {
    v[name]
        .as_i64()
        .ok_or_else(|| format!("missing {name} integer"))
}
impl SolutionCodec<Roster> for RosterCodec {
    fn encode(&self, r: &Roster) -> AppResult<Vec<u8>> {
        validate(r)?;
        serde_json::to_vec_pretty(&json!({"version":1,"employees":r.employees.iter().map(|e| json!({"id":e.id,"skills":e.skills})).collect::<Vec<_>>(),"shifts":r.shifts.iter().map(|s|json!({"id":s.id,"start":s.start,"end":s.end,"skill":s.skill,"preferred":r.employees[s.preferred].id,"employee":s.employee.map(|i| &r.employees[i].id)})).collect::<Vec<_>>(),"score":r.score.map(|s|s.to_string())})).map_err(|e|e.to_string())
    }
    fn decode(&self, bytes: &[u8]) -> AppResult<Roster> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if v["version"] != 1 {
            return Err("unsupported roster document version".into());
        }
        let employees = array(&v, "employees")?
            .iter()
            .map(|e| {
                Ok(Employee {
                    id: text(e, "id")?,
                    skills: array(e, "skills")?
                        .iter()
                        .map(|s| {
                            s.as_str()
                                .map(str::to_owned)
                                .ok_or_else(|| "skill must be text".into())
                        })
                        .collect::<AppResult<_>>()?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let lookup = |id: &str| {
            employees
                .iter()
                .position(|e| e.id == id)
                .ok_or_else(|| format!("unknown employee {id:?}"))
        };
        let shifts = array(&v, "shifts")?
            .iter()
            .map(|s| {
                Ok(Shift {
                    id: text(s, "id")?,
                    start: number(s, "start")?,
                    end: number(s, "end")?,
                    skill: text(s, "skill")?,
                    preferred: lookup(&text(s, "preferred")?)?,
                    employee: if s["employee"].is_null() {
                        None
                    } else {
                        Some(lookup(&text(s, "employee")?)?)
                    },
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let r = Roster {
            employees,
            shifts,
            score: if v["score"].is_null() {
                None
            } else {
                Some(
                    text(&v, "score")?
                        .parse::<HardSoftScore>()
                        .map_err(|e| e.to_string())?,
                )
            },
        };
        validate(&r)?;
        Ok(r)
    }
}
pub fn sample() -> Roster {
    let employees = [
        ("Ada", vec!["doctor", "nurse"]),
        ("Ben", vec!["doctor"]),
        ("Cara", vec!["nurse"]),
    ]
    .into_iter()
    .map(|(id, skills)| Employee {
        id: id.into(),
        skills: skills.into_iter().map(str::to_owned).collect(),
    })
    .collect();
    let shifts = (0..6)
        .map(|i| Shift {
            id: format!("shift-{i}"),
            start: (i / 2) * 12,
            end: (i / 2) * 12 + 8,
            skill: if i % 2 == 0 { "doctor" } else { "nurse" }.into(),
            preferred: (i / 2) as usize,
            employee: None,
        })
        .collect();
    Roster {
        employees,
        shifts,
        score: None,
    }
}
pub fn binding() -> PlanningSolutionBinding<Roster, usize> {
    PlanningSolutionBinding::default().collection(
        PlanningEntityBinding::default()
            .member(
                "employee",
                "employees",
                |s: &Shift| s.employee,
                |s, v| s.employee = v,
            )
            .unwrap(),
        |r: &Roster| r.shifts.as_slice(),
        |r| r.shifts.as_mut_slice(),
        |r, name| (name == "employees").then(|| (0..r.employees.len()).collect()),
    )
}
fn overlap(a: &Shift, b: &Shift) -> bool {
    a.start < b.end && b.start < a.end
}
fn dense<'a>(
    r: &'a Roster,
    config: &Configuration,
) -> AppResult<ConstraintNetwork<'a, usize, HardSoftScore>> {
    let mut n: ConstraintNetwork<'a, usize, HardSoftScore> = ConstraintNetwork::new(r.shifts.len());
    let scope: Vec<_> = (0..r.shifts.len()).collect();
    n.add_weighted_constraint(
        "qualifications",
        scope.clone(),
        HardSoftScore::new(1, 0),
        |values| {
            -(values
                .iter()
                .enumerate()
                .filter(|(i, e)| {
                    e.is_some_and(|e| !r.employees[*e].skills.contains(&r.shifts[*i].skill))
                })
                .count() as i64)
        },
    )
    .map_err(|e| e.to_string())?;
    n.add_weighted_constraint(
        "overlap",
        scope.clone(),
        HardSoftScore::new(1, 0),
        |values| {
            let mut count = 0;
            for i in 0..values.len() {
                for j in i + 1..values.len() {
                    if values[i].is_some()
                        && values[i] == values[j]
                        && overlap(&r.shifts[i], &r.shifts[j])
                    {
                        count += 1;
                    }
                }
            }
            -count
        },
    )
    .map_err(|e| e.to_string())?;
    n.add_weighted_constraint("preferences", scope, HardSoftScore::new(0, 1), |values| {
        -(values
            .iter()
            .enumerate()
            .filter(|(i, e)| e.is_some_and(|e| *e != r.shifts[*i].preferred))
            .count() as i64)
    })
    .map_err(|e| e.to_string())?;
    let weights = configuration_binding()
        .constraint_weights(config, &["preference_weight"], |_, v| {
            Ok(HardSoftScore::new(0, v.as_i64().unwrap()))
        })
        .map_err(|e| e.to_string())?;
    n.apply_weight_overrides(&weights)
        .map_err(|e| e.to_string())?;
    Ok(n)
}
fn stream(
    r: &Roster,
    config: &Configuration,
) -> Result<
    (
        StreamGraph<'static, HardSoftScore>,
        PlanningStreamSource<usize>,
    ),
    GraphError,
> {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let assigned = b.filter(source.port(), |(_, e)| e.is_some())?;
    let facts = Arc::new(r.clone());
    let own = facts.clone();
    let bad = b.filter(&assigned, move |(s, e)| {
        !own.employees[e.unwrap()]
            .skills
            .contains(&own.shifts[*s].skill)
    })?;
    b.score_weighted("qualifications", &bad, HardSoftScore::new(1, 0), |_| -1)?;
    let pairs = b.join_equal(&assigned, &assigned, |(_, e)| *e, |(_, e)| *e)?;
    let own = facts.clone();
    let collisions = b.filter(&pairs, move |(a, c)| {
        a.0 < c.0 && overlap(&own.shifts[a.0], &own.shifts[c.0])
    })?;
    b.score_weighted("overlap", &collisions, HardSoftScore::new(1, 0), |_| -1)?;
    let disliked = b.filter(&assigned, move |(s, e)| {
        e.unwrap() != facts.shifts[*s].preferred
    })?;
    b.score_weighted("preferences", &disliked, HardSoftScore::new(0, 1), |_| -1)?;
    let mut graph = b.build();
    let weights = configuration_binding()
        .constraint_weights(config, &["preference_weight"], |_, v| {
            Ok(HardSoftScore::new(0, v.as_i64().unwrap()))
        })
        .map_err(|e| GraphError::Dependency(e.to_string()))?;
    graph.apply_weight_overrides(&weights)?;
    Ok((graph, source))
}
/// Separate employee-grouped interval sweep, not the graph join or dense pair loop.
pub fn independent_score(r: &Roster, config: &Configuration) -> HardSoftScore {
    let mut hard = 0;
    let mut soft = 0;
    let mut missing = 0;
    let mut employees: BTreeMap<usize, Vec<&Shift>> = BTreeMap::new();
    for s in &r.shifts {
        if let Some(e) = s.employee {
            hard -= i64::from(!r.employees[e].skills.iter().any(|skill| skill == &s.skill));
            soft -= config.preference_weight * i64::from(e != s.preferred);
            employees.entry(e).or_default().push(s);
        } else {
            missing += 1;
        }
    }
    for shifts in employees.values_mut() {
        shifts.sort_by_key(|s| s.start);
        let mut ends = Vec::new();
        for s in shifts {
            ends.retain(|&end| end > s.start);
            hard -= ends.len() as i64;
            ends.push(s.end);
        }
    }
    HardSoftScore::new(hard, soft).with_structural(-missing)
}
#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Dense,
    Stream,
}
#[derive(Clone, Debug)]
pub struct SolvedRoster {
    pub roster: Roster,
    pub work: SolverWork,
    pub phases: Vec<PhaseReport<HardSoftScore>>,
    pub termination: SolverTermination,
}
pub fn factory(
    config: Configuration,
    backend: Backend,
) -> AppResult<XmlSolverFactory<Arc<Roster>, SolvedRoster, String, HardSoftScore>> {
    let xml = format!(
        "<solver><termination><spentLimit>PT15S</spentLimit><bestScoreLimit>0hard/0soft</bestScoreLimit></termination><constructionHeuristic/><localSearch><termination><stepCountLimit>{}</stepCountLimit></termination></localSearch></solver>",
        config.steps
    );
    let plan = SolverXmlConfiguration::parse(&xml)
        .map_err(|e| e.to_string())?
        .native_termination_plan()
        .map_err(|e| e.to_string())?;
    factory_with_plan(config, backend, plan)
}
/// Use an explicit owned XML plan while retaining the application's native safeguards.
pub fn factory_with_plan(
    config: Configuration,
    backend: Backend,
    plan: XmlTerminationPlan,
) -> AppResult<XmlSolverFactory<Arc<Roster>, SolvedRoster, String, HardSoftScore>> {
    if !(1..=5000).contains(&config.steps) || config.preference_weight < 0 {
        return Err("steps must be1..5000 and preference_weight nonnegative".into());
    }
    let template = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 5000,
                max_evaluations: 100_000,
                selection: MoveSelection::BestAccepted,
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let binding = binding();
    XmlSolverFactory::new_with_controls(
        template,
        plan,
        move |input: Arc<Roster>, sequence, phase_bindings, stateful, progress| {
            validate(&input)?;
            let mut working = input.as_ref().clone();
            let problem = binding.open(&working).map_err(|e| e.to_string())?;
            let result = match backend {
                Backend::Dense => solve_with_extensions(
                    &problem,
                    &dense(&working, &config)?,
                    sequence,
                    SolverExtensions {
                        phase_terminations: phase_bindings,
                        stateful_termination: stateful,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?,
                Backend::Stream => solve_stream_with_extensions(
                    &problem,
                    || stream(&working, &config),
                    sequence,
                    SolverExtensions {
                        phase_terminations: phase_bindings,
                        stateful_termination: stateful,
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?,
            };
            binding
                .commit(&mut working, &result.assignment)
                .map_err(|e| e.to_string())?;
            working.score = result.score;
            if working.score != Some(independent_score(&working, &config)) {
                return Err("independent application score mismatch".into());
            }
            progress.record_work(result.work);
            Ok(SolvedRoster {
                roster: working,
                work: result.work,
                phases: result.phases,
                termination: result.termination,
            })
        },
    )
    .map_err(|e| e.to_string())
}
/// Read real input/config files, solve through an owned manager, publish and reread output.
pub fn run_files(
    input: &Path,
    configuration: &Path,
    output: &Path,
    backend: Backend,
) -> AppResult<SolvedRoster> {
    run_files_with_plan(input, configuration, output, backend, None)
}
/// Read a complete custom solver XML document before publishing any output.
pub fn run_files_with_xml(
    input: &Path,
    configuration: &Path,
    solver_xml: &Path,
    output: &Path,
    backend: Backend,
) -> AppResult<SolvedRoster> {
    let xml = std::fs::read_to_string(solver_xml).map_err(|e| e.to_string())?;
    let plan = SolverXmlConfiguration::parse(&xml)
        .map_err(|e| e.to_string())?
        .native_termination_plan()
        .map_err(|e| e.to_string())?;
    run_files_with_plan(input, configuration, output, backend, Some(plan))
}
fn run_files_with_plan(
    input: &Path,
    configuration: &Path,
    output: &Path,
    backend: Backend,
    plan: Option<XmlTerminationPlan>,
) -> AppResult<SolvedRoster> {
    let config = configuration_binding()
        .decode(&std::fs::read_to_string(configuration).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let io = SolutionFileIo::new(RosterCodec);
    let retained = Arc::new(io.read(input).map_err(|e| e.to_string())?);
    let before = retained.as_ref().clone();
    let factory = match plan {
        Some(plan) => factory_with_plan(config, backend, plan)?,
        None => factory(config, backend)?,
    };
    let mut manager = factory.build_manager(1).map_err(|e| e.to_string())?;
    let job = manager
        .job_builder(1)
        .with_problem(retained.clone())
        .run()
        .map_err(|e| e.to_string())?;
    drop(factory);
    let result = job.wait();
    let result = result.as_ref().as_ref().map_err(|e| e.to_string())?;
    if retained.as_ref() != &before {
        return Err("retained input changed".into());
    }
    io.write(&result.roster, output)
        .map_err(|e| e.to_string())?;
    let readback: Roster = io.read(output).map_err(|e| e.to_string())?;
    if readback != result.roster {
        return Err("output document did not round trip".into());
    }
    manager.shutdown().map_err(|e| e.to_string())?;
    Ok(result.clone())
}
