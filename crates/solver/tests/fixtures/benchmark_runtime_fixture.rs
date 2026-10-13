// Adapted complete concrete TEST point fixtures; Apache-2.0, see provenance.
use timetuck::*;
pub fn input_points() -> Vec<(RecordStatistic, Vec<BenchmarkStatisticPoint>)> {
    use BenchmarkStatisticPoint::*;
    use RecordStatistic as K;
    let t = i64::MAX;
    let v = i64::from(i32::MAX);
    let score = SimpleScore::new(v);
    vec![
        (
            K::BestScore,
            vec![Score {
                time: t,
                score,
                initialized: true,
            }],
        ),
        (
            K::StepScore,
            vec![Score {
                time: t,
                score,
                initialized: true,
            }],
        ),
        (K::BestSolutionMutation, vec![Long { time: t, value: v }]),
        (
            K::MemoryUse,
            vec![BenchmarkStatisticPoint::linux_memory(t).unwrap()],
        ),
        (
            K::MoveCountPerStep,
            vec![Moves {
                time: t,
                accepted: 0,
                selected: t,
            }],
        ),
        (
            K::MoveCountPerType,
            vec![
                MoveType {
                    name: "Move Type 1".into(),
                    count: i64::MIN,
                },
                MoveType {
                    name: "Move Type 2".into(),
                    count: i64::MAX,
                },
            ],
        ),
        (K::MoveEvaluationSpeed, vec![Long { time: t, value: t }]),
        (K::ScoreCalculationSpeed, vec![Long { time: t, value: t }]),
        (
            K::ConstraintMatchTotalBestScore,
            vec![Constraint {
                time: t,
                name: "CP".into(),
                count: v,
                score,
            }],
        ),
        (
            K::ConstraintMatchTotalStepScore,
            vec![Constraint {
                time: t,
                name: "CP".into(),
                count: v,
                score,
            }],
        ),
        (
            K::PickedMoveTypeBestScoreDiff,
            vec![MoveScore {
                time: t,
                name: "SwapMove(A.class, B.class)".into(),
                score,
            }],
        ),
        (
            K::PickedMoveTypeStepScoreDiff,
            vec![MoveScore {
                time: t,
                name: "SwapMove(A.class, B.class)".into(),
                score,
            }],
        ),
    ]
}
pub fn assert_source_points(kind: RecordStatistic, points: &[BenchmarkStatisticPoint]) {
    let expected = input_points()
        .into_iter()
        .find(|(k, _)| *k == kind)
        .unwrap()
        .1;
    assert_eq!(points.len(), expected.len());
    if kind == RecordStatistic::MemoryUse {
        assert!(
            matches!(points[0],BenchmarkStatisticPoint::Memory{time:i64::MAX,used,maximum} if used>0&&maximum>0)
        );
    } else {
        assert_eq!(points, expected);
    }
}
pub fn temp(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("timetuck-runtime-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}
