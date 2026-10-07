// Adapted from Timefold Community AbstractUniConstraintStreamTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn problem() -> FiniteProblem<&'static str> {
    FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2"])
            .with_initial("v1")
            .allowing_unassigned(),
        Variable::new(vec!["v1", "v2"]).allowing_unassigned(),
    ])
    .unwrap()
}
#[test]
fn for_each_basic_var_uninitialized() {
    let p = problem();
    let schema = BasicEntityRows::new(2, vec![vec![0], vec![1]]).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source();
    let rows = basic_entity_stream(&mut b, &source, &schema, false).unwrap();
    let matches = b.score("test", &rows, |_| SimpleScore::new(-1)).unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    let check = |d: &StreamScoreDirector<'_, '_, &'static str, SimpleScore>, expected: &[usize]| {
        assert_eq!(d.score(), SimpleScore::new(-(expected.len() as i64)));
        assert_eq!(
            d.graph()
                .constraint_matches(&matches)
                .unwrap()
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            expected
        );
    };
    check(&d, &[0]);
    d.apply(&Move::Change {
        variable: 1,
        value: Some("v2"),
    })
    .unwrap()
    .commit();
    check(&d, &[0, 1]);
    d.apply(&Move::Change {
        variable: 0,
        value: None,
    })
    .unwrap()
    .commit();
    check(&d, &[1]);
    d.assert_consistent().unwrap();
}
#[test]
fn for_each_including_unassigned_basic_var_uninitialized() {
    let p = problem();
    let schema = BasicEntityRows::new(2, vec![vec![0], vec![1]]).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source();
    let rows = basic_entity_stream(&mut b, &source, &schema, true).unwrap();
    let matches = b.score("test", &rows, |_| SimpleScore::new(-1)).unwrap();
    let d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-2));
    assert_eq!(
        d.graph()
            .constraint_matches(&matches)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [0, 1]
    );
}
