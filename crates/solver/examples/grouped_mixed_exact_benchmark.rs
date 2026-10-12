//! Paired native grouping probe; this is not a Java performance comparison.
use std::{hint::black_box, time::Instant};
use timetuck::*;
fn problem(basics: usize, domain: i32) -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(vec![Variable::new((0..domain).collect()); basics]).unwrap(),
        ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap(),
    )
}
fn network(basics: usize) -> MixedConstraintNetwork<'static, i32, i32, SimpleScore> {
    let mut n = MixedConstraintNetwork::new(basics, 2, 3);
    let mut scopes: Vec<_> = (0..basics).map(MixedScope::Basic).collect();
    scopes.extend([MixedScope::ListEntity(0), MixedScope::ListEntity(1)]);
    n.add_constraint("coupled", scopes, |facts| {
        let basic: i64 = facts
            .iter()
            .filter_map(|f| {
                if let MixedFact::Basic(v) = f {
                    Some(i64::from(v.copied().unwrap_or(-1)))
                } else {
                    None
                }
            })
            .sum();
        let list: i64 = facts
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                if let MixedFact::ListEntity(row) = f {
                    Some(
                        row.iter()
                            .enumerate()
                            .map(|(j, v)| i64::from(*v) * (j as i64 + 1) + i as i64)
                            .sum::<i64>(),
                    )
                } else {
                    None
                }
            })
            .sum();
        SimpleScore::new(basic * list - list * list + basic)
    })
    .unwrap();
    n
}
fn run(
    grouped: bool,
    p: &MixedProblem<i32, i32>,
    n: &MixedConstraintNetwork<'_, i32, i32, SimpleScore>,
) -> MixedBranchAndBoundResult<i32, i32, SimpleScore> {
    if grouped {
        GroupedMixedExactSolver::default()
            .solve(p, n, None)
            .unwrap()
    } else {
        branch_and_bound_mixed(
            p,
            |a, _, _| BranchEvaluation::unbounded(n.calculate(p, a).unwrap()),
            BranchAndBoundConfig::default(),
        )
        .unwrap()
    }
}
fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
fn main() {
    let mut workloads = vec![];
    for (basics, domain) in [(2, 3), (4, 3), (6, 2)] {
        let p = problem(basics, domain);
        let n = network(basics);
        let old = run(false, &p, &n);
        let grouped = run(true, &p, &n);
        assert!(old.is_optimal() && grouped.is_optimal());
        assert_eq!(old.complete_evaluations, grouped.complete_evaluations);
        assert_eq!(
            old.best.as_ref().unwrap().score,
            grouped.best.as_ref().unwrap().score
        );
        assert_eq!((old.pruned_branches, grouped.pruned_branches), (0, 0));
        for _ in 0..3 {
            black_box(run(false, &p, &n));
            black_box(run(true, &p, &n));
        }
        let mut before = vec![];
        let mut after = vec![];
        for sample in 0..15 {
            for which in [sample % 2 == 1, sample % 2 == 0] {
                let started = Instant::now();
                for _ in 0..20 {
                    let r = run(which, black_box(&p), black_box(&n));
                    assert_eq!(r.complete_evaluations, old.complete_evaluations);
                    assert_eq!(
                        r.best.as_ref().unwrap().score,
                        old.best.as_ref().unwrap().score
                    );
                    black_box(r);
                }
                let ms = started.elapsed().as_secs_f64() * 1000.0 / 20.0;
                if which {
                    after.push(ms)
                } else {
                    before.push(ms)
                }
            }
        }
        workloads.push(serde_json::json!({"basic_variables":basics,"domain_size":domain,"complete_candidates":old.complete_evaluations,"score":old.best.as_ref().unwrap().score.to_string(),"before_evaluations":old.evaluations,"after_evaluations":grouped.evaluations,"before_expansions":old.expanded_nodes,"after_expansions":grouped.expanded_nodes,"before_samples_ms":before,"after_samples_ms":after,"before_median_ms":median(&before),"after_median_ms":median(&after),"before_over_after":median(&before)/median(&after)}));
    }
    println!(
        "{}",
        serde_json::json!({"comparison":"native per-variable versus grouped mixed Cartesian exact search","java_comparison":false,"warmups":3,"samples":15,"invocations_per_sample":20,"workloads":workloads})
    );
}
