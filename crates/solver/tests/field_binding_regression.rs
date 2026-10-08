//! Original typed field integration checks; no upstream credit.
use timetuck::*;
struct Solution;
struct Basic;
struct Lists;
fn schema(optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Basic>(vec![
            PlanningVariableDefinition::basic::<i32>("value", optional),
            PlanningVariableDefinition::shadow::<i32>("shadow"),
        ]),
        PlanningEntityDefinition::of::<Lists>(vec![PlanningVariableDefinition::list::<i32>(
            "tasks", optional,
        )]),
    ])
    .unwrap()
}
fn basic(s: &PlanningSchema, slots: Vec<usize>) -> SchemaVariableSlots<'_> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Basic>()
            .unwrap()
            .genuine_variable("value")
            .unwrap(),
        slots,
    }
}
fn lists(s: &PlanningSchema, slots: Vec<usize>) -> SchemaVariableSlots<'_> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Lists>()
            .unwrap()
            .genuine_variable("tasks")
            .unwrap(),
        slots,
    }
}
fn pos(row: usize, index: usize) -> FieldListPosition {
    FieldListPosition { row, index }
}
fn basic_problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0).with_pinned(true),
        Variable::new(vec![1, 2]).with_initial(1),
    ])
    .unwrap()
}
fn list_problem() -> ListProblem<i32> {
    ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3]),
            ListEntity::new(vec![4]).with_pinned(true),
        ],
    )
    .unwrap()
}
#[test]
fn binding_rejects_kind_type_policy_alias_and_bounds_errors() {
    let s = schema(false);
    let p = basic_problem();
    let l = list_problem();
    assert!(matches!(
        lists(&s, vec![0]).bind_basic(&p),
        Err(FieldBindingError::WrongKind)
    ));
    assert!(matches!(
        basic(&s, vec![0]).bind_list(&l),
        Err(FieldBindingError::WrongKind)
    ));
    let wrong = FiniteProblem::new(vec![Variable::new(vec![0u8])]).unwrap();
    assert!(matches!(
        basic(&s, vec![0]).bind_basic(&wrong),
        Err(FieldBindingError::WrongValueType { .. })
    ));
    for slots in [vec![9], vec![0, 0]] {
        assert!(basic(&s, slots.clone()).bind_basic(&p).is_err());
        assert!(lists(&s, slots).bind_list(&l).is_err());
    }
    let optional = schema(true);
    assert!(matches!(
        basic(&optional, vec![0]).bind_basic(&p),
        Err(FieldBindingError::UnassignedPolicy)
    ));
    assert!(matches!(
        lists(&optional, vec![0]).bind_list(&l),
        Err(FieldBindingError::UnassignedPolicy)
    ));
    let shadow = SchemaVariableSlots {
        variable: s
            .genuine_entity::<Basic>()
            .unwrap()
            .variable("shadow")
            .unwrap(),
        slots: vec![0],
    };
    assert!(matches!(
        shadow.bind_basic(&p),
        Err(FieldBindingError::WrongKind)
    ));
    let field = basic(&s, vec![1]).bind_basic(&p).unwrap();
    assert!(matches!(
        field.change(1, Some(2)),
        Err(FieldBindingError::RowOutOfBounds { .. })
    ));
    assert!(matches!(
        field.read(&Assignment::new(vec![]), 0),
        Err(FieldBindingError::AssignmentSize { .. })
    ));
    let field = lists(&s, vec![1]).bind_list(&l).unwrap();
    assert!(matches!(
        field.value_at(&l.initial_assignment(), pos(0, 1)),
        Err(FieldBindingError::IndexOutOfBounds { .. })
    ));
    assert!(matches!(
        field.position_of(&ListAssignment::new(vec![]), &1),
        Err(FieldBindingError::AssignmentSize { .. })
    ));
}
#[test]
fn borrowed_ranges_and_positions_follow_binding_order_and_scope() {
    let p = basic_problem();
    let field = {
        let s = schema(false);
        basic(&s, vec![1, 0]).bind_basic(&p).unwrap()
    };
    assert_eq!(field.name(), "value");
    assert_eq!(field.slots(), &[1, 0]);
    assert_eq!(
        field.range(0).unwrap().iter().collect::<Vec<_>>(),
        vec![Some(&1), Some(&2)]
    );
    assert!(!field.range(0).unwrap().contains(&None));
    assert_eq!(field.range(0).unwrap().get(u128::MAX), None);
    assert!(!field.is_pinned(0).unwrap());
    assert!(field.is_pinned(1).unwrap());
    assert!(!field.is_value_in_range(1, Some(&2)).unwrap());
    let s = schema(true);
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]).allowing_unassigned()]).unwrap();
    let field = basic(&s, vec![0]).bind_basic(&p).unwrap();
    let range = field.range(0).unwrap();
    assert_eq!(range.len(), 3);
    assert_eq!(
        range.iter().collect::<Vec<_>>(),
        vec![Some(&1), Some(&2), None]
    );
    assert_eq!(range.get(3), None);
    assert!(field.is_value_in_range(0, None).unwrap());
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2, 3]),
            ListEntity::new(vec![3]).with_value_range(vec![2, 3]),
            ListEntity::new(vec![4]).with_pinned(true),
        ],
    )
    .unwrap();
    let s = schema(false);
    let field = lists(&s, vec![1, 0]).bind_list(&p).unwrap();
    let a = p.initial_assignment();
    assert_eq!(field.range(0).unwrap(), &[2, 3]);
    assert!(!field.is_value_in_range(0, &1).unwrap());
    assert_eq!(field.position_of(&a, &4).unwrap(), None);
    assert_eq!(field.position_of(&a, &99).unwrap(), None);
    assert_eq!(field.position_of(&a, &2).unwrap(), Some(pos(1, 1)));
    let pinned = lists(&s, vec![2]).bind_list(&p).unwrap();
    assert!(pinned.is_pinned(0).unwrap());
    assert_eq!(pinned.first_unpinned_index(&a, 0).unwrap(), 1);
}
#[test]
fn constructed_list_moves_are_scored_atomically_and_sessions_restore_live_reads() {
    let s = schema(true);
    let p = ListProblem::new(
        (0..7).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4, 5]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let field = lists(&s, vec![1, 0]).bind_list(&p).unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 7);
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let before = d.assignment().clone();
    let moves = vec![
        field.assign(6, pos(0, 3)).unwrap(),
        field.relocate(pos(0, 0), pos(1, 3)).unwrap(),
        field.swap(pos(0, 0), pos(1, 1)).unwrap(),
        field.relocate_span(0, 0..2, pos(1, 1), true).unwrap(),
        field.swap_spans(0, 0..2, 1, 1..3, true).unwrap(),
        field.unassign(0, 0..2).unwrap(),
    ];
    for movement in moves {
        let mut edits = d.edit_session().unwrap();
        edits.apply(&movement).unwrap();
        for row in 0..2 {
            for (index, v) in field
                .read(edits.assignment(), row)
                .unwrap()
                .iter()
                .enumerate()
            {
                assert_eq!(
                    field.position_of(edits.assignment(), v).unwrap(),
                    Some(pos(row, index))
                );
            }
        }
        edits.assert_consistent().unwrap();
        drop(edits);
        assert_eq!(d.assignment(), &before);
    }
    let mut edits = d.edit_session().unwrap();
    edits.apply(&field.assign(6, pos(0, 3)).unwrap()).unwrap();
    assert_eq!(
        field.position_of(edits.assignment(), &6).unwrap(),
        Some(pos(0, 3))
    );
    {
        let mut inner = edits.nested().unwrap();
        inner.apply(&field.unassign(0, 3..4).unwrap()).unwrap();
        assert_eq!(field.position_of(inner.assignment(), &6).unwrap(), None);
    }
    assert_eq!(
        field.position_of(edits.assignment(), &6).unwrap(),
        Some(pos(0, 3))
    );
    let successful = edits.assignment().clone();
    assert!(
        edits
            .apply(&field.relocate(pos(1, 0), pos(0, 0)).unwrap())
            .is_err()
    );
    assert_eq!(edits.assignment(), &successful);
    drop(edits);
    assert_eq!(d.assignment(), &before);
}
#[test]
fn basic_bound_custom_commands_evaluate_then_commit_on_both_backends() {
    let s = schema(false);
    let p = basic_problem();
    let field = basic(&s, vec![1, 0]).bind_basic(&p).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().map(|x| i64::from(*x.unwrap())).sum())
    })
    .unwrap();
    macro_rules! command {
        () => {
            |c: &mut CustomPhaseContext<'_, Assignment<i32>, Move<i32>, SimpleScore, _>| {
                let m = field.change(0, Some(2)).unwrap();
                assert_eq!(field.read(c.assignment(), 0).unwrap(), Some(&1));
                c.evaluate(&m, |a, score| {
                    assert_eq!(field.read(a, 0).unwrap(), Some(&2));
                    assert_eq!(score, SimpleScore::new(2));
                })?;
                assert_eq!(field.read(c.assignment(), 0).unwrap(), Some(&1));
                c.execute(&m)?;
                Ok(())
            }
        };
    }
    let a = custom_phase(
        &p,
        &n,
        p.initial_assignment(),
        CustomPhaseConfig::default(),
        command!(),
    )
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("sum", source.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let z = stream_custom_phase(
        &p,
        b.build(),
        source,
        p.initial_assignment(),
        CustomPhaseConfig::default(),
        command!(),
    )
    .unwrap();
    assert_eq!(a.score, Some(SimpleScore::new(2)));
    assert_eq!(a.score, z.score);
    assert_eq!(a.assignment, z.assignment);
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    assert!(d.apply(&field.swap(0, 1).unwrap()).is_err());
}
#[test]
fn list_and_mixed_bound_custom_commands_share_the_same_scored_moves() {
    let s = schema(false);
    let p = MixedProblem::new(basic_problem(), list_problem());
    let basic = basic(&s, vec![1, 0]).bind_basic(p.basic()).unwrap();
    let list = lists(&s, vec![1, 0]).bind_list(p.lists()).unwrap();
    let lm = list.swap(pos(0, 0), pos(1, 1)).unwrap();
    let score = |v: &[i32]| SimpleScore::new(if v == [2] { 10 } else { 0 });
    let mut ln = ListConstraintNetwork::new(3, 4);
    ln.add_constraint("list", vec![ListScope::Entity(1)], |v| {
        let ListFact::Entity(v) = v[0] else { panic!() };
        score(v)
    })
    .unwrap();
    macro_rules! list_command {
        () => {
            |c: &mut CustomPhaseContext<'_, ListAssignment<i32>, ListMove<i32>, SimpleScore, _>| {
                c.evaluate(&lm, |a, score| {
                    assert_eq!(list.position_of(a, &2).unwrap(), Some(pos(0, 0)));
                    assert_eq!(score, SimpleScore::new(10));
                })?;
                assert_eq!(
                    list.position_of(c.assignment(), &2).unwrap(),
                    Some(pos(1, 1))
                );
                c.execute(&lm)?;
                Ok(())
            }
        };
    }
    let a = list_custom_phase(
        p.lists(),
        &ln,
        p.lists().initial_assignment(),
        CustomPhaseConfig::default(),
        list_command!(),
    )
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = ListStreamSources::new(&mut b);
    b.score("list", source.entities.port(), |(i, v)| {
        if *i == 1 {
            score(v)
        } else {
            SimpleScore::new(0)
        }
    })
    .unwrap();
    let z = list_stream_custom_phase(
        p.lists(),
        b.build(),
        source,
        p.lists().initial_assignment(),
        CustomPhaseConfig::default(),
        list_command!(),
    )
    .unwrap();
    assert_eq!(a.score, Some(SimpleScore::new(10)));
    assert_eq!(a.score, z.score);
    assert_eq!(a.assignment, z.assignment);
    let m = MixedMove {
        basic: basic.change(0, Some(2)).unwrap(),
        lists: lm,
    };
    let mut n = MixedConstraintNetwork::new(2, 3, 4);
    n.add_constraint("basic", vec![MixedScope::Basic(1)], |v| {
        let MixedFact::Basic(v) = v[0] else { panic!() };
        SimpleScore::new(i64::from(*v.unwrap()))
    })
    .unwrap();
    n.add_constraint("list", vec![MixedScope::ListEntity(1)], |v| {
        let MixedFact::ListEntity(v) = v[0] else {
            panic!()
        };
        score(v)
    })
    .unwrap();
    macro_rules! mixed_command {
        () => {
            |c: &mut CustomPhaseContext<
                '_,
                MixedAssignment<i32, i32>,
                MixedMove<i32, i32>,
                SimpleScore,
                _,
            >| {
                c.evaluate(&m, |a, score| {
                    assert_eq!(basic.read(a.basic(), 0).unwrap(), Some(&2));
                    assert_eq!(list.read(a.lists(), 0).unwrap(), &[2]);
                    assert_eq!(score, SimpleScore::new(12));
                })?;
                assert_eq!(basic.read(c.assignment().basic(), 0).unwrap(), Some(&1));
                assert_eq!(list.read(c.assignment().lists(), 0).unwrap(), &[3]);
                c.execute(&m)?;
                Ok(())
            }
        };
    }
    let a = mixed_custom_phase(
        &p,
        &n,
        p.initial_assignment(),
        CustomPhaseConfig::default(),
        mixed_command!(),
    )
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = MixedStreamSources::new(&mut b);
    b.score("basic", source.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    b.score("list", source.lists.entities.port(), |(i, v)| {
        if *i == 1 {
            score(v)
        } else {
            SimpleScore::new(0)
        }
    })
    .unwrap();
    let z = mixed_stream_custom_phase(
        &p,
        b.build(),
        source,
        p.initial_assignment(),
        CustomPhaseConfig::default(),
        mixed_command!(),
    )
    .unwrap();
    assert_eq!(a.score, Some(SimpleScore::new(12)));
    assert_eq!(a.score, z.score);
    assert_eq!(a.assignment, z.assignment);
}
