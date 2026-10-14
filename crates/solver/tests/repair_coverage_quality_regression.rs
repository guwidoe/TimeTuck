//! Original generated quality fixtures; no upstream method credit.
use serde_json::Value;
use timetuck::*;

fn numbers(v: &Value) -> Vec<i64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap())
        .collect()
}
struct Case {
    id: String,
    problem: ListProblem<usize>,
    caps: Vec<usize>,
    costs: Vec<Vec<i64>>,
    missing: Vec<i64>,
}
impl Case {
    fn from_json(v: &Value) -> Self {
        let costs: Vec<_> = v["costs"].as_array().unwrap().iter().map(numbers).collect();
        let caps: Vec<_> = numbers(&v["caps"])
            .into_iter()
            .map(|n| n as usize)
            .collect();
        let mut entities = vec![ListEntity::new(vec![]); caps.len()];
        entities[0] = ListEntity::new((0..costs.len()).collect());
        let mut problem = ListProblem::new((0..costs.len()).collect(), entities).unwrap();
        if v["optional"].as_bool().unwrap() {
            problem = problem.allowing_unassigned();
        }
        Self {
            id: v["id"].as_str().unwrap().into(),
            problem,
            caps,
            costs,
            missing: numbers(&v["missing"]),
        }
    }
    fn score(&self, lists: &[Vec<usize>]) -> HardSoftScore {
        let mut seen = vec![false; self.costs.len()];
        let mut cost = 0;
        for (owner, row) in lists.iter().enumerate() {
            let excess = row.len().saturating_sub(self.caps[owner]) as i64;
            cost += excess * excess;
            for &value in row {
                seen[value] = true;
                cost += self.costs[value][owner];
            }
            cost += row.windows(2).filter(|w| w[0] < w[1]).count() as i64 * 2;
        }
        cost += seen
            .iter()
            .zip(&self.missing)
            .filter(|(seen, _)| !**seen)
            .map(|(_, cost)| cost)
            .sum::<i64>();
        HardSoftScore::new(0, -cost)
    }
    fn run(
        &self,
        config: RuinRecreateConfig<'_, HardSoftScore>,
    ) -> ListRuinRecreateResult<usize, HardSoftScore> {
        let count = self.caps.len();
        let mut network = ListConstraintNetwork::new(count, self.costs.len());
        network
            .add_constraint(
                "full",
                (0..count).map(ListScope::Entity).collect(),
                |facts| {
                    let rows: Vec<_> = facts
                        .iter()
                        .map(|f| match f {
                            ListFact::Entity(row) => row.to_vec(),
                            _ => unreachable!(),
                        })
                        .collect();
                    self.score(&rows)
                },
            )
            .unwrap();
        let dense = ruin_recreate_list_search_from(
            &self.problem,
            &network,
            self.problem.initial_assignment(),
            config.clone(),
        )
        .unwrap();
        let stream = ruin_recreate_list_stream_search_from(
            &self.problem,
            || {
                let mut b = StreamGraphBuilder::new();
                let sources = ListStreamSources::new(&mut b);
                let lists = b.collect_list(sources.entities.port(), |row| row.clone())?;
                b.score("full", &lists, |rows| {
                    let mut all = vec![vec![]; count];
                    for (i, row) in rows.iter() {
                        all[*i] = row.clone();
                    }
                    self.score(&all)
                })?;
                Ok((b.build(), sources))
            },
            self.problem.initial_assignment(),
            config,
        )
        .unwrap();
        assert_eq!(dense.best, stream.best, "{}", self.id);
        assert_eq!(
            (
                dense.attempts,
                dense.complete_reconstructions,
                dense.evaluations,
                dense.repair_move_evaluations,
                dense.accepted_moves,
                dense.termination
            ),
            (
                stream.attempts,
                stream.complete_reconstructions,
                stream.evaluations,
                stream.repair_move_evaluations,
                stream.accepted_moves,
                stream.termination
            ),
            "{}",
            self.id
        );
        let best = dense.best.as_ref().unwrap();
        self.problem.validate(&best.assignment).unwrap();
        assert!(self.problem.is_complete(&best.assignment));
        assert_eq!(best.score, self.score(best.assignment.lists()));
        assert!(best.score >= self.score(self.problem.initial_assignment().lists()));
        for (entity, row) in best.assignment.lists().iter().enumerate() {
            for (index, value) in row.iter().enumerate() {
                assert_eq!(
                    best.assignment.element(value).unwrap().position,
                    ListPosition { entity, index }
                );
            }
        }
        dense
    }
}
#[test]
fn generated_quality_cases_preserve_legacy_and_qualify_coverage_on_both_backends() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/repair_coverage_quality.json")).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 64);
    let (mut legacy_optimal, mut cycle_optimal, mut fixed) = (0, 0, 0);
    for row in cases {
        let case = Case::from_json(&row["case"]);
        let optimum = numbers(&row["optimum"]);
        let optimum = HardSoftScore::new(optimum[0], optimum[1]);
        let legacy = numbers(&row["legacy_score"]);
        let legacy = HardSoftScore::new(legacy[0], legacy[1]);
        let config = RuinRecreateConfig {
            seed: 20261225,
            minimum_ruined: 1,
            maximum_ruined: if row["mode"] == "all" {
                case.costs.len()
            } else {
                case.costs.len().div_ceil(2)
            },
            max_attempts: 8,
            max_evaluations: u64::MAX,
            acceptance: Acceptance::StepCountingHillClimbing {
                period: 9,
                mode: StepCountingMode::Step,
            },
            ..Default::default()
        };
        assert_eq!(config.ruin_sampling, RuinSampling::Independent);
        let original = case.run(config.clone());
        assert_eq!(
            original.best.unwrap().score,
            legacy,
            "{} {}",
            case.id,
            row["mode"]
        );
        legacy_optimal += usize::from(legacy == optimum);
        let cycle = case.run(RuinRecreateConfig {
            ruin_sampling: RuinSampling::CoverageCycle,
            ..config
        });
        assert_eq!(cycle.attempts, 8);
        assert_eq!(cycle.complete_reconstructions, 8);
        let score = cycle.best.unwrap().score;
        assert!(score <= optimum, "independent optimum bound {}", case.id);
        cycle_optimal += usize::from(score == optimum);
        fixed += usize::from(legacy < optimum && score == optimum);
    }
    assert_eq!(legacy_optimal, 56);
    assert_eq!(cycle_optimal, 64);
    assert_eq!(fixed, 8);
    println!(
        "quality: legacy {legacy_optimal}/64; coverage {cycle_optimal}/64; old misses fixed {fixed}/8"
    );
}
