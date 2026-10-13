// Adapted from pinned Community ConstraintMatchTest and ConstraintMatchTotalTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    cmp::Ordering,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
type Ledger = ConstraintOccurrenceLedger<
    SimpleScore,
    DefaultJustification<SimpleScore, &'static str>,
    &'static str,
>;
type Match = ConstraintOccurrence<
    SimpleScore,
    DefaultJustification<SimpleScore, &'static str>,
    &'static str,
>;
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
fn add(ledger: &mut Ledger, name: &str, score: i64, facts: &[&'static str]) -> Match {
    let score = SimpleScore::new(score);
    ledger
        .add(
            name,
            ExplainedMatch {
                score,
                justification: DefaultJustification {
                    score,
                    facts: facts.to_vec(),
                },
                indicted_facts: facts.iter().copied().collect(),
            },
        )
        .unwrap()
}
#[test]
fn match_equals_and_hash_code() {
    let mut ledger = Ledger::default();
    let first = add(&mut ledger, "c", 0, &["e1"]);
    let same = first.clone();
    assert_eq!(first, same);
    assert_eq!(hash(&first), hash(&same));
    assert_eq!(first.diagnostic_cmp(&same), Ordering::Equal);
    let second = add(&mut ledger, "c", 0, &["e1"]);
    assert_ne!(first, second);
}
#[test]
fn match_compare_to() {
    let mut ledger = Ledger::default();
    let rows = [
        ("a", 0, vec!["a"]),
        ("a", 0, vec!["a", "aa"]),
        ("a", 0, vec!["a", "ab"]),
        ("a", 0, vec!["a", "c"]),
        ("a", 0, vec!["a", "aa", "a"]),
        ("a", 0, vec!["a", "aa", "b"]),
        ("a", 1, vec!["a", "aa"]),
        ("b", 0, vec!["a", "aa"]),
        ("b", 0, vec!["a", "ab"]),
        ("b", 0, vec!["a", "c"]),
    ];
    let matches = rows
        .iter()
        .map(|(name, score, facts)| add(&mut ledger, name, *score, facts))
        .collect::<Vec<_>>();
    for (i, a) in matches.iter().enumerate() {
        for b in &matches[i + 1..] {
            assert_eq!(a.diagnostic_cmp(b), Ordering::Less);
            assert_eq!(b.diagnostic_cmp(a), Ordering::Greater);
        }
    }
}
#[test]
fn total_equals_and_hash_code() {
    let mut a = Ledger::default();
    a.ensure_total("c").unwrap();
    let mut b = Ledger::default();
    b.ensure_total("c").unwrap();
    let mut c = Ledger::default();
    add(&mut c, "c", -7, &[]);
    let totals = [
        a.total("c").unwrap(),
        b.total("c").unwrap(),
        c.total("c").unwrap(),
    ];
    for (i, left) in totals.iter().enumerate() {
        for right in &totals[i + 1..] {
            assert_eq!(left, right);
            assert_eq!(hash(left), hash(right));
            assert_eq!(left.cmp(right), Ordering::Equal);
        }
    }
    let mut d = Ledger::default();
    d.ensure_total("d").unwrap();
    let other = d.total("d").unwrap();
    assert_ne!(totals[0], other);
    assert_ne!(totals[0].cmp(&other), Ordering::Equal);
}
#[test]
fn total_compare_to() {
    let mut ledger = Ledger::default();
    let names = ["a", "aa", "ab", "b", "c", "ca", "cb", "d"];
    for name in names {
        ledger.ensure_total(name).unwrap();
    }
    let totals = names.map(|name| ledger.total(name).unwrap());
    for (i, a) in totals.iter().enumerate() {
        for b in &totals[i + 1..] {
            assert_eq!(a.cmp(b), Ordering::Less);
            assert_eq!(b.cmp(a), Ordering::Greater);
        }
    }
}
