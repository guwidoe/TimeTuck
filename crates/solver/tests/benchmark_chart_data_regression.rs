// Original native controls; no upstream method credit.
use timetuck::*;
#[test]
fn reduction_is_ordered_subset_with_endpoints_and_isolated_peaks() {
    for length in [3, 10, 100, 1001] {
        let points: Vec<_> = (0..length)
            .map(|i| ChartPoint {
                x: i as f64,
                y: Some(if i == length / 2 { 1000.0 } else { 0.0 }),
            })
            .collect();
        let s = BenchmarkSeries::new(points.clone()).unwrap();
        for budget in [2, 3, 7, 50] {
            let actual = s.downsample(budget).unwrap();
            assert_eq!(actual.len(), budget.min(length));
            assert_eq!(actual.first(), points.first());
            assert_eq!(actual.last(), points.last());
            assert!(actual.windows(2).all(|w| w[0].x < w[1].x));
            assert!(actual.iter().all(|p| points.contains(p)));
            if budget >= 3 && length > 3 {
                assert!(actual.iter().any(|p| p.y == Some(1000.0)));
            }
        }
    }
}
#[test]
fn gaps_preserve_run_endpoints_and_never_join_discontinuous_runs() {
    let points: Vec<_> = (0..100)
        .map(|i| ChartPoint {
            x: i as f64,
            y: if (20..40).contains(&i) || (70..75).contains(&i) {
                None
            } else {
                Some((i * i) as f64)
            },
        })
        .collect();
    let s = BenchmarkSeries::new(points).unwrap();
    let actual = s.downsample(12).unwrap();
    assert_eq!(actual.iter().filter(|p| p.y.is_some()).count(), 12);
    for x in [0, 19, 20, 39, 40, 69, 70, 74, 75, 99] {
        assert!(actual.iter().any(|p| p.x == x as f64));
    }
    assert!(matches!(
        s.downsample(5),
        Err(ChartDataError::BudgetTooSmall { minimum: 6 })
    ));
    let empty = BenchmarkSeries::new(vec![]).unwrap();
    assert!(empty.downsample(0).unwrap().is_empty());
    assert!(
        BenchmarkSeries::new(vec![ChartPoint {
            x: f64::NAN,
            y: Some(1.0)
        }])
        .is_err()
    );
    assert!(
        BenchmarkSeries::new(vec![
            ChartPoint {
                x: 1.0,
                y: Some(1.0)
            },
            ChartPoint { x: 1.0, y: None }
        ])
        .is_err()
    );
}
#[test]
fn exact_ticks_preserve_translation_and_decimal_scale_without_float_limits() {
    for span in [
        "1e-300", "3e-300", "4e-300", "1e300", "3e300", "4e300", "10.0000",
    ] {
        let span: Decimal = span.parse().unwrap();
        let step = benchmark_tick_step(&Decimal::ZERO, &span).unwrap();
        let translated = benchmark_tick_step(&span, &span.add(&span)).unwrap();
        assert_eq!(step, translated);
        assert!(step.numeric_cmp(&Decimal::ZERO).is_gt());
    }
    assert!(benchmark_tick_step(&Decimal::ZERO, &Decimal::ZERO).is_err());
    assert_eq!(format_benchmark_millis(Some(999)), "999 ms.");
    assert_eq!(
        format_benchmark_millis(Some(60_000)),
        "01:00.000 s. (60,000 ms.)"
    );
    assert!(format_benchmark_millis(Some(u64::MAX)).contains("18,446,744,073,709,551,615 ms."));
}
#[test]
fn nonuniform_series_selection_is_invariant_under_positive_affine_axes() {
    let points: Vec<_> = (0..64)
        .map(|i| ChartPoint {
            x: (i * i + i) as f64,
            y: Some(((i * 17) % 31) as f64),
        })
        .collect();
    let transformed: Vec<_> = points
        .iter()
        .map(|p| ChartPoint {
            x: 3.0 * p.x + 5.0,
            y: p.y.map(|v| 7.0 * v - 4.0),
        })
        .collect();
    for budget in [3, 9, 17, 32] {
        let a = BenchmarkSeries::new(points.clone())
            .unwrap()
            .downsample(budget)
            .unwrap();
        let b = BenchmarkSeries::new(transformed.clone())
            .unwrap()
            .downsample(budget)
            .unwrap();
        assert_eq!(a.len(), budget);
        assert_eq!(b.len(), budget);
        assert_eq!(
            a.into_iter()
                .map(|p| ChartPoint {
                    x: 3.0 * p.x + 5.0,
                    y: p.y.map(|v| 7.0 * v - 4.0)
                })
                .collect::<Vec<_>>(),
            b
        );
    }
}
