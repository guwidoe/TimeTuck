//! Application-origin RealWorld model; not an adapted Timefold test.
//! Derived from the Claude native-application scratchpad; see tests/application/real_world/NOTICE.md.
//! Deliberately retains the application's self-joins and grouped encounter counts.
use serde_json::{Value, json};
use std::{collections::HashMap, time::Duration};
use timetuck::*;

const SCALE: f64 = 100.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum PairKind {
    KeepTogether,
    KeepApart,
    PreferTogether,
    PreferApart,
}

struct PairCons {
    kind: PairKind,
    weight: i64,
    sessions: Vec<bool>,
}

struct Balance {
    groups: Vec<bool>,
    sessions: Vec<bool>,
    key: String,
    targets: Vec<(String, i64)>,
    at_least: bool,
    weight: i64,
}

pub struct Model {
    people: Vec<String>,
    groups: Vec<String>,
    sessions: usize,
    cap: Vec<Vec<i64>>,
    var_ps: Vec<(usize, usize)>,
    fixed_var: Vec<bool>,
    session_vars: Vec<Vec<usize>>,
    uc_weight: i64,
    repeats: Option<(u32, bool, i64)>,
    pair_cons: Vec<PairCons>,
    member: Vec<Vec<usize>>,
    balances: Vec<Balance>,
    attrs: Vec<HashMap<String, String>>,
}

fn w(v: &Value) -> i64 {
    (v.as_f64().unwrap_or(1000.0) * SCALE).round() as i64
}

fn sessions_mask(c: &Value, n: usize) -> Vec<bool> {
    match c.get("session_indices").and_then(Value::as_array) {
        Some(list) => {
            let mut m = vec![false; n];
            for s in list {
                m[s.as_u64().unwrap() as usize] = true;
            }
            m
        }
        None => vec![true; n],
    }
}

pub fn load(problem: &Value) -> (Model, FiniteProblem<usize>, Vec<Option<usize>>) {
    let sc = &problem["scenario"];
    let n_s = sc["session_count"].as_u64().unwrap() as usize;
    let people: Vec<String> = sc["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_string())
        .collect();
    let attrs: Vec<HashMap<String, String>> = sc["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            p["attributes"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    let attends: Vec<Vec<bool>> = sc["people"]
        .as_array()
        .unwrap()
        .iter()
        .map(
            |p| match p.get("session_indices").and_then(Value::as_array) {
                Some(list) => {
                    let mut m = vec![false; n_s];
                    for s in list {
                        m[s.as_u64().unwrap() as usize] = true;
                    }
                    m
                }
                None => vec![true; n_s],
            },
        )
        .collect();
    let groups: Vec<String> = sc["groups"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["id"].as_str().unwrap().to_string())
        .collect();
    let cap: Vec<Vec<i64>> = (0..n_s)
        .map(|s| {
            sc["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(
                    |g| match g.get("session_capacities").and_then(Value::as_array) {
                        Some(c) => c[s].as_i64().unwrap(),
                        None => g["capacity"].as_i64().unwrap(),
                    },
                )
                .collect()
        })
        .collect();
    let pidx: HashMap<&str, usize> = people
        .iter()
        .enumerate()
        .map(|(i, p)| (p.as_str(), i))
        .collect();
    let gidx: HashMap<&str, usize> = groups
        .iter()
        .enumerate()
        .map(|(i, g)| (g.as_str(), i))
        .collect();

    let mut uc_weight = 0;
    let mut repeats = None;
    for o in problem["objectives"].as_array().unwrap() {
        match o["type"].as_str().unwrap() {
            "maximize_unique_contacts" => uc_weight = w(&o["weight"]),
            "avoid_repeats" => {
                repeats = Some((
                    o["max_encounters_per_pair"].as_u64().unwrap() as u32,
                    o["penalty_curve"] == "squared",
                    w(&o["weight"]),
                ))
            }
            t => panic!("unsupported objective {t}"),
        }
    }

    let mut fixed: HashMap<(usize, usize), usize> = HashMap::new();
    let mut pair_cons = Vec::new();
    let mut member = vec![Vec::new(); people.len()];
    let mut balances = Vec::new();
    for c in problem["constraints"].as_array().unwrap() {
        let ids = || {
            c["person_ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| pidx[p.as_str().unwrap()])
                .collect::<Vec<_>>()
        };
        let kind = match c["type"].as_str().unwrap() {
            "fixed_placement" => {
                let g = gidx[c["group_id"].as_str().unwrap()];
                let m = sessions_mask(c, n_s);
                for p in ids() {
                    for s in (0..n_s).filter(|&s| m[s]) {
                        fixed.insert((p, s), g);
                    }
                }
                continue;
            }
            "attribute_balance" => {
                let groups_mask = match c.get("group_ids").and_then(Value::as_array) {
                    Some(list) => {
                        let mut m = vec![false; groups.len()];
                        for g in list {
                            m[gidx[g.as_str().unwrap()]] = true;
                        }
                        m
                    }
                    None => vec![true; groups.len()],
                };
                balances.push(Balance {
                    groups: groups_mask,
                    sessions: sessions_mask(c, n_s),
                    key: c["attribute_key"].as_str().unwrap().to_string(),
                    targets: c["target_counts"]
                        .as_object()
                        .unwrap()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_i64().unwrap()))
                        .collect(),
                    at_least: c["target_mode"] == "at_least",
                    weight: w(&c["penalty_weight"]),
                });
                continue;
            }
            "keep_together" => PairKind::KeepTogether,
            "keep_apart" => PairKind::KeepApart,
            "prefer_together" => PairKind::PreferTogether,
            "prefer_apart" => PairKind::PreferApart,
            t => panic!("unsupported constraint {t}"),
        };
        let index = pair_cons.len();
        pair_cons.push(PairCons {
            kind,
            weight: c.get("penalty_weight").map(w).unwrap_or(0),
            sessions: sessions_mask(c, n_s),
        });
        for p in ids() {
            member[p].push(index);
        }
    }

    // One planning variable per attended (person, session), valued by group index.
    let mut var_ps = Vec::new();
    let mut vars = Vec::new();
    let mut fixed_var = Vec::new();
    let mut session_vars = vec![Vec::new(); n_s];
    for s in 0..n_s {
        for (p, attendance) in attends.iter().enumerate() {
            if !attendance[s] {
                continue;
            }
            let domain = match fixed.get(&(p, s)) {
                Some(&g) => vec![g],
                None => (0..groups.len()).filter(|&g| cap[s][g] > 0).collect(),
            };
            let is_fixed = fixed.contains_key(&(p, s));
            if !is_fixed {
                session_vars[s].push(var_ps.len());
            }
            fixed_var.push(is_fixed);
            var_ps.push((p, s));
            vars.push(Variable::new(domain));
        }
    }
    // Capacity-respecting start: fixed people first, then first fit.
    let mut initial = vec![None; var_ps.len()];
    let mut occ = vec![vec![0i64; groups.len()]; n_s];
    for (v, &(p, s)) in var_ps.iter().enumerate() {
        if let Some(&g) = fixed.get(&(p, s)) {
            initial[v] = Some(g);
            occ[s][g] += 1;
        }
    }
    for (v, &(_, s)) in var_ps.iter().enumerate() {
        if initial[v].is_none() {
            let g = (0..groups.len())
                .max_by_key(|&g| cap[s][g] - occ[s][g])
                .unwrap();
            initial[v] = Some(g);
            occ[s][g] += 1;
        }
    }
    let model = Model {
        people,
        groups,
        sessions: n_s,
        cap,
        var_ps,
        fixed_var,
        session_vars,
        uc_weight,
        repeats,
        pair_cons,
        member,
        balances,
        attrs,
    };
    (model, FiniteProblem::new(vars).unwrap(), initial)
}

pub fn graph(m: &Model) -> (StreamGraph<'_, HardSoftScore>, PlanningStreamSource<usize>) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let present = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
    // (session, person, group)
    let assigned = b
        .map(&present, move |&(id, v)| {
            let (p, s) = m.var_ps[id];
            (s, p, v.unwrap())
        })
        .unwrap();

    let loads = b
        .group_collect(&assigned, |&(s, _, g)| ((s, g), 1))
        .unwrap();
    b.score("group capacity", &loads, move |((s, g), sum)| {
        HardSoftScore::new(-(sum.sum - m.cap[*s][*g]).max(0), 0)
    })
    .unwrap();

    let meets = b
        .join_equal(&assigned, &assigned, |r| (r.0, r.2), |r| (r.0, r.2))
        .unwrap();
    let meets = b.filter(&meets, |(a, b)| a.1 < b.1).unwrap();
    let pair_counts = b.group_collect(&meets, |(a, b)| ((a.1, b.1), 1)).unwrap();
    b.score("contacts", &pair_counts, move |(_, sum)| {
        let mut soft = m.uc_weight;
        if let Some((max, squared, weight)) = m.repeats {
            let excess = (sum.sum - i64::from(max)).max(0);
            soft -= weight * if squared { excess * excess } else { excess };
        }
        HardSoftScore::new(0, soft)
    })
    .unwrap();

    if !m.pair_cons.is_empty() {
        let members = b
            .flat_map(&assigned, move |&(s, p, g)| {
                m.member[p]
                    .iter()
                    .filter(|&&c| m.pair_cons[c].sessions[s])
                    .map(|&c| (c, s, p, g))
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let pairs = b
            .join_equal(&members, &members, |r| (r.0, r.1), |r| (r.0, r.1))
            .unwrap();
        let pairs = b.filter(&pairs, |(a, b)| a.2 < b.2).unwrap();
        b.score("pair constraints", &pairs, move |(a, b)| {
            let c = &m.pair_cons[a.0];
            let same = a.3 == b.3;
            match c.kind {
                PairKind::KeepTogether if !same => HardSoftScore::new(-1, 0),
                PairKind::KeepApart if same => HardSoftScore::new(-1, 0),
                PairKind::PreferTogether if !same => HardSoftScore::new(0, -c.weight),
                PairKind::PreferApart if same => HardSoftScore::new(0, -c.weight),
                _ => HardSoftScore::ZERO,
            }
        })
        .unwrap();
    }

    if !m.balances.is_empty() {
        let tallies = b
            .flat_map(&assigned, move |&(s, p, g)| {
                let mut rows = Vec::new();
                for (bi, bal) in m.balances.iter().enumerate() {
                    if !bal.sessions[s] || !bal.groups[g] {
                        continue;
                    }
                    let value = m.attrs[p].get(&bal.key);
                    for (ti, (target, _)) in bal.targets.iter().enumerate() {
                        rows.push((bi, s, g, ti, i64::from(value == Some(target))));
                    }
                }
                rows
            })
            .unwrap();
        let counts = b
            .group_collect(&tallies, |&(bi, s, g, ti, hit)| ((bi, s, g, ti), hit))
            .unwrap();
        b.score(
            "attribute balance",
            &counts,
            move |((bi, _, _, ti), sum)| {
                let bal = &m.balances[*bi];
                let target = bal.targets[*ti].1;
                let gap = if bal.at_least {
                    (target - sum.sum).max(0)
                } else {
                    (target - sum.sum).abs()
                };
                HardSoftScore::new(0, -bal.weight * gap)
            },
        )
        .unwrap();
    }
    (b.build(), source)
}

/// Random within-session changes into groups with spare seats, and swaps.
pub struct GroupMoves<'m> {
    m: &'m Model,
    rng: u64,
}
impl GroupMoves<'_> {
    fn next(&mut self, n: usize) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % n as u64) as usize
    }
}
impl<'m> MoveProvider<FiniteProblem<usize>, Assignment<usize>, Move<usize>> for GroupMoves<'m> {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<usize>,
        a: &'a Assignment<usize>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<usize>> + 'a>
    where
        Move<usize>: 'a,
    {
        let m = self.m;
        let mut occ = vec![vec![0i64; m.groups.len()]; m.sessions];
        for (v, val) in a.values().iter().enumerate() {
            occ[m.var_ps[v].1][val.unwrap()] += 1;
        }
        let movable: Vec<usize> = (0..m.var_ps.len()).filter(|&v| !m.fixed_var[v]).collect();
        Box::new(std::iter::from_fn(move || {
            loop {
                let v = movable[self.next(movable.len())];
                let s = m.var_ps[v].1;
                let g = a.values()[v].unwrap();
                if self.next(2) == 0 {
                    let pool = &m.session_vars[s];
                    let u = pool[self.next(pool.len())];
                    if a.values()[u] != Some(g) {
                        return Some(Move::Swap { left: v, right: u });
                    }
                } else {
                    let h = self.next(m.groups.len());
                    if h != g && occ[s][h] < m.cap[s][h] {
                        return Some(Move::Change {
                            variable: v,
                            value: Some(h),
                        });
                    }
                }
            }
        }))
    }
}

impl Model {
    pub fn dimensions(&self) -> (usize, usize, usize, usize) {
        (
            self.people.len(),
            self.groups.len(),
            self.sessions,
            self.var_ps.len(),
        )
    }

    /// The unchanged spike interchange format, independently rescorable from JSON.
    pub fn schedule(&self, assignment: &Assignment<usize>) -> Value {
        let mut schedule = serde_json::Map::new();
        for s in 0..self.sessions {
            let mut by_group: serde_json::Map<String, Value> =
                self.groups.iter().map(|g| (g.clone(), json!([]))).collect();
            for (v, &(p, ps)) in self.var_ps.iter().enumerate() {
                if ps == s {
                    let g = &self.groups[assignment.values()[v].unwrap()];
                    by_group[g]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(self.people[p]));
                }
            }
            schedule.insert(format!("session_{s}"), Value::Object(by_group));
        }
        Value::Object(schedule)
    }
}

impl<'m> GroupMoves<'m> {
    pub fn new(model: &'m Model, seed: u64) -> Self {
        Self {
            m: model,
            rng: seed | 1,
        }
    }
}

pub struct BenchmarkRun {
    pub initial_score: HardSoftScore,
    pub result: LocalSearchResult<usize, HardSoftScore>,
    pub elapsed_ns: u128,
}

/// Uses the spike's SA=5/COOL=40000 configuration, with an optional deterministic
/// evaluation ceiling for regression tests. Only solving is timed, including
/// stream score-director initialization, as in the original application spike.
pub fn solve(
    m: &Model,
    problem: &FiniteProblem<usize>,
    initial: &[Option<usize>],
    seed: u64,
    max_time: Option<Duration>,
    max_evaluations: u64,
) -> BenchmarkRun {
    let (initial_graph, initial_source) = graph(m);
    let initial_director = StreamScoreDirector::new(
        problem,
        initial_graph,
        initial_source,
        Assignment::new(initial.to_vec()),
    )
    .unwrap();
    let initial_score = initial_director.score();
    drop(initial_director);
    let (g, source) = graph(m);
    let config = LocalSearchConfig {
        max_evaluations,
        max_accepted_moves: u64::MAX,
        max_steps: u64::MAX,
        max_time,
        acceptance: Acceptance::SimulatedAnnealing {
            starting_temperature: HardSoftScore::new(0, 500),
            cooling_steps: 40_000,
            seed,
        },
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    };
    let mut provider = GroupMoves::new(m, seed);
    let started = std::time::Instant::now();
    let result = stream_local_search_from_with_provider(
        problem,
        g,
        source,
        Assignment::new(initial.to_vec()),
        config,
        &mut provider,
    )
    .unwrap();
    BenchmarkRun {
        initial_score,
        result,
        elapsed_ns: started.elapsed().as_nanos(),
    }
}
