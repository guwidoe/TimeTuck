//! Original explanation regressions; no upstream method credit.
use std::cell::Cell;
use timetuck::*;

#[test]
fn overlapping_blame_deduplicates_per_match_but_preserves_occurrences_and_constraints() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let source = b.source::<usize, (Vec<u32>, i64)>();
    let penalty = b
        .score("penalty", source.port(), |(_, n)| {
            calls.set(calls.get() + 1);
            HardSoftScore::new(-n, 0)
        })
        .unwrap();
    let reward = b
        .score("reward", source.port(), |(_, n)| HardSoftScore::new(0, *n))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (id, row) in [
        (0, (vec![7, 7, 9], 3)),
        (1, (vec![7], 2)),
        (2, (vec![], 4)),
        (3, (vec![9], 0)),
    ] {
        tx.apply(&source, TupleChange::put(id, row)).unwrap();
    }
    tx.commit().unwrap();
    let before = calls.get();
    let p = g
        .explain_constraint_default(&penalty, |(facts, _)| facts.clone())
        .unwrap();
    assert_eq!(calls.get(), before);
    assert_eq!(p.matches[&0].justification.facts, [7, 7, 9]);
    assert_eq!(
        p.matches[&0]
            .indicted_facts
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [7, 9]
    );
    assert_eq!(p.indictments[&7].score, HardSoftScore::new(-5, 0));
    assert_eq!(
        p.indictments[&9]
            .matches
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [0, 3]
    );
    assert_eq!(p.matches.len(), 4); // Includes zero score and no-blame matches.
    let r = g
        .explain_constraint_default(&reward, |(facts, _)| facts.clone())
        .unwrap();
    let mut all = ScoreExplanation::default();
    all.add_constraint(p.clone()).unwrap();
    all.add_constraint(r).unwrap();
    assert_eq!(all.score, g.score());
    assert_eq!(all.indictments[&7].score, HardSoftScore::new(-5, 5));
    assert_eq!(all.indictments[&7].matches.len(), 4);
    let snapshot = all.clone();
    assert_eq!(
        all.add_constraint(p),
        Err(GraphError::DuplicateConstraint("penalty".into()))
    );
    assert_eq!(all, snapshot);
    assert_eq!(calls.get(), before);
}

#[test]
fn snapshots_follow_commits_survive_rollback_and_never_rescore() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let source = b.source::<usize, (&str, &str)>();
    let cost = b
        .score("cost", source.port(), |(_, n)| {
            calls.set(calls.get() + 1);
            n.parse().unwrap()
        })
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, ("a", "0.000000000000000001")))
        .unwrap();
    tx.apply(&source, TupleChange::put(1, ("a", "0.000000000000000001")))
        .unwrap();
    tx.commit().unwrap();
    let old = g
        .explain_constraint_default(&cost, |(fact, _)| vec![*fact])
        .unwrap();
    assert_eq!(
        old.indictments["a"].score,
        "0.000000000000000002".parse().unwrap()
    );
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, ("b", "-9.5")))
            .unwrap();
        tx.apply(&source, TupleChange::retract(1)).unwrap();
    }
    let before = calls.get();
    assert_eq!(
        g.explain_constraint_default(&cost, |(fact, _)| vec![*fact])
            .unwrap(),
        old
    );
    assert_eq!(calls.get(), before);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, ("b", "-9.5")))
        .unwrap();
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    let new = g
        .explain_constraint(&cost, |(fact, _), score| {
            (
                format!("fact={fact}; score={score:?}"),
                vec!["custom blame"],
            )
        })
        .unwrap();
    assert_eq!(new.score, "-9.5".parse().unwrap());
    assert_eq!(new.indictments.len(), 1);
    assert!(new.indictments.contains_key("custom blame"));
    assert!(new.matches[&0].justification.starts_with("fact=b;"));
    assert_eq!(old.matches.len(), 2);
    assert_eq!(old.indictments["a"].matches.len(), 2);
    g.assert_consistent().unwrap();
}

#[test]
fn empty_foreign_and_panicking_explanations_leave_graph_untouched() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, u32>();
    let c = b
        .score("c", source.port(), |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let empty = g.explain_constraint_default(&c, |v| vec![*v]).unwrap();
    assert_eq!(empty.name, "c");
    assert_eq!(empty.score, SimpleScore::ZERO);
    assert!(empty.matches.is_empty());
    let foreign = StreamGraphBuilder::<SimpleScore>::new().build();
    assert_eq!(foreign.constraint_name(&c), Err(GraphError::ForeignPort));
    assert_eq!(
        foreign.explain_constraint_default(&c, |_| panic!("must validate first")),
        Err::<
            ConstraintExplanation<usize, SimpleScore, DefaultJustification<SimpleScore, u32>, u32>,
            _,
        >(GraphError::ForeignPort)
    );
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(1, 7)).unwrap();
    tx.commit().unwrap();
    let snapshot = g.explain_constraint_default(&c, |v| vec![*v]).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<ConstraintExplanation<usize, SimpleScore, (), u32>, _> =
                g.explain_constraint(&c, |_, _| panic!("presentation callback"));
        }))
        .is_err()
    );
    assert_eq!(
        g.explain_constraint_default(&c, |v| vec![*v]).unwrap(),
        snapshot
    );
}

#[test]
fn different_tuple_id_types_aggregate_without_losing_original_identities() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<u32, u32>();
    let pairs = b
        .join_equal(source.port(), source.port(), |_| 0, |_| 0)
        .unwrap();
    let unary = b
        .score("unary", source.port(), |_| SimpleScore::new(-1))
        .unwrap();
    let binary = b.score("binary", &pairs, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(50, 7)).unwrap();
    tx.apply(&source, TupleChange::put(10, 9)).unwrap();
    tx.commit().unwrap();
    let (unary_ids, unary) = g
        .explain_constraint_default(&unary, |v| vec![*v])
        .unwrap()
        .into_indexed();
    let (binary_ids, binary) = g
        .explain_constraint_default(&binary, |(a, b)| vec![*a, *b])
        .unwrap()
        .into_indexed();
    assert_eq!(unary_ids, [10, 50]);
    assert_eq!(binary_ids, [(10, 10), (10, 50), (50, 10), (50, 50)]);
    assert_eq!(
        binary.indictments[&7]
            .matches
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let mut all = ScoreExplanation::default();
    all.add_constraint(unary).unwrap();
    all.add_constraint(binary).unwrap();
    assert_eq!(all.score, SimpleScore::new(-6));
    assert_eq!(all.score, g.score());
    assert_eq!(all.indictments[&7].score, SimpleScore::new(-4));
    assert_eq!(all.indictments[&9].score, SimpleScore::new(-4));
}
