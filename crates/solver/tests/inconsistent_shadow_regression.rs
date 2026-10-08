//! Original integration checks using an attributed concurrent-visit test fixture.
#[path = "support/concurrent_shadow_fixture.rs"]
mod fixture;
use std::cell::Cell;
use timetuck::*;
fn initial() -> ListDerivedShadows<&'static str, fixture::Schedule> {
    ListDerivedShadows {
        builtin: vec![],
        derived: Default::default(),
    }
}
#[test]
fn tolerant_refresh_preserves_flags_and_rolls_back_failed_scoring_then_recovers() {
    let schema = fixture::schema();
    let p = fixture::problem();
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut m = fixture::manager(&schema, &p, &calls, &fail);
    let mut s = ManagedSolution::new(p.initial_assignment(), initial());
    assert!(matches!(
        m.update(&mut s),
        Err(ShadowSolutionError::Shadows(
            ConsistentShadowUpdateError::Inconsistent(_)
        ))
    ));
    assert!(s.shadows.builtin.is_empty());
    assert_eq!(s.score, None);
    m = m.with_consistency_policy(ShadowConsistencyPolicy::AllowInconsistent);
    m.update(&mut s).unwrap();
    assert_eq!(s.score, Some(HardSoftScore::new(-4, 0)));
    assert_eq!(s.shadows.derived.inconsistent, [true; 4]);
    let before = s.shadows.clone();
    let graph_before = shadow_consistency(m.session()).unwrap();
    s.assignment = ListAssignment::new(vec![vec!["a1", "b1"], vec!["a2", "b2"]]);
    fail.set(true);
    assert!(matches!(
        m.update(&mut s),
        Err(ShadowSolutionError::Adapter(ModelShadowError::Adapter(
            "scoring"
        )))
    ));
    assert_eq!(s.shadows, before);
    assert_eq!(shadow_consistency(m.session()).unwrap(), graph_before);
    assert_eq!(s.score, Some(HardSoftScore::new(-4, 0)));
    fail.set(false);
    m.update(&mut s).unwrap();
    assert_eq!(s.shadows.derived.inconsistent, [false; 4]);
    assert_eq!(
        s.shadows.derived.ready,
        [Some(0), Some(0), Some(60), Some(60)]
    );
    assert_eq!(
        s.shadows.derived.start,
        [Some(0), Some(0), Some(60), Some(60)]
    );
    assert_eq!(
        s.shadows.derived.finish,
        [Some(30), Some(30), Some(90), Some(90)]
    );
    assert_eq!(
        s.shadows.derived.cascading,
        [Some(0), Some(0), Some(1440), Some(1440)]
    );
    assert_eq!(s.score, Some(HardSoftScore::new(0, -240)));
    s.assignment = p.initial_assignment();
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(s.shadows.derived.inconsistent, [true; 4]);
    assert_eq!(s.score, Some(HardSoftScore::new(0, -240)));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
        .unwrap();
    assert_eq!(s.score, Some(HardSoftScore::new(-4, 0)));
    m = m.with_consistency_policy(ShadowConsistencyPolicy::RequireConsistent);
    assert!(m.update(&mut s).is_err());
    assert_eq!(s.shadows.derived.inconsistent, [true; 4]);
}
#[test]
fn calculator_search_can_leave_a_cyclic_start_and_find_consistent_optimum() {
    let schema = fixture::schema();
    let p = fixture::problem();
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let factory = || {
        ShadowCalculator::new(
            fixture::manager(&schema, &p, &calls, &fail)
                .with_consistency_policy(ShadowConsistencyPolicy::AllowInconsistent),
            initial,
        )
    };
    let local = list_calculator_local_search_from(
        &p,
        p.initial_assignment(),
        factory,
        LocalSearchConfig {
            max_steps: 20,
            max_evaluations: 500,
            assert_scores: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(local.best.unwrap().score, HardSoftScore::new(0, -240));
    let result = solve_list_calculator(
        &p,
        &factory,
        SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.is_optimal());
    assert_eq!(result.score, Some(HardSoftScore::new(0, -240)));
    // Consistency requires one member of each concurrent pair on each route,
    // in the same group order; finish times are 30,30,90,90 relative minutes.
    for row in result.assignment.lists() {
        assert_eq!(row.len(), 2);
        assert_ne!(row[0].as_bytes()[0], row[1].as_bytes()[0]);
    }
    assert_eq!(
        result.assignment.lists()[0][0].as_bytes()[0],
        result.assignment.lists()[1][0].as_bytes()[0]
    );
}
#[test]
fn native_dependency_binding_validates_handles_before_readers_and_preserves_cache() {
    let schema = fixture::schema();
    let field = schema
        .entity::<fixture::Visit>()
        .unwrap()
        .variable("serviceFinishTime")
        .unwrap();
    let mut foreign = ShadowSessionBuilder::<usize, ()>::new(&schema);
    let foreign_node = foreign.derive(field, 0, |_, _| Some(0i64)).unwrap().erase();
    let calls = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, u8>::new(&schema);
    let source = b
        .derive(field, 0, |_, _| {
            calls.set(calls.get() + 1);
            Some(5i64)
        })
        .unwrap();
    let out = b
        .derive(field, 1, |_, i| {
            i.for_field::<i64>(field)
                .unwrap()
                .into_iter()
                .flatten()
                .copied()
                .sum::<i64>()
                .into()
        })
        .unwrap();
    let normal = source.erase();
    let cycle = out.erase();
    b.bind_dependencies(&out, move |mode| {
        Ok(vec![match mode {
            1 => foreign_node.clone(),
            2 => cycle.clone(),
            _ => normal.clone(),
        }])
    })
    .unwrap();
    assert!(b.set_dependencies(&out, &[source.erase()]).is_err());
    let mut g = b.build();
    update_shadows_with_policy(&mut g, &0, ShadowConsistencyPolicy::AllowInconsistent).unwrap();
    assert_eq!(g.read(&out).unwrap().value, Some(&5));
    let before = calls.get();
    assert!(matches!(
        update_shadows_with_policy(&mut g, &1, ShadowConsistencyPolicy::AllowInconsistent),
        Err(ConsistentShadowUpdateError::Session(
            ShadowSessionError::ForeignNode
        ))
    ));
    assert_eq!(calls.get(), before);
    assert_eq!(g.read(&out).unwrap().value, Some(&5));
    update_shadows_with_policy(&mut g, &2, ShadowConsistencyPolicy::AllowInconsistent).unwrap();
    assert!(g.read(&out).unwrap().inconsistent);
    update_shadows_with_policy(&mut g, &0, ShadowConsistencyPolicy::AllowInconsistent).unwrap();
    assert!(!g.read(&out).unwrap().inconsistent);
    assert_eq!(g.read(&out).unwrap().value, Some(&5));
}
