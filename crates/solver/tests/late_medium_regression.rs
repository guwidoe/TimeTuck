// Original regressions for score-level-aware late-history resets.
use timetuck::*;
#[test]
fn medium_improvement_resets_history_without_changing_feasibility() {
    let initial = HardMediumSoftScore::new(-1, -100, -1000);
    let medium = HardMediumSoftScore::new(-1, 0, -1000);
    let mut state = LateAcceptance::new(3, false, initial).unwrap();
    state.step_ended(medium);
    assert_eq!(state.history(), &[medium; 3]);
    assert_eq!(medium.hard_cmp(&initial), std::cmp::Ordering::Equal);
    assert!(!medium.is_feasible());
    assert!(HardMediumSoftScore::new(0, -1000, -1000).is_feasible());
}
fn value(v: i32) -> HardMediumSoftScore {
    match v {
        0 => HardMediumSoftScore::new(-1, -100, -1000),
        1 => HardMediumSoftScore::new(-1, 0, -1000),
        _ => HardMediumSoftScore::new(-1, -50, 1),
    }
}
fn config() -> LocalSearchConfig<'static, HardMediumSoftScore> {
    LocalSearchConfig {
        acceptance: Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: false,
        },
        max_evaluations: 100,
        max_steps: 10,
        assert_scores: true,
        ..Default::default()
    }
}
#[test]
fn medium_improvement_prevents_basic_search_returning_to_old_thresholds() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| value(*v[0].unwrap()))
        .unwrap();
    let a = local_search(&p, &n, config()).unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", source.port(), |(_, v)| value(v.unwrap()))
        .unwrap();
    let a = stream_local_search(&p, b.build(), source, config()).unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
}
#[test]
fn list_and_mixed_backends_use_medium_resets() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("order", vec![ListScope::Entity(0)], |v| {
        let ListFact::Entity(v) = v[0] else {
            unreachable!()
        };
        value(v[0])
    })
    .unwrap();
    let a = list_local_search_from(&p, &n, p.initial_assignment(), config()).unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("order", sources.entities.port(), |(_, v)| value(v[0]))
        .unwrap();
    let a = list_stream_local_search_from(&p, b.build(), sources, p.initial_assignment(), config())
        .unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
    let p = MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), p);
    let mut n = MixedConstraintNetwork::new(0, 1, 2);
    n.add_constraint("order", vec![MixedScope::ListEntity(0)], |v| {
        let MixedFact::ListEntity(v) = v[0] else {
            unreachable!()
        };
        value(v[0])
    })
    .unwrap();
    let settings = MixedLocalSearchConfig {
        search: config(),
        joint_moves: true,
    };
    let a = mixed_local_search_from(&p, &n, p.initial_assignment(), settings).unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
    let mut b = StreamGraphBuilder::new();
    let sources = MixedStreamSources::new(&mut b);
    b.score("order", sources.lists.entities.port(), |(_, v)| value(v[0]))
        .unwrap();
    let a =
        mixed_stream_local_search_from(&p, b.build(), sources, p.initial_assignment(), settings)
            .unwrap();
    assert_eq!(a.completed_steps, 1);
    assert_eq!(a.termination, LocalSearchTermination::NoAcceptedMove);
    assert_eq!(a.best.unwrap().score, value(1));
}
#[test]
fn chronological_history_reference_covers_best_worsening_ties_and_cursor_wrap() {
    for size in [1, 2, 3, 7] {
        let initial = HardMediumSoftScore::new(-3, -2, -1);
        let mut m = LateAcceptance::new(size, false, initial).unwrap();
        let mut history = vec![initial; size];
        let mut best = initial;
        for i in 0..240 {
            let next = HardMediumSoftScore::new(
                (i % 19) as i64 - 4,
                ((i * 13) % 11) as i64 - 5,
                ((i * 7) % 17) as i64 - 8,
            );
            if next > best {
                if (next.hard(), next.medium()) > (best.hard(), best.medium()) {
                    history.fill(next);
                }
                best = next;
            }
            history[i % size] = next;
            m.step_ended(next);
            assert_eq!(m.history(), history);
            assert_eq!(m.threshold(), history[(i + 1) % size]);
        }
    }
}
#[test]
fn decimal_medium_reset_is_exact_and_other_families_keep_their_policy() {
    let before: HardMediumSoftDecimalScore =
        "-1hard/100000000000000000000.00000000000000000001medium/-3soft"
            .parse()
            .unwrap();
    let after: HardMediumSoftDecimalScore =
        "-1hard/100000000000000000000.00000000000000000002medium/-999soft"
            .parse()
            .unwrap();
    let mut m = LateAcceptance::new(3, false, before.clone()).unwrap();
    m.step_ended(after.clone());
    assert_eq!(m.history(), vec![after.clone(); 3]);
    assert_eq!(after.hard_cmp(&before), std::cmp::Ordering::Equal);
    let soft: HardMediumSoftDecimalScore =
        "-1hard/100000000000000000000.000000000000000000020medium/-998soft"
            .parse()
            .unwrap();
    m.step_ended(soft.clone());
    assert_eq!(m.history(), &[after.clone(), soft, after]);
    let before = BendableScore::<1, 2>::new([-1], [0, 0]);
    let after = BendableScore::new([-1], [1, 0]);
    let mut m = LateAcceptance::new(3, false, before).unwrap();
    m.step_ended(after);
    assert_eq!(m.history(), &[after, before, before]);
    let before =
        BendableDecimalScore::<1, 2>::new(["-1".parse().unwrap()], [Decimal::ZERO, Decimal::ZERO]);
    let after = BendableDecimalScore::new(
        ["-1".parse().unwrap()],
        ["1".parse().unwrap(), Decimal::ZERO],
    );
    let mut m = LateAcceptance::new(3, false, before.clone()).unwrap();
    m.step_ended(after.clone());
    assert_eq!(m.history(), &[after, before.clone(), before]);
    assert_eq!(
        SimpleScore::new(1).late_acceptance_cmp(&SimpleScore::ZERO),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        "1".parse::<SimpleDecimalScore>()
            .unwrap()
            .late_acceptance_cmp(&SimpleDecimalScore::ZERO),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        "0hard/2soft"
            .parse::<HardSoftDecimalScore>()
            .unwrap()
            .late_acceptance_cmp(&HardSoftDecimalScore::ZERO),
        std::cmp::Ordering::Equal
    );
    assert_eq!(
        HardMediumSoftScore::new(-1, 0, 2)
            .with_structural(-3)
            .late_acceptance_cmp(&HardMediumSoftScore::new(-1, 0, -2)),
        std::cmp::Ordering::Equal
    );
}
