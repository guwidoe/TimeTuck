// Original nearby tree immutability, validation, real metric and legal-move checks.
use std::{cell::Cell, collections::HashSet};
use timetuck::*;
struct Meter;
struct OtherMeter;
struct Entity;
struct Solution;
fn basic() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("x", false),
    ])])
    .unwrap()
}
fn mixed() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("x", false),
        PlanningVariableDefinition::list::<i64>("list", false),
    ])])
    .unwrap()
}
fn spec() -> NearbySelectionConfig {
    NearbyMoveConfig::new(NearbyMoveKind::ListChange)
        .enable_nearby(PlanningType::of::<Meter>(), 0)
        .unwrap()
        .selector(NearbySelectorRole::Destination)
        .unwrap()
        .nearby
        .clone()
        .unwrap()
}
#[test]
fn nested_transform_preserves_inputs_avoids_existing_ids_and_is_idempotent() {
    let mut c = NearbyMoveConfig::new(NearbyMoveKind::Change);
    c.selector_mut(NearbySelectorRole::Entity).id = Some("nearby-7-0".into());
    c.selector_mut(NearbySelectorRole::Entity).order = Some(SelectionOrder::Probabilistic);
    let input = NearbyMoveConfig::union(vec![
        c,
        NearbyMoveConfig::union(vec![
            NearbyMoveConfig::new(NearbyMoveKind::ListSwap),
            NearbyMoveConfig::new(NearbyMoveKind::PillarSwap),
        ]),
    ]);
    let before = input.clone();
    let out = input.enable_nearby(PlanningType::of::<Meter>(), 7).unwrap();
    assert_eq!(input, before);
    assert_eq!(
        out.enable_nearby(PlanningType::of::<Meter>(), 7).unwrap(),
        out
    );
    let ids = [
        out.children[1]
            .selector(NearbySelectorRole::Entity)
            .unwrap()
            .id
            .as_ref()
            .unwrap(),
        out.children[2].children[1]
            .selector(NearbySelectorRole::Value)
            .unwrap()
            .id
            .as_ref()
            .unwrap(),
    ];
    assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), 2);
    assert!(ids.iter().all(|s| s.as_str() != "nearby-7-0"));
    assert_eq!(out.children[2].children[2].kind, NearbyMoveKind::PillarSwap);
}
#[test]
fn invalid_origins_sorting_and_mixed_basic_nearby_reject_while_list_nearby_is_valid() {
    let mut c = NearbyMoveConfig::new(NearbyMoveKind::Change)
        .enable_nearby(PlanningType::of::<Meter>(), 0)
        .unwrap();
    c.validate(&basic()).unwrap();
    assert_eq!(
        c.validate(&mixed()),
        Err(NearbyConfigError::MixedModel(NearbyMoveKind::Change))
    );
    c.selector_mut(NearbySelectorRole::Value).order = Some(SelectionOrder::Sorted);
    assert_eq!(c.validate(&basic()), Err(NearbyConfigError::NearbySorted));
    c.selector_mut(NearbySelectorRole::Value).order = None;
    c.selector_mut(NearbySelectorRole::Entity).id = Some("wrong".into());
    assert_eq!(c.validate(&basic()), Err(NearbyConfigError::InvalidOrigin));
    for kind in [
        NearbyMoveKind::ListChange,
        NearbyMoveKind::ListSwap,
        NearbyMoveKind::KOpt,
    ] {
        NearbyMoveConfig::new(kind)
            .enable_nearby(PlanningType::of::<Meter>(), 0)
            .unwrap()
            .validate(&mixed())
            .unwrap();
    }
    assert!(matches!(
        NearbyMoveConfig::new(NearbyMoveKind::PillarSwap)
            .enable_nearby(PlanningType::of::<Meter>(), 0),
        Err(NearbyConfigError::Unsupported(_))
    ));
}
#[test]
fn ranking_calls_actual_meters_once_preserves_ties_and_rejects_bad_inputs_before_work() {
    let calls = Cell::new(0);
    let mut registry = NearbyDistanceRegistry::default();
    registry
        .register::<Meter>(|o: &f64, c: &f64| {
            calls.set(calls.get() + 1);
            (c - o).abs()
        })
        .unwrap();
    let s = spec();
    assert_eq!(
        registry
            .rank(&s, &10.0, &[12.0, 8.0, 100.0, 10.0], 4)
            .unwrap(),
        vec![3, 0, 1, 2]
    );
    assert_eq!(calls.get(), 4);
    assert!(matches!(
        registry.rank(&s, &0.0, &[1.0, 2.0], 1),
        Err(NearbyConfigError::CandidateLimit { .. })
    ));
    assert_eq!(calls.get(), 4);
    let mut bad = s.clone();
    bad.meter = PlanningType::of::<OtherMeter>();
    assert_eq!(
        registry.rank(&bad, &0.0, &[1.0], 1),
        Err(NearbyConfigError::UnknownMeter(bad.meter))
    );
    assert_eq!(calls.get(), 4);
    assert!(matches!(
        registry.register::<Meter>(|_, _| 0.0),
        Err(NearbyConfigError::DuplicateMeter(_))
    ));
    assert_eq!(
        registry.rank(&s, &0.0, &[f64::NAN], 1),
        Err(NearbyConfigError::InvalidDistance { candidate: 0 })
    );
    assert!(registry.rank(&s, &0.0, &[], 0).unwrap().is_empty());
    let mut zeros = NearbyDistanceRegistry::default();
    zeros.register::<Meter>(|_: &(), c: &f64| *c).unwrap();
    assert_eq!(
        zeros.rank(&s, &(), &[0.0, -0.0, 1.0], 3).unwrap(),
        vec![0, 1, 2]
    );
    assert_eq!(
        zeros.rank(&s, &(), &[-1.0], 1),
        Err(NearbyConfigError::InvalidDistance { candidate: 0 })
    );
}
#[test]
fn configured_nearby_ranking_selects_and_executes_a_real_legal_list_move() {
    let p = ListProblem::new(
        vec![10_i64, 20, 100],
        vec![ListEntity::new(vec![10]), ListEntity::new(vec![20, 100])],
    )
    .unwrap();
    let before = p.initial_assignment();
    let candidates:Vec<_>=list_change_moves(&p,&before).unwrap().filter(|m|matches!(m,ListMove::Change {source,destination} if source.entity==0 && source.index==0 && destination.entity==1 && destination.index<2)).collect();
    assert_eq!(candidates.len(), 2);
    let movement = {
        let mut registry = NearbyDistanceRegistry::default();
        registry
            .register::<Meter>(|origin: &i64, m: &ListMove<i64>| {
                let ListMove::Change { destination, .. } = m else {
                    unreachable!()
                };
                (*origin - before.lists()[destination.entity][destination.index]).abs() as f64
            })
            .unwrap();
        let order = registry.rank(&spec(), &10, &candidates, 2).unwrap();
        candidates[order[0]].clone()
    };
    let mut after = before.clone();
    p.apply(&mut after, &movement).unwrap().commit();
    assert_eq!(after.lists(), &[vec![], vec![10, 20, 100]]);
    p.validate(&after).unwrap();
    assert_eq!(before, p.initial_assignment());
}
