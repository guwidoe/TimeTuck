use timetuck::*;
struct Solution;
type Move<'s> = InspectedBasicMove<'s, usize, usize>;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<usize>(vec![
        PlanningVariableDefinition::basic::<usize>("first", true),
        PlanningVariableDefinition::basic::<usize>("second", true),
    ])])
    .unwrap()
}
#[test]
fn actual_multi_column_swap_and_pillar_change_preserve_retained_values() {
    let s = schema();
    let first = s.entity::<usize>().unwrap().variable("first").unwrap();
    let second = s.entity::<usize>().unwrap().variable("second").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(
        first,
        [
            (0usize, Some(10usize)),
            (1, Some(10)),
            (2, Some(20)),
            (3, Some(20)),
        ],
    )
    .unwrap();
    r.reset_basic(
        second,
        [
            (0usize, None::<usize>),
            (1, None),
            (2, Some(30)),
            (3, Some(30)),
        ],
    )
    .unwrap();
    let m = Move::pillar_swap(&mut r, vec![second, first], vec![1, 0], vec![3, 2]).unwrap();
    assert_eq!(m.variables(), &[second, first]);
    assert_eq!(m.original_left_values(), &[None, Some(10)]);
    let before = m.describe(|e| e.to_string(), |v| v.to_string());
    m.execute(&mut r).unwrap();
    assert_eq!(
        r.basic::<usize, usize>(first).unwrap().snapshot(),
        [(0, Some(20)), (1, Some(20)), (2, Some(10)), (3, Some(10))].into()
    );
    assert_eq!(
        r.basic::<usize, usize>(second).unwrap().snapshot(),
        [(0, Some(30)), (1, Some(30)), (2, None), (3, None)].into()
    );
    assert_eq!(m.describe(|e| e.to_string(), |v| v.to_string()), before);
    let snapshot = r.basic::<usize, usize>(first).unwrap().snapshot();
    assert!(m.execute(&mut r).is_err());
    assert_eq!(r.basic::<usize, usize>(first).unwrap().snapshot(), snapshot);
    let change = Move::pillar_change(&mut r, second, vec![2, 3], Some(99)).unwrap();
    change.execute(&mut r).unwrap();
    assert_eq!(change.original_left_values(), &[None]);
    assert_eq!(
        r.basic::<usize, usize>(second).unwrap().snapshot()[&2],
        Some(99)
    );
}
#[test]
fn invalid_metadata_unknown_groups_and_stale_sources_do_not_publish() {
    let s = schema();
    let v = s.entity::<usize>().unwrap().variable("first").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(v, [(0usize, None::<usize>), (1, Some(5))])
        .unwrap();
    assert!(Move::pillar_change(&mut r, v, Vec::<usize>::new(), Some(1usize)).is_err());
    assert!(Move::pillar_change(&mut r, v, vec![0, 1], Some(1usize)).is_err());
    assert!(Move::change(&mut r, v, 99, Some(1usize)).is_err());
    assert!(InspectedBasicMove::<usize, &str>::change(&mut r, v, 0, Some("wrong type")).is_err());
    let other = schema();
    let foreign = other.entity::<usize>().unwrap().variable("first").unwrap();
    assert!(Move::change(&mut r, foreign, 0, Some(1usize)).is_err());
    let m = Move::swap(&mut r, vec![v], 0, 1).unwrap();
    r.change_basic(v, 1usize, Some(7usize)).unwrap();
    let before = r.basic::<usize, usize>(v).unwrap().snapshot();
    assert!(m.execute(&mut r).is_err());
    assert_eq!(r.basic::<usize, usize>(v).unwrap().snapshot(), before);
}
