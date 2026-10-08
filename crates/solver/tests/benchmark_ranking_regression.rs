// Original independent relative-ranking checks; no upstream credit.
use timetuck::*;
fn entry(
    configuration: usize,
    problem: usize,
    value: Option<(i64, bool)>,
) -> BenchmarkComparison<usize> {
    BenchmarkComparison {
        configuration,
        problem,
        outcome: value.map_or(BenchmarkOutcome::Failure, |(n, c)| {
            BenchmarkOutcome::success(&SimpleScore::new(n), c).unwrap()
        }),
    }
}
#[test]
fn prefix_counting_matches_naive_pair_oracle_with_missing_repeated_and_failed_rows() {
    for seed in 0..32usize {
        let entries: Vec<_> = (0..43)
            .map(|i| {
                entry(
                    (i * 7 + seed) % 5,
                    (i * 3 + seed) % 7,
                    (!(i + seed).is_multiple_of(6)).then_some((
                        ((i * 13 + seed) % 11) as i64 - 7,
                        !(i + seed).is_multiple_of(4),
                    )),
                )
            })
            .collect();
        let mut expected = [[0u128; 3]; 5];
        for a in &entries {
            for b in &entries {
                if a.configuration == b.configuration || a.problem != b.problem {
                    continue;
                }
                let i = match (&a.outcome, &b.outcome) {
                    (BenchmarkOutcome::Failure, BenchmarkOutcome::Failure) => 1,
                    (BenchmarkOutcome::Failure, _) => 2,
                    (_, BenchmarkOutcome::Failure) => 0,
                    (
                        BenchmarkOutcome::Success {
                            score: a,
                            initialized: ai,
                        },
                        BenchmarkOutcome::Success {
                            score: b,
                            initialized: bi,
                        },
                    ) => {
                        // Inputs are one-level sound integers; compare independently of outcome.compare.
                        match ai.cmp(bi).then_with(|| {
                            a.levels()[0].coefficient().cmp(b.levels()[0].coefficient())
                        }) {
                            std::cmp::Ordering::Greater => 0,
                            std::cmp::Ordering::Equal => 1,
                            std::cmp::Ordering::Less => 2,
                        }
                    }
                };
                expected[a.configuration][i] += 1;
            }
        }
        let standings = rank_benchmarks(5, &entries).unwrap();
        for w in &standings {
            assert_eq!([w.wins, w.ties, w.losses], expected[w.configuration]);
        }
        for pair in standings.windows(2) {
            assert!(!pair[0].compare(&pair[1]).is_lt());
        }
    }
}
#[test]
fn scale_differences_do_not_dominate_matched_problem_wins_and_ties_are_stable() {
    let entries = [
        entry(0, 0, Some((-1_000_000, true))),
        entry(1, 0, Some((0, true))),
        entry(0, 1, Some((-1, true))),
        entry(1, 1, Some((-2, true))),
        entry(0, 2, Some((-1, true))),
        entry(1, 2, Some((-2, true))),
    ];
    let w = rank_benchmarks(2, &entries).unwrap();
    assert_eq!(w[0].configuration, 0);
    assert_eq!((w[0].wins, w[0].losses), (2, 1));
    assert!(
        w[0].summary
            .compare(&w[1].summary, BenchmarkRanking::TotalScore)
            .is_lt()
    );
    let entries = [
        entry(0, 0, Some((1, true))),
        entry(1, 0, Some((1, true))),
        entry(2, 0, Some((0, true))),
    ];
    let w = rank_benchmarks(3, &entries).unwrap();
    assert_eq!(
        w.iter()
            .map(|w| (w.configuration, w.rank))
            .collect::<Vec<_>>(),
        [(0, 1), (1, 1), (2, 3)]
    );
    let empty = rank_benchmarks::<usize>(2, &[]).unwrap();
    assert_eq!(empty.iter().map(|w| w.rank).collect::<Vec<_>>(), [1, 1]);
    assert!(matches!(
        rank_benchmarks(1, &[entry(1, 0, None)]),
        Err(BenchmarkStandingError::UnknownConfiguration {
            configuration: 1,
            count: 1
        })
    ));
}
#[test]
fn runner_matches_repetitions_excludes_warmups_and_rejects_incomplete_schedules() {
    let r = BenchmarkSchedule::default()
        .run(&[0, 1], &[0, 1], None, |&config, _, t| {
            Ok::<_, ()>(BenchmarkObservation {
                score: SimpleScore::new(if t.warmup { -config } else { config }),
                initialized: true,
                work: SolverWork::default(),
            })
        })
        .unwrap();
    let w = r.standings().unwrap();
    assert_eq!(w[0].configuration, 1);
    assert_eq!((w[0].wins, w[0].ties, w[0].losses), (6, 0, 0));
    assert_eq!(w[0].summary.run_count(), 6);
    let token = CancellationToken::new();
    token.cancel();
    let r = BenchmarkSchedule::default()
        .run(
            &[()],
            &[()],
            Some(&token),
            |_, _, _| -> Result<BenchmarkObservation<SimpleScore>, ()> { panic!("cancelled") },
        )
        .unwrap();
    assert!(matches!(
        r.standings(),
        Err(BenchmarkStandingError::IncompleteRun)
    ));
}
#[test]
fn large_repeated_tie_group_counts_pairs_without_materializing_them() {
    let entries: Vec<_> = (0..20_000)
        .map(|i| entry(i % 2, 0, Some((1, true))))
        .collect();
    let w = rank_benchmarks(2, &entries).unwrap();
    for result in w {
        assert_eq!(
            (result.wins, result.ties, result.losses, result.rank),
            (0, 100_000_000, 0, 1)
        );
    }
}
