// Adapted from pinned Single/MultiConstraintAssertionTest, AbstractUniConstraintStreamTest,
// and shadow/multiplelistener and shadow/dependency fixtures. Apache-2.0;
// see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Owner(usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Value(usize);
#[derive(Clone)]
struct Fact {
    id: Value,
    location: Option<VariableListLocation<Owner, Value>>,
    cascade: Option<i64>,
}
struct Solution {
    values: Vec<Fact>,
    lists: Vec<(Owner, Vec<Value>)>,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![PlanningVariableDefinition::shadow::<i64>(
            "cascade",
        )]),
    ])
    .unwrap()
}
fn generated(value_count: usize) -> Solution {
    Solution {
        values: (0..value_count)
            .map(|id| Fact {
                id: Value(id),
                location: None,
                cascade: None,
            })
            .collect(),
        lists: vec![(Owner(0), vec![Value(0)])],
    }
}
fn set_all_shadows(solution: &mut Solution) {
    let schema = schema();
    let list = schema
        .entity::<Owner>()
        .unwrap()
        .variable("values")
        .unwrap();
    let target = schema
        .entity::<Value>()
        .unwrap()
        .variable("cascade")
        .unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry.reset_lists(list, solution.lists.clone()).unwrap();
    let state = registry.list::<Owner, Value>(list).unwrap();
    let session = ListShadowSession::new(
        state.clone(),
        target,
        solution.values.iter().map(|v| v.id),
        |ctx| ctx.location.map(|p| p.index as i64 + 10),
    )
    .unwrap();
    // Explicit preparation writes native derived fields only when requested.
    for value in &mut solution.values {
        value.location = state.location(&value.id);
        value.cascade = session.values().unwrap()[&value.id].value;
    }
}
#[derive(Clone, Copy)]
enum Mode {
    Penalty,
    Reward,
    All,
}
type Assertion = ConstraintAssertion<SimpleScore, (SimpleScore, Value)>;
fn verify(solution: &Solution, mode: Mode) -> (Option<Assertion>, ScoreAssertion<SimpleScore>) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<Value, Fact>();
    let assigned = b.filter(source.port(), |v| v.location.is_some()).unwrap();
    let penalty = if !matches!(mode, Mode::Reward) {
        Some(
            track_constraint(
                &mut b,
                "Penalize by cascade values",
                &assigned,
                SimpleScore::new(1),
                |v| -v.cascade.unwrap_or(2),
            )
            .unwrap(),
        )
    } else {
        None
    };
    let reward = if !matches!(mode, Mode::Penalty) {
        Some(
            track_constraint(
                &mut b,
                "Reward by cascade values",
                &assigned,
                SimpleScore::new(1),
                |v| v.cascade.unwrap_or(2) * 2,
            )
            .unwrap(),
        )
    } else {
        None
    };
    let mut g = b.build();
    let mut tx = g.transaction();
    for v in &solution.values {
        tx.apply(&source, TupleChange::put(v.id, v.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    let single = match mode {
        Mode::Penalty => penalty.as_ref(),
        Mode::Reward => reward.as_ref(),
        Mode::All => None,
    }
    .map(|c| constraint_assertion(&g, c, |v, s| (*s, v.id)).unwrap());
    (single, score_assertion(&g))
}
#[test]
fn single_update_shadow_variables_list_single_solution() {
    let mut solution = generated(2);
    set_all_shadows(&mut solution);
    verify(&solution, Mode::Penalty)
        .0
        .unwrap()
        .check(
            ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly(Decimal::from(10))),
            None,
        )
        .unwrap();
    set_all_shadows(&mut solution);
    verify(&solution, Mode::Reward)
        .0
        .unwrap()
        .check(
            ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly(Decimal::from(20))),
            None,
        )
        .unwrap();
    set_all_shadows(&mut solution);
    verify(&solution, Mode::Penalty)
        .0
        .unwrap()
        .check_justifications(
            &[(SimpleScore::new(-10), Value(0))],
            JustificationMatch::Contains,
            None,
        )
        .unwrap();
    assert_eq!(
        solution.values[0].location,
        Some(VariableListLocation {
            entity: Owner(0),
            index: 0,
            previous: None,
            next: None
        })
    );
    assert!(solution.values[1].location.is_none());
}
#[test]
fn multi_update_shadow_variables_list_single_solution() {
    let mut solution = generated(1);
    set_all_shadows(&mut solution);
    verify(&solution, Mode::All)
        .1
        .check(ConstraintComparison::Exactly(SimpleScore::new(10)), None)
        .unwrap();
}
#[derive(Clone, Copy)]
enum Selection {
    Assigned,
    IncludingUnassigned,
    Unfiltered,
}
fn reference(entity: Value, variable: &str) -> VerificationShadowRef<Value> {
    VerificationShadowRef {
        entity,
        variable: variable.into(),
    }
}
fn selection_case(selection: Selection) {
    let schema = schema();
    let list = schema
        .entity::<Owner>()
        .unwrap()
        .variable("values")
        .unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    registry
        .reset_lists::<Owner, Value>(list, [(Owner(0), vec![]), (Owner(1), vec![])])
        .unwrap();
    let state = registry.list::<Owner, Value>(list).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<Value, (Value, bool, bool)>();
    let visible = b
        .filter(
            source.port(),
            move |(_, assigned, inconsistent)| match selection {
                Selection::Assigned => *assigned && !inconsistent,
                Selection::IncludingUnassigned => !inconsistent,
                Selection::Unfiltered => true,
            },
        )
        .unwrap();
    let c = b
        .score("testConstraint", &visible, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut update = |rows: Vec<(Owner, Vec<Value>)>, expected: &[Value]| {
        registry.update_lists(list, rows).unwrap();
        let metadata: Vec<_> = (0..2)
            .map(|id| {
                let id = Value(id);
                let mut dependencies = if id == Value(1) {
                    vec![reference(Value(0), "endTime")]
                } else {
                    vec![]
                };
                if let Some(previous) = state.location(&id).and_then(|p| p.previous) {
                    dependencies.push(reference(previous, "endTime"));
                }
                VerificationEntity {
                    id,
                    inconsistent: None,
                    shadows: vec![
                        VerificationShadow {
                            variable: "startTime".into(),
                            dependencies,
                        },
                        VerificationShadow {
                            variable: "endTime".into(),
                            dependencies: vec![reference(id, "startTime")],
                        },
                    ],
                }
            })
            .collect();
        let flags = verification_consistency(&metadata, [Value(0), Value(1)]).unwrap();
        let mut tx = g.transaction();
        for id in [Value(0), Value(1)] {
            tx.apply(
                &source,
                TupleChange::put(
                    id,
                    (
                        id,
                        state.is_assigned(&id),
                        flags.is_inconsistent(&id).unwrap(),
                    ),
                ),
            )
            .unwrap();
        }
        tx.commit().unwrap();
        let mut matches: Vec<_> = g
            .constraint_matches(&c)
            .unwrap()
            .values()
            .map(|((id, _, _), s)| {
                assert_eq!(*s, SimpleScore::new(-1));
                *id
            })
            .collect();
        matches.sort();
        assert_eq!(matches, expected);
        assert_eq!(g.score(), SimpleScore::new(-(expected.len() as i64)));
    };
    let both = &[Value(0), Value(1)];
    update(
        vec![
            (
                Owner(0),
                if matches!(selection, Selection::Assigned) {
                    vec![Value(0), Value(1)]
                } else {
                    vec![]
                },
            ),
            (Owner(1), vec![]),
        ],
        both,
    );
    update(
        vec![(Owner(0), vec![Value(1), Value(0)]), (Owner(1), vec![])],
        if matches!(selection, Selection::Unfiltered) {
            both
        } else {
            &[]
        },
    );
    // IncludingUnassigned and Unfiltered source methods write e1 twice in their
    // last transition. Preserve that final list snapshot instead of fixing it.
    let final_rows = if matches!(selection, Selection::Assigned) {
        vec![(Owner(0), vec![Value(1)]), (Owner(1), vec![Value(0)])]
    } else {
        vec![(Owner(0), vec![Value(0)]), (Owner(1), vec![])]
    };
    update(final_rows, both);
}
#[test]
fn for_each_including_unassigned_exclude_inconsistent_entities() {
    selection_case(Selection::IncludingUnassigned);
}
#[test]
fn for_each_exclude_inconsistent_entities() {
    selection_case(Selection::Assigned);
}
#[test]
fn for_each_unfiltered_include_unassigned_and_inconsistent_entities() {
    selection_case(Selection::Unfiltered);
}
