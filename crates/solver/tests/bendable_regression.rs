// Original boundaries and integration tests; not upstream coverage credit.
use timetuck::{
    Assignment, BendableScore as B, ConstraintNetwork, ConstructionConfig, FiniteProblem,
    LocalSearchConfig, Move, Score, ScoreDirector, SimulatedAnnealing, Variable, exhaustive_search,
    greedy_construction, local_search,
};

#[test]
fn wrapping_checked_arithmetic_and_structural_clearing_cover_both_groups() {
    let a = B::new([i64::MAX, i64::MIN], [i64::MIN, i64::MAX]).with_structural(-3);
    let one = B::new([1, -1], [-1, 1]).with_structural(4);
    assert_eq!(a + one, B::new([i64::MIN, i64::MAX], [i64::MAX, i64::MIN]));
    assert_eq!(a.checked_add(one), None);
    assert_eq!(a.checked_sub(-one), None);
    assert_eq!(a.checked_mul(-1), None);
    assert_eq!((-a).hard_levels(), [-i64::MAX, i64::MIN]);
    assert_eq!(a.abs(), B::new([i64::MAX, i64::MIN], [i64::MIN, i64::MAX]));
    let small = B::new([2, -3], [4, -5]).with_structural(-8);
    assert_eq!(small.checked_mul(3), Some(B::new([6, -9], [12, -15])));
    assert_eq!(small.checked_sub(small), Some(B::ZERO));
    assert_eq!(small.checked_add(-small), Some(B::ZERO));
    assert_eq!(small.multiply(0.0), B::ZERO);
    // Catch overflow in a soft level after every hard operation succeeded.
    assert_eq!(B::new([0], [i64::MAX]).checked_add(B::new([0], [1])), None);
    assert_eq!(B::new([0], [i64::MIN]).checked_sub(B::new([0], [1])), None);
    assert_eq!(B::new([0], [i64::MAX]).checked_mul(2), None);
}

#[test]
fn shapes_empty_groups_and_index_bounds_are_explicit() {
    let empty = B::<0, 0>::ZERO;
    assert!(empty.is_feasible() && empty.is_zero());
    assert_eq!(empty.to_string(), "[]hard/[]soft");
    assert_eq!(empty.to_short_string(), "0");
    assert_eq!(empty.level(0), None);
    assert_eq!(B::<0, 0>::of_hard(0, 1), None);
    assert_eq!(B::<0, 0>::of_soft(0, 1), None);
    assert_eq!(B::<0, 0>::ZERO.checked_mul(i64::MAX), Some(empty));
    let hard = B::new([0, 1], []);
    assert_eq!(hard.level(2), None);
    assert_eq!(hard.level(usize::MAX), None);
    assert_eq!(B::<2, 0>::of_hard(2, 7), None);
    assert_eq!(
        B::new([], [-1, -2]).ordinary_levels().collect::<Vec<_>>(),
        [-1, -2]
    );
    let mut copy = hard.hard_levels();
    copy[0] = 9;
    assert_eq!(copy, [9, 1]);
    assert_eq!(hard.hard_levels(), [0, 1]);
}

#[test]
fn parser_rejects_wrong_dimensions_and_malformed_groups() {
    type Score = B<2, 1>;
    for bad in [
        "",
        "0",
        "[0]hard/[0]soft",
        "[0/1]hard/[0/1]soft",
        "[0/1/]hard/[0]soft",
        "[0//1]hard/[0]soft",
        "[0/1]soft/[0]hard",
        "[ 0/1]hard/[0]soft",
        "[0/1]hard/[9223372036854775808]soft",
        "[]hard/[]soft",
        "0structural/[0/1]hard/[0]soft/trailing",
    ] {
        assert!(bad.parse::<Score>().is_err(), "{bad}");
    }
    assert_eq!("[]hard/[]soft".parse::<B<0, 0>>(), Ok(B::ZERO));
    assert_eq!(
        "[*/1]hard/[-7]soft".parse::<Score>(),
        Ok(Score::new([i64::MIN, 1], [-7]))
    );
    let negative = Score::new([0, 1], [-9]).with_structural(-2);
    assert_eq!(negative.to_string().parse::<Score>(), Ok(negative));
    let positive = negative.with_structural(2);
    assert_eq!(positive.to_string(), "[0/1]hard/[-9]soft");
    assert_eq!(positive.to_short_string(), "2structural/[0/1]hard/[-9]soft");
}

#[test]
fn hard_priority_feasibility_and_annealing_see_all_levels_in_order() {
    let a = B::new([0, 0], [1, 2]);
    assert_eq!(a.ordinary_levels().collect::<Vec<_>>(), [0, 0, 1, 2]);
    let b = B::new([1, -1], [i64::MIN, i64::MIN]);
    assert!(b > a);
    assert!(!b.is_feasible());
    assert_eq!(
        a.hard_cmp(&a.with_structural(-2)),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        a.hard_cmp(&B::new([0, 1], [-100, -100])),
        std::cmp::Ordering::Less
    );
    let cooling = SimulatedAnnealing::new(B::new([1, 2], [4, 8]), 2, 0).unwrap();
    assert_eq!(
        cooling.probability_at(B::ZERO, B::new([-1, -2], [-4, -8]), 0.0),
        Ok((-4.0_f64).exp())
    );
}

fn network() -> ConstraintNetwork<'static, i32, B<2, 2>> {
    let mut net = ConstraintNetwork::new(2);
    net.add_constraint("collision", vec![0, 1], |v| {
        B::new([-i64::from(v[0].is_some() && v[0] == v[1]), 0], [0, 0])
    })
    .unwrap();
    net.add_constraint("first", vec![0], |v| {
        B::new([0, -i64::from(v[0].copied().unwrap_or(0))], [0, 0])
    })
    .unwrap();
    net.add_constraint("second", vec![1], |v| {
        let b = i64::from(v[0].copied().unwrap_or(0));
        B::new([0, 0], [-7 * b, 2 * b])
    })
    .unwrap();
    net
}

#[test]
fn scoped_transactions_preserve_all_levels_and_rollback() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let net = network();
    let mut director =
        ScoreDirector::new(&model, &net, Assignment::new(vec![Some(0), Some(0)])).unwrap();
    let original = director.score();
    {
        let trial = director
            .apply(&Move::Change {
                variable: 1,
                value: Some(1),
            })
            .unwrap();
        assert_eq!(trial.score(), B::new([0, 0], [-7, 2]));
    }
    assert_eq!(director.score(), original);
    director.assert_consistent().unwrap();
    director
        .apply(&Move::Change {
            variable: 1,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(director.score(), B::new([0, 0], [-7, 2]));
    director.assert_consistent().unwrap();
}

#[test]
fn construction_and_local_search_respect_multiple_hard_priorities() {
    let model = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let net = network();
    let exact = exhaustive_search(
        &model,
        |a| {
            let x = i64::from(a.values()[0].unwrap());
            let y = i64::from(a.values()[1].unwrap());
            B::new([-i64::from(x == y), -x], [-7 * y, 2 * y])
        },
        4,
    );
    assert!(exact.is_optimal());
    let optimum = exact.best.unwrap();
    assert_eq!(optimum.score, B::new([0, 0], [-7, 2]));
    let constructed = greedy_construction(
        &model,
        &net,
        ConstructionConfig {
            assert_scores: true,
            ..ConstructionConfig::default()
        },
    )
    .unwrap();
    assert_eq!(constructed.solution().unwrap().score, optimum.score);
    let searched = local_search(
        &model,
        &net,
        LocalSearchConfig {
            assert_scores: true,
            ..LocalSearchConfig::default()
        },
    )
    .unwrap();
    assert_eq!(searched.best.unwrap(), optimum);
}

#[test]
fn integer_round_trips_cover_longer_fixed_shapes() {
    // Exercise arrays beyond the two shapes used by upstream test fixtures.
    for i in -50_i64..=50 {
        let value = B::new([i, i * 2, i * 3], [i * 4, 0, i * 5, i * 6]).with_structural(-i.abs());
        assert_eq!(value.to_string().parse::<B<3, 4>>(), Ok(value));
        assert_eq!((value - value).structural(), 0);
        assert_eq!((value - value), B::ZERO);
    }
}
