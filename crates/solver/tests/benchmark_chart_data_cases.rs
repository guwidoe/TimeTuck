// Adapted Timefold Community TEST methods/CSV resources, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn step_size() {
    for (difference, expected) in [
        ("0.000012", "0.0000001"),
        ("0.000089", "0.000001"),
        ("0.00012", "0.000001"),
        ("0.0012", "0.00001"),
        ("0.012", "0.0001"),
        ("0.12", "0.001"),
        ("1", "0.01"),
        ("12", "0.1"),
        ("123", "1"),
        ("1234", "10"),
        ("12345", "100"),
        ("98765", "1000"),
        ("42179797364", "1000000000"),
    ] {
        let difference: Decimal = difference.parse().unwrap();
        let expected: Decimal = expected.parse().unwrap();
        assert!(
            benchmark_tick_step(&Decimal::ZERO, &difference)
                .unwrap()
                .numeric_cmp(&expected)
                .is_eq()
        );
    }
}
#[test]
fn test_down_sampling_process() {
    let x: Vec<f64> = include_str!("fixtures/benchmark_chart/xValues.csv")
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().parse().unwrap())
        .collect();
    let y: Vec<Option<f64>> = include_str!("fixtures/benchmark_chart/yValues.csv")
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            if v.trim() == "null" {
                None
            } else {
                Some(v.trim().parse().unwrap())
            }
        })
        .collect();
    let expected: Vec<(f64, f64)> = include_str!("fixtures/benchmark_chart/resultValues.csv")
        .split(',')
        .filter(|v| !v.trim().is_empty())
        .map(|v| {
            let (x, y) = v.trim().split_once('/').unwrap();
            (x.parse().unwrap(), y.parse().unwrap())
        })
        .collect();
    assert_eq!(x.len(), y.len());
    assert_eq!((x.len(), expected.len()), (8932, 3840));
    // Source builder omits null y rows. Native explicit resolution replaces
    // chart presentation configuration; expected data never enters the runtime.
    let series = BenchmarkSeries::new(
        x.into_iter()
            .zip(y)
            .filter_map(|(x, y)| y.map(|y| ChartPoint { x, y: Some(y) }))
            .collect(),
    )
    .unwrap();
    let actual = series.downsample(3840).unwrap();
    assert_eq!(actual.len(), 3840);
    assert!(actual.len() < series.points().len());
    let keys: Vec<_> = actual.iter().map(|p| p.x).collect();
    let values: Vec<_> = actual.iter().filter_map(|p| p.y).collect();
    for (x, y) in expected {
        assert!(keys.contains(&x), "missing source key {x}");
        assert!(values.contains(&y), "missing source value {y}");
    }
}
#[test]
fn millisecond_formatting() {
    for (millis, expected) in [
        (None, "None."),
        (Some(0), "0 ms."),
        (Some(2089), "2.089 s. (2,089 ms.)"),
        (Some(346089), "05:46.089 s. (346,089 ms.)"),
        (Some(19346089), "05:22:26.089 s. (19,346,089 ms.)"),
        (Some(119346089), "01:09:09:06.089 s. (119,346,089 ms.)"),
    ] {
        assert_eq!(format_benchmark_millis(millis), expected);
    }
}
