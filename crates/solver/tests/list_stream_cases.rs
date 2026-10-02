// Adapted from pinned Timefold Community AbstractUniConstraintStreamTest.
// Apache-2.0; all case and fixture/helper attribution is in compat/provenance.json.
use timetuck::*;
fn check(optional: bool, including: bool) {
    let p = ListProblem::new(
        vec!["v1", "v2"],
        vec![ListEntity::new(vec!["v1"]), ListEntity::new(vec![])],
    )
    .unwrap();
    let p = if optional { p.allowing_unassigned() } else { p };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let rows = b
        .filter(sources.values.port(), move |r| {
            including || r.position.is_some()
        })
        .unwrap();
    let ids = b.map(&rows, |r| r.value).unwrap();
    let terminal = b
        .score("testConstraint", &ids, |_| SimpleScore::new(-1))
        .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let assert_matches = |d: &ListStreamScoreDirector<'_, '_, _, _>, expected: Vec<&str>| {
        let score = SimpleScore::new(-(expected.len() as i64));
        assert_eq!(d.graph().score(), score);
        assert_eq!(
            d.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", score)]
        );
        let actual = d
            .graph()
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|v| (v, SimpleScore::new(-1)))
                .collect::<Vec<_>>()
        );
    };
    if including {
        assert_matches(&d, vec!["v1", "v2"]);
        return;
    }
    assert_matches(&d, vec!["v1"]);
    d.apply(&ListMove::Assign {
        value: "v2",
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_matches(&d, vec!["v1", "v2"]);
    d.apply(&ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 1,
    })
    .unwrap()
    .commit();
    assert_matches(&d, vec!["v2"]);
}
#[test]
fn for_each_list_var_not_allows_unassigned_values() {
    check(false, false)
}
#[test]
fn for_each_list_var_allows_unassigned_values() {
    check(true, false)
}
#[test]
fn for_each_including_unassigned_list_var_not_allows_unassigned_values() {
    check(false, true)
}
#[test]
fn for_each_including_unassigned_list_var_allows_unassigned_values() {
    check(true, true)
}
