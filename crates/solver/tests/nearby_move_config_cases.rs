// Complete adapted MoveSelectorConfigTest and five mixed-model capability tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct TestDistanceMeter;
fn enabled(c: &NearbyMoveConfig) -> NearbyMoveConfig {
    assert!(!c.has_nearby());
    assert!(c.kind.supports_auto_nearby());
    let n = c
        .enable_nearby(PlanningType::of::<TestDistanceMeter>(), 0)
        .unwrap();
    assert!(n.has_nearby());
    n
}
fn check_leaf(
    c: &NearbyMoveConfig,
    kind: NearbyMoveKind,
    origin: NearbySelectorRole,
    receiver: NearbySelectorRole,
) {
    assert_eq!(c.kind, kind);
    let o = c.selector(origin).unwrap();
    let r = c.selector(receiver).unwrap();
    let nearby = r.nearby.as_ref().unwrap();
    assert_eq!(nearby.origin_role, origin);
    assert_eq!(o.id.as_ref(), Some(&nearby.origin_id));
    assert_eq!(nearby.meter, PlanningType::of::<TestDistanceMeter>());
}
fn leaf(
    kind: NearbyMoveKind,
    origin: NearbySelectorRole,
    receiver: NearbySelectorRole,
    custom: NearbySelectorRole,
) {
    let mut c = NearbyMoveConfig::new(kind);
    check_leaf(&enabled(&c), kind, origin, receiver);
    c.selector_mut(custom).order = Some(SelectionOrder::Probabilistic);
    let n = enabled(&c);
    check_leaf(&n, kind, origin, receiver);
    assert_eq!(
        n.selector(custom).unwrap().order,
        Some(SelectionOrder::Probabilistic)
    );
}
#[test]
fn assert_enable_nearby_for_change() {
    leaf(
        NearbyMoveKind::Change,
        NearbySelectorRole::Entity,
        NearbySelectorRole::Value,
        NearbySelectorRole::Entity,
    );
}
#[test]
fn assert_enable_nearby_for_swap() {
    leaf(
        NearbyMoveKind::Swap,
        NearbySelectorRole::Entity,
        NearbySelectorRole::SecondaryEntity,
        NearbySelectorRole::Entity,
    );
}
#[test]
fn assert_enable_nearby_for_list_change() {
    leaf(
        NearbyMoveKind::ListChange,
        NearbySelectorRole::Value,
        NearbySelectorRole::Destination,
        NearbySelectorRole::Value,
    );
}
#[test]
fn assert_enable_nearby_for_list_swap() {
    leaf(
        NearbyMoveKind::ListSwap,
        NearbySelectorRole::Value,
        NearbySelectorRole::SecondaryValue,
        NearbySelectorRole::Value,
    );
}
#[test]
fn assert_enable_nearby_for_kopt() {
    leaf(
        NearbyMoveKind::KOpt,
        NearbySelectorRole::OriginValue,
        NearbySelectorRole::Value,
        NearbySelectorRole::Value,
    );
}
#[test]
fn assert_enable_nearby_for_union() {
    for custom in [false, true] {
        let mut change = NearbyMoveConfig::new(NearbyMoveKind::Change);
        if custom {
            change.selector_mut(NearbySelectorRole::Entity).order =
                Some(SelectionOrder::Probabilistic);
        }
        let c = NearbyMoveConfig::union(vec![change, NearbyMoveConfig::new(NearbyMoveKind::Swap)]);
        let n = enabled(&c);
        assert_eq!(n.children.len(), 4);
        assert_eq!(n.children[0].kind, NearbyMoveKind::Change);
        assert!(!n.children[0].has_nearby());
        check_leaf(
            &n.children[1],
            NearbyMoveKind::Change,
            NearbySelectorRole::Entity,
            NearbySelectorRole::Value,
        );
        assert_eq!(n.children[2].kind, NearbyMoveKind::Swap);
        assert!(!n.children[2].has_nearby());
        check_leaf(
            &n.children[3],
            NearbyMoveKind::Swap,
            NearbySelectorRole::Entity,
            NearbySelectorRole::SecondaryEntity,
        );
        if custom {
            assert_eq!(
                n.children[1]
                    .selector(NearbySelectorRole::Entity)
                    .unwrap()
                    .order,
                Some(SelectionOrder::Probabilistic)
            );
            assert_eq!(n.children[3].order, None);
        }
    }
}
#[test]
fn assert_enable_nearby_for_multiple_union() {
    let c = NearbyMoveConfig::union(vec![
        NearbyMoveConfig::union(vec![NearbyMoveConfig::new(NearbyMoveKind::Change)]),
        NearbyMoveConfig::union(vec![
            NearbyMoveConfig::new(NearbyMoveKind::Swap),
            NearbyMoveConfig::new(NearbyMoveKind::PillarSwap),
        ]),
    ]);
    let n = enabled(&c);
    assert_eq!(n.children.len(), 2);
    assert_eq!(n.children[0].kind, NearbyMoveKind::Union);
    assert!(!n.children[0].children[0].has_nearby());
    check_leaf(
        &n.children[0].children[1],
        NearbyMoveKind::Change,
        NearbySelectorRole::Entity,
        NearbySelectorRole::Value,
    );
    let swap = &n.children[1];
    assert_eq!(swap.kind, NearbyMoveKind::Union);
    assert_eq!(swap.children.len(), 3);
    assert!(!swap.children[0].has_nearby());
    check_leaf(
        &swap.children[1],
        NearbyMoveKind::Swap,
        NearbySelectorRole::Entity,
        NearbySelectorRole::SecondaryEntity,
    );
}
#[test]
fn assert_disabled_nearby_auto_configuration() {
    for kind in [
        NearbyMoveKind::Cartesian,
        NearbyMoveKind::IteratorFactory,
        NearbyMoveKind::ListFactory,
        NearbyMoveKind::SubListChange,
        NearbyMoveKind::SubListSwap,
        NearbyMoveKind::PillarChange,
        NearbyMoveKind::PillarSwap,
    ] {
        let c = NearbyMoveConfig::new(kind);
        assert!(!c.kind.supports_auto_nearby());
        assert!(!c.has_nearby());
    }
}
#[test]
fn change_enable_nearby_mixed_model() {
    assert!(!NearbyMoveKind::Change.supports_nearby_in_mixed());
}
#[test]
fn swap_enable_nearby_mixed_model() {
    assert!(!NearbyMoveKind::Swap.supports_nearby_in_mixed());
}
#[test]
fn list_change_enable_nearby_mixed_model() {
    assert!(NearbyMoveKind::ListChange.supports_nearby_in_mixed());
}
#[test]
fn list_swap_enable_nearby_mixed_model() {
    assert!(NearbyMoveKind::ListSwap.supports_nearby_in_mixed());
}
#[test]
fn kopt_enable_nearby_mixed_model() {
    assert!(NearbyMoveKind::KOpt.supports_nearby_in_mixed());
}
