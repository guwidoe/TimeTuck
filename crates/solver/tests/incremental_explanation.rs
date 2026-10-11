//! Original incremental explanation regressions; no source-method credit.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn snapshot<F: Ord + Clone>(
    name: &str,
    rows: Vec<(usize, i64, Vec<F>)>,
) -> ConstraintExplanation<usize, SimpleScore, String, F> {
    let mut result = ConstraintExplanation {
        name: name.into(),
        score: SimpleScore::ZERO,
        matches: BTreeMap::new(),
        indictments: BTreeMap::new(),
    };
    for (id, value, facts) in rows {
        let score = SimpleScore::new(value);
        let indicted_facts: BTreeSet<_> = facts.into_iter().collect();
        for fact in &indicted_facts {
            let i = result
                .indictments
                .entry(fact.clone())
                .or_insert(Indictment {
                    score: SimpleScore::ZERO,
                    matches: BTreeSet::new(),
                });
            i.score = i.score + score;
            i.matches.insert(id);
        }
        result.score = result.score + score;
        result.matches.insert(
            id,
            ExplainedMatch {
                score,
                justification: format!("{name}/{id}"),
                indicted_facts,
            },
        );
    }
    result
}

#[test]
fn replacement_and_removal_match_independent_reaggregation() {
    let a = snapshot("a", vec![(0, -3, vec![1, 1, 2]), (1, 0, vec![1])]);
    let b = snapshot("b", vec![(0, -5, vec![1, 3]), (1, 0, vec![3])]);
    let mut actual = ScoreExplanation::default();
    assert_eq!(actual.replace_constraint(a.clone()).unwrap(), None);
    actual.replace_constraint(b.clone()).unwrap();
    assert_eq!(actual.indictments[&1].score, SimpleScore::new(-8));
    assert_eq!(actual.indictments[&1].matches.len(), 3);
    let new = snapshot("a", vec![(0, 7, vec![2, 4]), (2, 0, vec![1, 4])]);
    assert_eq!(actual.replace_constraint(new.clone()).unwrap(), Some(a));
    let mut fresh = ScoreExplanation::default();
    fresh.add_constraint(new.clone()).unwrap();
    fresh.add_constraint(b.clone()).unwrap();
    assert_eq!(actual, fresh);
    assert_eq!(actual.remove_constraint("unknown").unwrap(), None);
    assert_eq!(actual.remove_constraint("b").unwrap(), Some(b));
    let mut fresh = ScoreExplanation::default();
    fresh.add_constraint(new.clone()).unwrap();
    assert_eq!(actual, fresh);
    assert_eq!(actual.indictments[&1].score, SimpleScore::ZERO);
    assert_eq!(actual.indictments[&1].matches.len(), 1);
    assert_eq!(actual.remove_constraint("a").unwrap(), Some(new));
    assert_eq!(actual, ScoreExplanation::default());
}

#[test]
fn checked_final_totals_overflow_and_wrapping_are_atomic() {
    let mut report = ScoreExplanation::default();
    report
        .add_constraint(snapshot("a", vec![(0, i64::MAX, vec![1])]))
        .unwrap();
    let original = report.clone();
    assert_eq!(
        report.replace_constraint(snapshot("b", vec![(0, 1, vec![2])])),
        Err(ScoreOverflow)
    );
    assert_eq!(report, original);
    // Final zero fits despite subtraction from a negative current total first.
    report
        .add_constraint(snapshot("b", vec![(0, -i64::MAX, vec![1])]))
        .unwrap();
    report
        .add_constraint(snapshot("c", vec![(0, -i64::MAX, vec![1])]))
        .unwrap();
    report
        .replace_constraint(snapshot("a", vec![(0, i64::MAX, vec![1])]))
        .unwrap();
    assert_eq!(report.score, SimpleScore::new(-i64::MAX));
    // Aggregate fits but one shared fact's indictment overflows.
    let mut shared = ScoreExplanation::default();
    shared
        .add_constraint(snapshot(
            "a",
            vec![(0, i64::MAX, vec![1]), (1, -i64::MAX, vec![2])],
        ))
        .unwrap();
    let original = shared.clone();
    assert_eq!(
        shared.replace_constraint(snapshot("b", vec![(0, 1, vec![1]), (1, -1, vec![2])])),
        Err(ScoreOverflow)
    );
    assert_eq!(shared, original);
    shared
        .replace_constraint_with_arithmetic(
            snapshot("b", vec![(0, 1, vec![1]), (1, -1, vec![2])]),
            ScoringArithmetic::Wrapping,
        )
        .unwrap();
    assert_eq!(shared.indictments[&1].score, SimpleScore::new(i64::MIN));
    shared
        .remove_constraint_with_arithmetic("b", ScoringArithmetic::Wrapping)
        .unwrap();
    assert_eq!(shared, original);
    // Removing a cancelling contribution can overflow the remaining total.
    let mut removal = ScoreExplanation::default();
    for (name, score) in [("a", i64::MAX), ("b", 1), ("c", -1)] {
        removal
            .add_constraint(snapshot::<i32>(name, vec![(0, score, vec![])]))
            .unwrap();
    }
    let original = removal.clone();
    assert_eq!(removal.remove_constraint("c"), Err(ScoreOverflow));
    assert_eq!(removal, original);
}

#[derive(Debug, PartialEq, Eq)]
struct NotClone(String);
#[test]
fn graph_updates_and_rollback_keep_custom_nonclone_justifications() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (i32, i64)>();
    let cost = b
        .score("cost", source.port(), |(_, n)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(*n)
        })
        .unwrap();
    let zero = b
        .score("zero", source.port(), |_| SimpleScore::ZERO)
        .unwrap();
    let disabled = b
        .score_weighted("disabled", source.port(), SimpleScore::ZERO, |_| {
            panic!("disabled")
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (7, -3))).unwrap();
    tx.commit().unwrap();
    let explain = |tuple: &(i32, i64), _: &SimpleScore| {
        (
            NotClone(format!("fact {}", tuple.0)),
            vec![tuple.0, tuple.0],
        )
    };
    let mut report = ScoreExplanation::default();
    report
        .replace_constraint(graph.explain_constraint(&cost, explain).unwrap())
        .unwrap();
    report
        .replace_constraint(graph.explain_constraint(&zero, explain).unwrap())
        .unwrap();
    report
        .replace_constraint(graph.explain_constraint(&disabled, explain).unwrap())
        .unwrap();
    assert!(report.constraints["disabled"].matches.is_empty());
    assert_eq!(report.indictments[&7].matches.len(), 2);
    let before_calls = calls.get();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, (8, 0))).unwrap();
    }
    assert_eq!(graph.score(), SimpleScore::new(-3));
    let calls_after_trial = calls.get();
    assert!(calls_after_trial > before_calls);
    report
        .replace_constraint(graph.explain_constraint(&cost, explain).unwrap())
        .unwrap();
    assert_eq!(calls.get(), calls_after_trial);
    assert_eq!(
        report.constraints["cost"].matches[&0].justification,
        NotClone("fact 7".into())
    );
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (8, 0))).unwrap();
    tx.commit().unwrap();
    let calls_after_update = calls.get();
    report
        .replace_constraint(graph.explain_constraint(&cost, explain).unwrap())
        .unwrap();
    report
        .replace_constraint(graph.explain_constraint(&zero, explain).unwrap())
        .unwrap();
    assert_eq!(calls.get(), calls_after_update);
    assert_eq!(report.score, graph.score());
    assert!(!report.indictments.contains_key(&7));
    assert_eq!(report.indictments[&8].matches.len(), 2);
    assert_eq!(report.indictments[&8].score, SimpleScore::ZERO);
    graph.assert_consistent().unwrap();
}

#[derive(Debug)]
struct Fragile {
    id: i32,
    budget: Rc<Cell<Option<usize>>>,
    clone_budget: Rc<Cell<Option<usize>>>,
}
fn tick(budget: &Cell<Option<usize>>) {
    if let Some(n) = budget.get() {
        assert!(n > 0, "injected panic");
        budget.set(Some(n - 1));
    }
}
impl Clone for Fragile {
    fn clone(&self) -> Self {
        tick(&self.clone_budget);
        Self {
            id: self.id,
            budget: self.budget.clone(),
            clone_budget: self.clone_budget.clone(),
        }
    }
}
impl PartialEq for Fragile {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Fragile {}
impl PartialOrd for Fragile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Fragile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        tick(&self.budget);
        self.id.cmp(&other.id)
    }
}

#[test]
fn key_comparison_and_clone_panics_restore_insertions_and_partial_publication() {
    for clone in [false, true] {
        let budget = Rc::new(Cell::new(None));
        let clone_budget = Rc::new(Cell::new(None));
        let key = |id| Fragile {
            id,
            budget: budget.clone(),
            clone_budget: clone_budget.clone(),
        };
        let old = snapshot(
            "a",
            (0..24)
                .map(|i| (i as usize, -1, vec![key(i * 4)]))
                .collect(),
        );
        let new = snapshot(
            "a",
            (0..32).map(|i| (i as usize, 2, vec![key(i * 3)])).collect(),
        );
        let mut baseline = ScoreExplanation::default();
        baseline.add_constraint(old).unwrap();
        let mut successes = 0;
        // Every comparison/clone failure position, including permanent panics
        // throughout unwinding. Restoration itself must not compare/clone keys.
        let chosen = if clone { &clone_budget } else { &budget };
        let mut measured = baseline.clone();
        let replacement = new.clone();
        chosen.set(Some(usize::MAX));
        measured.replace_constraint(replacement).unwrap();
        let operation_count = usize::MAX - chosen.get().unwrap();
        chosen.set(None);
        for allowed in 0..=operation_count {
            let mut actual = baseline.clone();
            let replacement = new.clone();
            let chosen = if clone { &clone_budget } else { &budget };
            chosen.set(Some(allowed));
            let result = catch_unwind(AssertUnwindSafe(|| actual.replace_constraint(replacement)));
            chosen.set(None);
            match result {
                Err(_) => assert_eq!(actual, baseline, "budget {allowed}, clone {clone}"),
                Ok(result) => {
                    result.unwrap();
                    let mut expected = ScoreExplanation::default();
                    expected.add_constraint(new.clone()).unwrap();
                    assert_eq!(actual, expected);
                    successes += 1;
                    break;
                }
            }
        }
        assert_eq!(successes, 1, "sweep reached a complete operation");
    }
}

#[test]
fn deleting_many_fact_keys_preserves_other_constraints_across_tree_rebalancing() {
    let a = snapshot("a", (0..200).map(|i| (i as usize, 1, vec![i])).collect());
    let b = snapshot(
        "b",
        (0..200)
            .filter(|i| i % 7 == 0)
            .map(|i| (i as usize, -2, vec![i]))
            .collect(),
    );
    let mut report = ScoreExplanation::default();
    report.add_constraint(a).unwrap();
    report.add_constraint(b.clone()).unwrap();
    report.remove_constraint("a").unwrap();
    let mut expected = ScoreExplanation::default();
    expected.add_constraint(b).unwrap();
    assert_eq!(report, expected);
}

#[test]
fn existing_fact_replacement_does_not_clone_or_compare_unrelated_payloads() {
    let budget = Rc::new(Cell::new(None));
    let clone_budget = Rc::new(Cell::new(None));
    let key = |id| Fragile {
        id,
        budget: budget.clone(),
        clone_budget: clone_budget.clone(),
    };
    let mut report = ScoreExplanation::default();
    report
        .add_constraint(snapshot(
            "unrelated",
            (10..2010).map(|i| (i as usize, 1, vec![key(i)])).collect(),
        ))
        .unwrap();
    report
        .add_constraint(snapshot(
            "changed",
            (0..3).map(|i| (i as usize, 1, vec![key(i)])).collect(),
        ))
        .unwrap();
    let replacement = snapshot(
        "changed",
        (0..3).map(|i| (i as usize, 2, vec![key(i)])).collect(),
    );
    budget.set(Some(500));
    clone_budget.set(Some(3));
    report.replace_constraint(replacement).unwrap();
    assert_eq!(
        clone_budget.get(),
        Some(0),
        "clone only the three affected fact keys"
    );
    budget.set(None);
    clone_budget.set(None);
    assert_eq!(report.score, SimpleScore::new(2006));
    assert_eq!(report.constraints["unrelated"].matches.len(), 2000);
}

#[test]
fn normalized_join_ids_and_weighted_snapshots_replace_as_whole_constraints() {
    fn captured(
        weight: i64,
    ) -> ConstraintExplanation<usize, SimpleScore, DefaultJustification<SimpleScore, i32>, i32>
    {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i32>();
        let pairs = b
            .join_equal(source.port(), source.port(), |_| (), |_| ())
            .unwrap();
        let constraint = b
            .score_weighted("pairs", &pairs, SimpleScore::new(weight), |_| 1)
            .unwrap();
        let mut graph = b.build();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, 7)).unwrap();
        tx.apply(&source, TupleChange::put(1, 8)).unwrap();
        tx.commit().unwrap();
        graph
            .explain_constraint_default(&constraint, |(a, b)| vec![*a, *b])
            .unwrap()
            .into_indexed()
            .1
    }
    let mut explanation = ScoreExplanation::default();
    explanation.replace_constraint(captured(-2)).unwrap();
    assert_eq!(explanation.score, SimpleScore::new(-8));
    assert_eq!(explanation.indictments[&7].matches.len(), 3);
    explanation.replace_constraint(captured(3)).unwrap();
    let mut independent = ScoreExplanation::default();
    independent.add_constraint(captured(3)).unwrap();
    assert_eq!(explanation, independent);
    explanation.replace_constraint(captured(0)).unwrap();
    assert_eq!(explanation.score, SimpleScore::ZERO);
    assert!(explanation.indictments.is_empty());
    assert!(explanation.constraints["pairs"].matches.is_empty());
}
