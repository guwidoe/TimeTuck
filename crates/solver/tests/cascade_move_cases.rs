// Adapted from pinned Timefold Community MoveDirectorTest and single-cascade
// fixtures. Apache-2.0. See compat/provenance.json for exact attribution.
use timetuck::*;
#[test]
fn undo_cascading_update_shadow_variable() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![])]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let derived = list_cascade_stream(&mut b, &sources, &p, |c| Some(*c.value + 1)).unwrap();
    let matches = b
        .score("nonnull cascade", &derived, |(_, v)| {
            SimpleScore::new(i64::from(v.value.is_some()))
        })
        .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    assert!(
        d.graph()
            .constraint_matches(&matches)
            .unwrap()
            .values()
            .all(|(r, _)| r.1.value.is_none())
    );
    {
        let t = d
            .apply(&ListMove::Assign {
                value: 0,
                destination: ListPosition {
                    entity: 0,
                    index: 0,
                },
            })
            .unwrap();
        let rows = t.graph().constraint_matches(&matches).unwrap();
        assert_eq!(rows[&0].0.1.value, Some(1));
        assert_eq!(rows[&1].0.1.value, None);
        assert_eq!(t.graph().score(), SimpleScore::new(1));
    }
    assert!(
        d.graph()
            .constraint_matches(&matches)
            .unwrap()
            .values()
            .all(|(r, _)| r.1.value.is_none())
    );
    assert_eq!(d.graph().score(), SimpleScore::new(0));
    d.assert_consistent().unwrap();
}
