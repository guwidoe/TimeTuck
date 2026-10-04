//! Original comparisons of retained explanations; no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
#[test]
fn diffs_capture_zero_delta_blame_changes_occurrence_changes_and_unchanged_constraints() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (u32, i64)>();
    let c = b
        .score("cost", source.port(), |(_, n)| {
            calls.set(calls.get() + 1);
            s(*n)
        })
        .unwrap();
    let zero = b.score("zero", source.port(), |_| s(0)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, (7, 1))).unwrap();
    tx.apply(&source, TupleChange::put(1, (7, 1))).unwrap();
    tx.commit().unwrap();
    let snapshot = |g: &StreamGraph<'_, SimpleScore>| {
        let mut all = ScoreExplanation::default();
        all.add_constraint(
            g.explain_constraint(&c, |(fact, _), _| ("fixed".to_owned(), vec![*fact]))
                .unwrap(),
        )
        .unwrap();
        // Constant explanation still preserves the match occurrence IDs.
        all.add_constraint(
            g.explain_constraint(&zero, |_, _| ("zero".to_owned(), vec![]))
                .unwrap(),
        )
        .unwrap();
        all
    };
    let before = snapshot(&g);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, (9, 1))).unwrap();
    tx.commit().unwrap();
    let after = snapshot(&g);
    let calls_before = calls.get();
    let diff = after.diff(&before);
    assert_eq!(calls.get(), calls_before);
    assert_eq!(diff.score_delta, s(0));
    assert_eq!(
        diff.constraints
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["cost"]
    );
    assert_eq!(diff.constraints["cost"].score_delta, s(0));
    assert_eq!(diff.indictments[&7].score_delta, s(-1));
    assert_eq!(diff.indictments[&9].score_delta, s(1));
    assert!(diff.indictments[&9].before.is_none());
    assert_eq!(
        diff.indictments[&7].after.as_ref().unwrap().matches.len(),
        1
    );
    let same = after.diff(&after);
    assert!(same.constraints.is_empty());
    assert!(same.indictments.is_empty());
    let reverse = before.diff(&after);
    for (fact, change) in &diff.indictments {
        assert_eq!(reverse.indictments[fact].before, change.after);
        assert_eq!(reverse.indictments[fact].after, change.before);
        assert_eq!(
            reverse.indictments[fact].score_delta,
            s(0) - change.score_delta
        );
    }
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    tx.apply(&source, TupleChange::put(2, (7, 1))).unwrap();
    tx.commit().unwrap();
    let occurrence = snapshot(&g).diff(&after);
    assert_eq!(occurrence.score_delta, s(0));
    assert_eq!(occurrence.constraints.len(), 2);
    assert_eq!(occurrence.indictments[&7].score_delta, s(0));
    assert_eq!(
        occurrence.indictments[&7]
            .after
            .as_ref()
            .unwrap()
            .matches
            .iter()
            .next()
            .unwrap()
            .tuple,
        2
    );
}
#[test]
fn diffs_distinguish_empty_constraints_zero_matches_and_justification_only_changes() {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, u32>();
    let c = b.score("empty", source.port(), |_| s(0)).unwrap();
    let mut g = b.build();
    let baseline: ScoreExplanation<usize, SimpleScore, String, u32> = ScoreExplanation::default();
    let snapshot = |g: &StreamGraph<'_, SimpleScore>, justification: &str| {
        let mut all = ScoreExplanation::default();
        all.add_constraint(
            g.explain_constraint(&c, |fact, _| (justification.to_owned(), vec![*fact]))
                .unwrap(),
        )
        .unwrap();
        all
    };
    let empty = snapshot(&g, "old");
    let addition = empty.diff(&baseline);
    assert_eq!(addition.constraints.len(), 1);
    assert!(addition.constraints["empty"].before.is_none());
    assert!(
        addition.constraints["empty"]
            .after
            .as_ref()
            .unwrap()
            .matches
            .is_empty()
    );
    assert_eq!(addition.score_delta, s(0));
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    let row = snapshot(&g, "old");
    let addition = row.diff(&empty);
    assert_eq!(addition.indictments[&7].score_delta, s(0));
    assert!(addition.indictments[&7].before.is_none());
    let changed = snapshot(&g, "new").diff(&row);
    assert_eq!(changed.constraints.len(), 1);
    assert!(changed.indictments.is_empty());
    assert_eq!(
        changed.constraints["empty"]
            .before
            .as_ref()
            .unwrap()
            .matches[&0]
            .justification,
        "old"
    );
    assert_eq!(
        changed.constraints["empty"].after.as_ref().unwrap().matches[&0].justification,
        "new"
    );
    let removal = baseline.diff(&row);
    assert!(removal.constraints["empty"].after.is_none());
    assert!(removal.indictments[&7].after.is_none());
}
#[test]
fn decimal_diff_keeps_small_changes_exact_and_snapshots_independent_of_rollback() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1i64, 2]).with_initial(1)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_decimal_weighted_constraint(
        "tiny",
        vec![0],
        "0.000000000000000001"
            .parse::<SimpleDecimalScore>()
            .unwrap(),
        |values| Decimal::new(values[0].copied().unwrap_or(0).into(), 18),
    )
    .unwrap();
    let mut director = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let snapshot = |d: &ScoreDirector<'_, '_, i64, SimpleDecimalScore>| {
        d.explain_default(|_, scope, _| scope.to_vec())
    };
    let before = snapshot(&director);
    let change = Move::Change {
        variable: 0,
        value: Some(2),
    };
    drop(director.apply(&change).unwrap());
    assert_eq!(snapshot(&director), before);
    director.apply(&change).unwrap().commit();
    let after = snapshot(&director);
    let delta = after.matches.diff(&before.matches);
    assert_eq!(
        delta.score_delta,
        SimpleDecimalScore::new(Decimal::new(1.into(), 36))
    );
    assert_eq!(delta.indictments[&0].score_delta, delta.score_delta);
    assert_eq!(
        delta.constraints["tiny"].before.as_ref().unwrap().score,
        before.matches.score
    );
    assert_eq!(
        delta.constraints["tiny"].after.as_ref().unwrap().score,
        after.matches.score
    );
}
