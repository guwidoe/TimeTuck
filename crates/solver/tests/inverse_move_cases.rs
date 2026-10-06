// Adapted from Timefold Community MoveDirectorTest and mixed/singleentity
// fixtures, Apache-2.0. Exact attribution is in compat/provenance.json.
use timetuck::*;
#[test]
fn two_changes_in_a_row() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec!["valueA1", "valueA2"]); 2]).unwrap(),
        ListProblem::<i32>::new(vec![], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = MixedStreamSources::new(&mut b);
    let values = b.source::<usize, &str>();
    let inverse = basic_inverse_stream(&mut b, &s.basic, values.port(), |v| {
        (v == 0).then_some("entityA")
    })
    .unwrap();
    let m = b
        .score("inverse view", &inverse, |_| SimpleScore::new(0))
        .unwrap();
    let assigned = b
        .group_list(s.basic.port(), |(_, v)| ((), v.is_some()))
        .unwrap();
    b.score("Dummy constraint", &assigned, |(_, values)| {
        SimpleScore::new(-i64::from(values.len() == 2 && values.iter().all(|v| *v)))
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&values, TupleChange::put(0, "valueA1")).unwrap();
    tx.apply(&values, TupleChange::put(1, "valueA2")).unwrap();
    tx.commit().unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    {
        let mut session = d.edit_session().unwrap();
        for (variable, value) in ["valueA1", "valueA2"].into_iter().enumerate() {
            session
                .apply(&MixedMove {
                    basic: Move::Change {
                        variable,
                        value: Some(value),
                    },
                    lists: ListMove::Composite(vec![]),
                })
                .unwrap();
        }
        assert_eq!(
            session.assignment().basic().values(),
            &[Some("valueA1"), Some("valueA2")]
        );
        let rows = session.director().graph().constraint_matches(&m).unwrap();
        assert_eq!(
            rows[&0].0.1.keys().copied().collect::<Vec<_>>(),
            ["entityA"]
        );
        assert!(rows[&1].0.1.is_empty());
        assert_eq!(session.score(), SimpleScore::new(-1));
    }
    assert_eq!(d.assignment().basic().values(), &[None, None]);
    let rows = d.graph().constraint_matches(&m).unwrap();
    assert!(rows[&0].0.1.is_empty());
    assert!(rows[&1].0.1.is_empty());
    d.assert_consistent().unwrap();
}

#[test]
fn two_unassigns_in_a_row() {
    let p = ListProblem::new(
        vec!["valueA1", "valueA2", "valueA3"],
        vec![ListEntity::new(vec!["valueA1", "valueA2", "valueA3"])],
    )
    .unwrap();
    macro_rules! edit {
        ($d:ident) => {{
            let mut session = $d.edit_session().unwrap();
            for value in ["valueA2", "valueA3"] {
                let position = session.assignment().location(&value).unwrap();
                assert_eq!(
                    position.index, 1,
                    "the second removal must use its updated index"
                );
                session
                    .apply(&ListMove::Unassign {
                        entity: position.entity,
                        start: position.index,
                        end: position.index + 1,
                    })
                    .unwrap();
            }
            assert_eq!(session.assignment().lists(), &[vec!["valueA1"]]);
            session
        }};
    }
    let mut n = ListConstraintNetwork::new(1, 3);
    n.add_constraint("Dummy constraint", vec![ListScope::Entity(0)], |_| {
        SimpleScore::new(-1)
    })
    .unwrap();
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    drop(edit!(d));
    assert_eq!(d.assignment(), &p.initial_assignment());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    b.score("Dummy constraint", s.entities.port(), |_| {
        SimpleScore::new(-1)
    })
    .unwrap();
    let m = b
        .score("owner/index view", s.values.port(), |_| SimpleScore::new(0))
        .unwrap();
    let mut d = ListStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let before = d.graph().constraint_matches(&m).unwrap().clone();
    {
        let session = edit!(d);
        let rows = session.director().graph().constraint_matches(&m).unwrap();
        assert_eq!(
            rows[&0].0.position,
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        );
        assert!(rows[&1].0.position.is_none());
        assert!(rows[&2].0.position.is_none());
    }
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.graph().constraint_matches(&m).unwrap(), &before);
    d.assert_consistent().unwrap();
}
