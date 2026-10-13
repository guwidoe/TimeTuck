use std::collections::BTreeSet;
use timetuck::*;
#[test]
fn real_graph_payload_occurrences_import_update_and_remove_exact_totals() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, i64>();
    let constraint = builder
        .score("cost", source.port(), |value| SimpleScore::new(-value))
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.apply(&source, TupleChange::put(1, 3)).unwrap();
    tx.commit().unwrap();
    let report = graph
        .explain_constraint_default(&constraint, |_| vec!["a", "a", "b"])
        .unwrap();
    let mut original = ScoreExplanation::default();
    original.add_constraint(report).unwrap();
    let (mut ledger, bindings) = ConstraintOccurrenceLedger::from_explanation(&original).unwrap();
    assert_eq!(ledger.explanation().score, graph.score());
    assert_eq!(bindings.len(), 2);
    let first = ledger.lookup(&bindings[0].occurrence).unwrap().clone();
    let second = ledger.lookup(&bindings[1].occurrence).unwrap().clone();
    assert_eq!(first.payload(), second.payload());
    assert_ne!(first, second);
    assert_eq!(first.diagnostic_cmp(&second), std::cmp::Ordering::Equal);
    assert_eq!(ledger.total("cost").unwrap().explanation().matches.len(), 2);
    assert_eq!(ledger.explanation().indictments["a"].matches.len(), 2);
    assert_eq!(
        ledger.explanation().indictments["a"].score,
        SimpleScore::new(-6)
    );
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 10)).unwrap();
    tx.commit().unwrap();
    let current = graph
        .explain_constraint_default(&constraint, |_| vec!["b", "c"])
        .unwrap();
    let replacement = current.matches[&0].clone();
    let old = ledger.replace(first.id(), replacement).unwrap().unwrap();
    assert_eq!(old, first);
    let updated = ledger.lookup(first.id()).unwrap();
    assert_eq!(*updated, first);
    assert_eq!(updated.payload().score, SimpleScore::new(-10));
    assert_eq!(first.payload().score, SimpleScore::new(-3));
    assert_eq!(ledger.explanation().score, SimpleScore::new(-13));
    assert_eq!(
        ledger.explanation().indictments["a"].score,
        SimpleScore::new(-3)
    );
    assert_eq!(
        ledger.explanation().indictments["b"].score,
        SimpleScore::new(-13)
    );
    assert_eq!(
        ledger.explanation().indictments["c"].score,
        SimpleScore::new(-10)
    );
    ledger.remove(first.id()).unwrap();
    assert!(ledger.lookup(first.id()).is_none());
    assert!(ledger.remove(first.id()).unwrap().is_none());
    assert_eq!(ledger.explanation().score, SimpleScore::new(-3));
    assert!(!ledger.explanation().indictments.contains_key("c"));
    ledger.remove(second.id()).unwrap();
    assert_eq!(ledger.explanation().score, SimpleScore::ZERO);
    assert!(ledger.explanation().indictments.is_empty());
    assert!(
        ledger
            .total("cost")
            .unwrap()
            .explanation()
            .matches
            .is_empty()
    );
    let readded = ledger.add("cost", first.payload().clone()).unwrap();
    assert_ne!(readded, first);
    assert!(ledger.lookup(first.id()).is_none());
}
#[test]
fn checked_overflow_foreign_handles_and_equal_payload_hash_sets_preserve_publication() {
    let payload = |score| ExplainedMatch {
        score: SimpleScore::new(score),
        justification: DefaultJustification {
            score: SimpleScore::new(score),
            facts: vec!["a"],
        },
        indicted_facts: BTreeSet::from(["a"]),
    };
    let mut ledger = ConstraintOccurrenceLedger::default();
    let a = ledger.add("cost", payload(i64::MAX)).unwrap();
    let before = ledger.explanation().clone();
    assert!(ledger.add("cost", payload(1)).is_err());
    assert_eq!(ledger.explanation(), &before);
    let b = ledger.add("cost", payload(0)).unwrap();
    let before = ledger.explanation().clone();
    assert!(ledger.replace(b.id(), payload(1)).is_err());
    assert_eq!(ledger.explanation(), &before);
    assert_eq!(
        ledger.lookup(b.id()).unwrap().payload().score,
        SimpleScore::ZERO
    );
    let mut other = ConstraintOccurrenceLedger::default();
    let foreign = other.add("cost", payload(i64::MAX)).unwrap();
    assert_ne!(foreign, a);
    assert!(ledger.remove(foreign.id()).unwrap().is_none());
    assert!(ledger.replace(foreign.id(), payload(0)).unwrap().is_none());
    let mut equal = ConstraintOccurrenceLedger::default();
    let x = equal.add("zero", payload(0)).unwrap();
    let y = equal.add("zero", payload(0)).unwrap();
    let set = std::collections::HashSet::from([x.clone(), x.clone(), y.clone()]);
    assert_eq!(set.len(), 2);
    assert_eq!(equal.explanation().constraints["zero"].matches.len(), 2);
}

#[test]
fn clone_panics_before_add_publication_leave_no_orphaned_occurrence() {
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Fragile(Rc<Cell<usize>>);
    impl Clone for Fragile {
        fn clone(&self) -> Self {
            let remaining = self.0.get();
            assert!(remaining > 0, "requested clone fault");
            self.0.set(remaining - 1);
            Self(self.0.clone())
        }
    }
    for budget in [0, 1] {
        let mut ledger = ConstraintOccurrenceLedger::<SimpleScore, Fragile, &str>::default();
        ledger.ensure_total("cost").unwrap();
        let remaining = Rc::new(Cell::new(budget));
        let row = ExplainedMatch {
            score: SimpleScore::new(-3),
            justification: Fragile(remaining),
            indicted_facts: BTreeSet::from(["a"]),
        };
        let failure =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ledger.add("cost", row)));
        assert!(failure.is_err());
        assert_eq!(ledger.explanation().score, SimpleScore::ZERO);
        assert!(ledger.explanation().constraints["cost"].matches.is_empty());
        assert!(ledger.explanation().indictments.is_empty());
    }
}

#[test]
fn import_batches_large_actual_reports_without_rebuilding_prior_match_payloads() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct CloneCount(Arc<AtomicUsize>);
    impl Clone for CloneCount {
        fn clone(&self) -> Self {
            self.0.fetch_add(1, Ordering::SeqCst);
            Self(self.0.clone())
        }
    }
    const COUNT: usize = 256;
    let scored = Arc::new(AtomicUsize::new(0));
    let calls = scored.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let cost = b
        .score("cost", source.port(), move |value| {
            calls.fetch_add(1, Ordering::SeqCst);
            SimpleScore::new(-value)
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for tuple in 0..COUNT {
        tx.apply(&source, TupleChange::put(tuple, 1)).unwrap();
    }
    tx.commit().unwrap();
    let clones = Arc::new(AtomicUsize::new(0));
    let report = graph
        .explain_constraint(&cost, |_, _| (CloneCount(clones.clone()), vec!["all"]))
        .unwrap();
    let mut actual = ScoreExplanation::default();
    actual.add_constraint(report).unwrap();
    let scores_before = scored.load(Ordering::SeqCst);
    let (ledger, bindings) = ConstraintOccurrenceLedger::from_explanation(&actual).unwrap();
    assert_eq!(bindings.len(), COUNT);
    assert_eq!(
        ledger.explanation().score,
        SimpleScore::new(-(COUNT as i64))
    );
    assert_eq!(
        ledger.total("cost").unwrap().explanation().matches.len(),
        COUNT
    );
    assert_eq!(ledger.explanation().indictments["all"].matches.len(), COUNT);
    assert_eq!(
        clones.load(Ordering::SeqCst),
        2 * COUNT,
        "only owned report rows and retained handles need a payload clone"
    );
    assert_eq!(
        scored.load(Ordering::SeqCst),
        scores_before,
        "import must not rescore graph tuples"
    );
    for binding in &bindings {
        assert_eq!(binding.constraint, "cost");
        assert_eq!(
            ledger.lookup(&binding.occurrence).unwrap().payload().score,
            actual.constraints["cost"].matches[&binding.tuple].score
        );
    }
}
