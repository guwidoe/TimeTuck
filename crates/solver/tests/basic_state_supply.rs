//! Original native scoring, undo, publication and failure recovery tests.
use std::collections::BTreeMap;
use timetuck::*;
struct Solution;
struct Job;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Job>(vec![PlanningVariableDefinition::basic::<i32>(
            "worker", true,
        )]),
        PlanningEntityDefinition::of::<i32>(vec![
            PlanningVariableDefinition::shadow::<Vec<usize>>("jobs"),
            PlanningVariableDefinition::shadow::<usize>("wrong"),
        ]),
    ])
    .unwrap()
}
fn consume(update: BasicStateUpdate<'_, usize, i32>, table: &mut BTreeMap<i32, Vec<usize>>) {
    assert!(update.notifications.is_empty());
    for change in update.shadows {
        assert_eq!(table[&change.value], change.before);
        table.insert(change.value, change.after);
    }
}
#[test]
fn materialized_inverses_follow_scored_moves_and_undo_for_every_partial_assignment() {
    let s = schema();
    let field = s.entity::<Job>().unwrap().variable("worker").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    let mut supply = BasicStateSupply::new(r.basic::<usize, i32>(field).unwrap(), [0, 1]).unwrap();
    supply
        .externalize(s.entity::<i32>().unwrap().variable("jobs").unwrap())
        .unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 3]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source();
    let universe = b.source::<usize, i32>();
    let inverse = basic_inverse_stream(&mut b, &source, universe.port(), Some).unwrap();
    let matches = b
        .score("balance", &inverse, |(_, owners)| {
            SimpleScore::new(-(owners.len() as i64).pow(2))
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for i in 0..2 {
        tx.apply(&universe, TupleChange::put(i, i as i32)).unwrap();
    }
    tx.commit().unwrap();
    let mut d = StreamScoreDirector::new(&p, graph, source, p.initial_assignment()).unwrap();
    let bindings = [(0usize, 0), (1, 1), (2, 2)];
    r.synchronize_basic(field, d.assignment(), &bindings)
        .unwrap();
    let mut table = BTreeMap::from([(0, vec![]), (1, vec![])]);
    consume(supply.refresh_changed().unwrap(), &mut table);
    for code in 0..27 {
        let before = table.clone();
        let mut digits = code;
        let values = (0..3)
            .map(|_| {
                let n = digits % 3;
                digits /= 3;
                if n == 2 { None } else { Some(n) }
            })
            .collect::<Vec<_>>();
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let trial = d.apply(&movement).unwrap();
        r.synchronize_basic(field, trial.assignment(), &bindings)
            .unwrap();
        consume(supply.refresh_changed().unwrap(), &mut table);
        for value in 0..2 {
            let expected = values
                .iter()
                .enumerate()
                .filter_map(|(e, v)| (*v == Some(value)).then_some(e))
                .collect::<Vec<_>>();
            assert_eq!(table[&value], expected);
            assert_eq!(supply.inverse(&value), expected);
            assert_eq!(
                trial.graph().constraint_matches(&matches).unwrap()[&(value as usize)]
                    .0
                    .1
                    .keys()
                    .copied()
                    .collect::<Vec<_>>(),
                expected
            );
        }
        assert!(supply.refresh_changed().unwrap().shadows.is_empty());
        drop(trial);
        r.synchronize_basic(field, d.assignment(), &bindings)
            .unwrap();
        consume(supply.refresh_changed().unwrap(), &mut table);
        assert_eq!(table, before);
    }
    d.assert_consistent().unwrap();
}
#[test]
fn invalid_metadata_seeds_and_unknown_values_preserve_publication_and_can_recover() {
    let s = schema();
    let field = s.entity::<Job>().unwrap().variable("worker").unwrap();
    let target = s.entity::<i32>().unwrap().variable("jobs").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(field, [(0usize, Some(0i32)), (1, Some(0))])
        .unwrap();
    let state = r.basic::<usize, i32>(field).unwrap();
    assert!(BasicStateSupply::new(state.clone(), [0, 0]).is_err());
    assert!(BasicStateSupply::new(state.clone(), [1]).is_err());
    let mut supply = BasicStateSupply::new(state, [0, 1]).unwrap();
    assert!(supply.externalize(field).is_err());
    assert!(
        supply
            .externalize(s.entity::<i32>().unwrap().variable("wrong").unwrap())
            .is_err()
    );
    let other = schema();
    assert!(
        supply
            .externalize(other.entity::<i32>().unwrap().variable("jobs").unwrap())
            .is_err()
    );
    supply.externalize(target).unwrap();
    supply.seed_inverse([(0, vec![1, 0])]).unwrap();
    assert!(supply.refresh_changed().unwrap().shadows.is_empty());
    assert_eq!(supply.published_inverse(&0), Some([0, 1].as_slice()));
    assert!(supply.seed_inverse([(0, vec![]), (9, vec![4])]).is_err());
    assert!(supply.seed_inverse([(0, vec![]), (0, vec![4])]).is_err());
    assert!(supply.seed_inverse([(0, vec![1, 1])]).is_err());
    assert_eq!(supply.published_inverse(&0), Some([0, 1].as_slice()));
    let mut foreign = VariableStateRegistry::new(&s);
    assert!(supply.change(&mut foreign, 0, Some(1)).is_err());
    assert!(supply.change(&mut r, 99, Some(1)).is_err());
    assert!(supply.change(&mut r, 0, Some(9)).is_err());
    assert_eq!(supply.inverse(&0), [0, 1]);
    r.change_basic(field, 0usize, Some(9i32)).unwrap();
    assert!(supply.refresh_changed().is_err());
    assert_eq!(supply.published_inverse(&0), Some([0, 1].as_slice()));
    r.change_basic(field, 0usize, Some(1i32)).unwrap();
    let update = supply.refresh_changed().unwrap();
    assert_eq!(update.shadows.len(), 2);
    assert_eq!(supply.published_inverse(&0), Some([1].as_slice()));
    assert_eq!(supply.published_inverse(&1), Some([0].as_slice()));
    r.reset_basic(field, [] as [(usize, Option<i32>); 0])
        .unwrap();
    let update = supply.refresh_changed().unwrap();
    assert_eq!(update.shadows.len(), 2);
    assert!(update.shadows.iter().all(|change| change.after.is_empty()));
}
#[test]
fn fallback_notifications_preserve_membership_counts_and_coalesce_intermediate_edits() {
    let s = schema();
    let field = s.entity::<Job>().unwrap().variable("worker").unwrap();
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(field, [(0usize, Some(0i32)), (1, Some(0)), (2, None)])
        .unwrap();
    let mut supply = BasicStateSupply::new(r.basic::<usize, i32>(field).unwrap(), [0, 1]).unwrap();
    assert_eq!(supply.refresh_changed().unwrap().notifications, [0, 0]);
    r.change_basic(field, 0usize, Some(1i32)).unwrap();
    r.change_basic(field, 0usize, Some(0i32)).unwrap();
    assert!(supply.refresh_changed().unwrap().notifications.is_empty());
    r.update_basic(field, [(1usize, None::<i32>), (2, Some(1)), (3, Some(1))])
        .unwrap();
    assert_eq!(
        supply.refresh_changed().unwrap().notifications,
        [0, 0, 1, 1]
    );
    r.reset_basic(field, [] as [(usize, Option<i32>); 0])
        .unwrap();
    assert_eq!(supply.refresh_changed().unwrap().notifications, [1, 1]);
    assert!(supply.refresh_changed().unwrap().notifications.is_empty());
}
