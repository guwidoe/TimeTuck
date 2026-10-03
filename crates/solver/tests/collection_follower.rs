//! Original collection-derived-value regressions; no upstream method credit.
#[path = "../examples/collection_follower_support/mod.rs"]
mod support;
use timetuck::*;
#[test]
fn utf16_order_and_equal_code_ties_preserve_first_referenced_identity() {
    // Java orders the surrogate pair for U+10000 before the BMP U+E000.
    // Distinct identities 2 and 3 share a code and must retain list order.
    let codes = vec![
        "\u{e000}".into(),
        "\u{10000}".into(),
        "same".into(),
        "same".into(),
    ];
    let deps = vec![
        vec![0, 1],
        vec![1, 0],
        vec![2, 3],
        vec![3, 2],
        vec![0, 0, 1],
    ];
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| Variable::new((0..4).collect()).with_initial(i))
            .collect(),
    )
    .unwrap();
    let (g, s, c) = support::graph(&deps, &codes);
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    let values = |g: &StreamGraph<'_, SimpleScore>| {
        g.constraint_matches(&c)
            .unwrap()
            .values()
            .map(|(v, _)| v.1)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        values(d.graph()),
        vec![Some(1), Some(1), Some(2), Some(3), Some(1)]
    );
    {
        let t = d
            .apply(&Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: None,
                },
                Move::Change {
                    variable: 1,
                    value: None,
                },
                Move::Change {
                    variable: 2,
                    value: None,
                },
            ]))
            .unwrap();
        assert_eq!(values(t.graph()), vec![None, None, Some(3), Some(3), None]);
    }
    assert_eq!(
        values(d.graph()),
        vec![Some(1), Some(1), Some(2), Some(3), Some(1)]
    );
    d.assert_consistent().unwrap();
}
#[test]
fn small_collection_search_matches_independent_full_scoring() {
    let deps = vec![vec![0, 1], vec![2, 1], vec![1, 0], vec![2, 2]];
    let codes = vec!["b".into(), "a".into(), "a".into()];
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
    let mut network = ConstraintNetwork::new(3);
    network
        .add_constraint("full", vec![0, 1, 2], |a| {
            let mut counts = std::collections::BTreeMap::new();
            for value in a {
                *counts.entry(value.copied()).or_insert(0_i64) += 1;
            }
            for refs in &deps {
                let mut best: Option<usize> = None;
                for &i in refs {
                    if let Some(&v) = a[i]
                        && best.is_none_or(|old| codes[v] < codes[old])
                    {
                        best = Some(v);
                    }
                }
                *counts.entry(best).or_insert(0) += 1;
            }
            SimpleScore::new(-counts.values().map(|n| n * n).sum::<i64>())
        })
        .unwrap();
    for cap in [1, 4, 12, 60] {
        let config = SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(cap),
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            }],
        };
        let expected = solve(&p, &network, config.clone()).unwrap();
        let actual = solve_stream(
            &p,
            || {
                let (g, s, _) = support::graph(&deps, &codes);
                Ok((g, s))
            },
            config,
        )
        .unwrap();
        assert_eq!(actual.assignment, expected.assignment);
        assert_eq!(actual.score, expected.score);
        assert_eq!(actual.work.move_evaluations, expected.work.move_evaluations);
    }
}
