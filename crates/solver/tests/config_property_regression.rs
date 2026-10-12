//! Original typed configuration mutation, wide integer and bounded iteration checks.
use timetuck::*;
#[test]
fn invalid_late_inputs_do_not_apply_earlier_setters() {
    let mut r = ConfigPropertyRegistry::<Vec<i32>>::default();
    r.register("value", |v, x: i32| v.push(x)).unwrap();
    assert!(matches!(
        r.register("value", |v, x: i32| v.push(x + 1)),
        Err(ConfigPropertyError::DuplicateRegistration(_))
    ));
    for inputs in [
        vec![("value", "2"), ("missing", "3")],
        vec![("value", "2"), ("value", "3")],
        vec![("value", "invalid")],
    ] {
        let mut v = vec![1];
        let e = r.apply(&mut v, "solver", inputs, "properties").unwrap_err();
        assert_eq!(v, vec![1]);
        assert!(e.to_string().contains("solver"));
    }
    let mut v = vec![];
    r.apply(&mut v, "solver", [("value", "2")], "properties")
        .unwrap();
    assert_eq!(v, vec![2]);
}
#[test]
fn wide_signed_ceiling_matches_exact_reference() {
    let values = [
        i64::MIN,
        i64::MIN + 1,
        -21,
        -1,
        0,
        1,
        21,
        i64::MAX - 1,
        i64::MAX,
    ];
    for a in values {
        for b in values {
            if b == 0 {
                continue;
            }
            let (a128, b128) = (i128::from(a), i128::from(b));
            let q = a128 / b128;
            let rem = a128 % b128;
            let want = q + i128::from(rem != 0 && (rem > 0) == (b128 > 0));
            assert_eq!(
                ceil_divide_config(a, b),
                i64::try_from(want).map_err(|_| ConfigDivisionError::Overflow)
            );
        }
    }
}
#[test]
fn abbreviation_consumes_four_items_from_an_unbounded_iterator() {
    let mut consumed = 0;
    let result = abbreviate_config(Some((0..).inspect(|_| consumed += 1)));
    assert_eq!(result, "0, 1, 2, ...");
    assert_eq!(consumed, 4);
}
#[test]
fn typed_properties_configure_a_real_solver_and_preserve_omitted_settings() {
    let mut registry = ConfigPropertyRegistry::<SolverConfig<'static>>::default();
    registry
        .register("maxSteps", |c, v: u64| c.local_search[0].max_steps = v)
        .unwrap();
    registry
        .register("assertScores", |c, v: bool| {
            c.limits.assert_phase_scores = v
        })
        .unwrap();
    let mut config = SolverConfig::default();
    registry
        .apply(
            &mut config,
            "solver",
            [("maxSteps", "3"), ("assertScores", "true")],
            "properties",
        )
        .unwrap();
    assert_eq!(config.local_search[0].max_steps, 3);
    assert!(config.limits.assert_phase_scores);
    assert_eq!(config.construction, ConstructionConfig::default());
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap_or(&0)))
    })
    .unwrap();
    let result = solve(&p, &n, config).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(1)));
}
