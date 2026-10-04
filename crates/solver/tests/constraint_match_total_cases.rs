// Adapted from Timefold Community ConstraintMatchTotalTest.getScoreTotal.
// Apache-2.0; exact source identity and substitutions: compat/provenance.json.
use timetuck::*;
#[test]
fn get_score_total() {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (&str, &str, i64)>();
    let constraint = builder
        .score("constraint1", source.port(), |(_, _, score)| {
            SimpleScore::new(*score)
        })
        .unwrap();
    let mut graph = builder.build();
    let snapshot = |graph: &StreamGraph<'_, SimpleScore>| {
        let mut all = ScoreExplanation::default();
        all.add_constraint(
            graph
                .explain_constraint_default(&constraint, |(a, b, _)| vec![*a, *b])
                .unwrap(),
        )
        .unwrap();
        all
    };
    let mut previous = snapshot(&graph);
    assert_eq!(previous.score, SimpleScore::ZERO);
    for (change, expected) in [
        (TupleChange::put(1, ("e1", "e2", -1)), -1),
        (TupleChange::put(2, ("e1", "e3", -20)), -21),
        (TupleChange::put(3, ("e2", "e1", -300)), -321),
        (TupleChange::retract(2), -301),
        (TupleChange::retract(1), -300),
        (TupleChange::retract(3), 0),
    ] {
        let mut tx = graph.transaction();
        tx.apply(&source, change).unwrap();
        tx.commit().unwrap();
        let current = snapshot(&graph);
        assert_eq!(current.score, SimpleScore::new(expected));
        // Original supplementary check: snapshot changes preserve the same totals.
        let diff = current.diff(&previous);
        assert_eq!(previous.score + diff.score_delta, current.score);
        previous = current;
    }
}
