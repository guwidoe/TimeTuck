// Adapted from pinned Community benchmark ranking tests; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md for attribution.
use timetuck::*;
fn outcome(n: i64, initialized: bool) -> BenchmarkOutcome {
    BenchmarkOutcome::success(&SimpleScore::new(n), initialized).unwrap()
}
fn rows(config: usize, scores: &[i64]) -> Vec<BenchmarkComparison<usize>> {
    scores
        .iter()
        .map(|&score| BenchmarkComparison {
            configuration: config,
            problem: 0,
            outcome: outcome(score, true),
        })
        .collect()
}
fn weights(count: usize, entries: &[BenchmarkComparison<usize>]) -> Vec<BenchmarkStanding> {
    let mut w = rank_benchmarks(count, entries).unwrap();
    w.sort_by_key(|w| w.configuration);
    w
}
fn ordered(w: &[BenchmarkStanding], indices: &[usize]) {
    for (i, &a) in indices.iter().enumerate() {
        for (j, &b) in indices.iter().enumerate() {
            assert_eq!(w[a].compare(&w[b]), i.cmp(&j));
        }
    }
}
#[test]
fn total_rank_normal() {
    let entries = [rows(0, &[-1000, -300, -40]), rows(1, &[-2000, -200, -30])].concat();
    ordered(&weights(2, &entries), &[0, 1]);
}
#[test]
fn total_rank_equal_count() {
    let entries = [
        rows(0, &[-5000, -900, -90]),
        rows(1, &[-1000, -200, -20]),
        rows(2, &[-5000, -100, -10]),
    ]
    .concat();
    ordered(&weights(3, &entries), &[0, 1, 2]);
}
#[test]
fn total_rank_uninitialized() {
    let mut entries = [rows(0, &[-1000, -400, -30]), rows(1, &[-1000, -400, -30])].concat();
    let w = weights(2, &entries);
    assert!(w[0].compare(&w[1]).is_eq());
    entries[0].outcome = outcome(-1000, false);
    entries[3].outcome = outcome(-1000, false);
    let w = weights(2, &entries);
    assert!(w[0].compare(&w[1]).is_eq());
    entries[3].outcome = outcome(-1000, true);
    entries[4].outcome = outcome(-400, false);
    ordered(&weights(2, &entries), &[0, 1]);
}
#[test]
fn total_rank_different_count() {
    let entries = [rows(0, &[-1000, -400]), rows(1, &[-1000, -400, -30])].concat();
    ordered(&weights(2, &entries), &[0, 1]);
}
#[test]
fn total_rank_different_score_type() {
    let mut entries = rows(0, &[-1000, -400, -30]);
    entries.extend([-1000, -400, -30].map(|score| BenchmarkComparison {
        configuration: 1,
        problem: 0,
        outcome: BenchmarkOutcome::success(&HardSoftScore::new(score, 0), true).unwrap(),
    }));
    let w = weights(2, &entries);
    assert!(w[0].compare(&w[1]).is_eq());
}
#[test]
fn total_rank_disjoint() {
    let mut entries = [rows(0, &[-1000, -400, -30]), rows(1, &[-2000, -200, -30])].concat();
    for (i, e) in entries.iter_mut().enumerate() {
        e.problem = i;
    }
    ordered(&weights(2, &entries), &[1, 0]);
}
#[test]
fn total_rank_disjoint_equal() {
    let mut entries = [rows(0, &[-1000, -400, -30]), rows(1, &[-1000, -400, -30])].concat();
    for (i, e) in entries.iter_mut().enumerate() {
        e.problem = i;
    }
    let w = weights(2, &entries);
    assert!(w[0].compare(&w[1]).is_eq());
}
#[test]
fn total_rank_overlapping() {
    let mut entries = [rows(0, &[-1000, -400, -30]), rows(1, &[-1000, -400, -30])].concat();
    for (i, e) in entries.iter_mut().enumerate() {
        e.problem = i % 3;
    }
    let w = weights(2, &entries);
    assert!(w[0].compare(&w[1]).is_eq());
    for (i, e) in entries.iter_mut().enumerate() {
        e.problem = i;
    }
    entries[4].problem = 0;
    ordered(&weights(2, &entries), &[0, 1]);
}
#[test]
fn individual_sub_single_order() {
    let values = [
        BenchmarkOutcome::Failure,
        outcome(-1, false),
        outcome(-300, true),
        outcome(-20, true),
    ];
    for (i, a) in values.iter().enumerate() {
        for (j, b) in values.iter().enumerate() {
            assert_eq!(a.compare(b), i.cmp(&j));
        }
    }
}
#[test]
fn individual_single_order() {
    let values = [
        BenchmarkOutcome::Failure,
        outcome(-1, false),
        outcome(-300, true),
        outcome(-20, true),
    ]
    .map(|o| BenchmarkSummary::new([o]));
    for (i, a) in values.iter().enumerate() {
        for (j, b) in values.iter().enumerate() {
            assert_eq!(a.compare(b, BenchmarkRanking::TotalScore), i.cmp(&j));
        }
    }
}
