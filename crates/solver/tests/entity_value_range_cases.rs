// Adapted from pinned Timefold Community ChangeMoveProviderTest and test fixtures.
// SPDX-License-Identifier: Apache-2.0
// Native slot IDs and per-variable domains replace entity objects and range annotations.
use timetuck::*;
struct Solution;
struct Entity;
const FIRST: &str = "Generated Value 0";
const SECOND: &str = "Generated Value 1";
fn provider(optional: bool, crossing: Option<bool>) -> SchemaMoveProvider {
    let s = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<&'static str>("value", optional),
    ])])
    .unwrap();
    let mut config = SchemaMoveConfig::new(SchemaMoveKind::Change);
    config.crossing_unassigned = crossing;
    config
        .resolve(&s)
        .unwrap()
        .bind(&[SchemaVariableSlots {
            variable: s
                .genuine_entity::<Entity>()
                .unwrap()
                .genuine_variable("value")
                .unwrap(),
            slots: vec![0, 1],
        }])
        .unwrap()
}
fn entity_problem(optional: bool) -> FiniteProblem<&'static str> {
    // generateSolution(2,2) starts both ranges with FIRST, then adds each current
    // value if missing. The second range therefore also contains SECOND.
    let mut a = Variable::new(vec![FIRST]).with_initial(FIRST);
    let mut b = Variable::new(vec![FIRST, SECOND]).with_initial(SECOND);
    if optional {
        a = a.allowing_unassigned();
        b = b.allowing_unassigned();
    }
    FiniteProblem::new(vec![a, b]).unwrap()
}
#[test]
fn solution_range_ignores_registered_facts_outside_the_range() {
    let facts = [FIRST, SECOND, "third"];
    let p = FiniteProblem::new(vec![
        Variable::new(facts[..2].to_vec()).with_initial(FIRST),
        Variable::new(facts[..2].to_vec()).with_initial(SECOND),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let q = provider(false, None);
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert!(moves.contains(&Move::Change {
        variable: 0,
        value: Some(SECOND)
    }));
    assert!(moves.contains(&Move::Change {
        variable: 1,
        value: Some(FIRST)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 0,
        value: Some(FIRST)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 1,
        value: Some(SECOND)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 0,
        value: Some(facts[2])
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 1,
        value: Some(facts[2])
    }));
}
#[test]
fn entity_ranges_propose_only_other_legal_values() {
    let p = entity_problem(false);
    let a = p.initial_assignment();
    let q = provider(false, None);
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert!(moves.contains(&Move::Change {
        variable: 1,
        value: Some(FIRST)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 0,
        value: Some(FIRST)
    }));
    assert_eq!(
        moves,
        vec![Move::Change {
            variable: 1,
            value: Some(FIRST)
        }]
    );
}
#[test]
fn optional_entity_ranges_honor_disabled_crossing() {
    let p = entity_problem(true);
    let a = p.initial_assignment();
    let q = provider(true, Some(false));
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert!(moves.contains(&Move::Change {
        variable: 1,
        value: Some(FIRST)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 0,
        value: Some(FIRST)
    }));
    assert_eq!(
        moves,
        vec![Move::Change {
            variable: 1,
            value: Some(FIRST)
        }]
    );
}
