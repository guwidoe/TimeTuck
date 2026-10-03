// Adapted from Timefold Community EqualsAndComparisonIndexerTest; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{JoinComparison, PartitionedJoinNode, TupleChange};
type Person = (&'static str, char, i32);
type Join = PartitionedJoinNode<'static, &'static str, usize, Person, (char, i32), char, i32>;
fn index() -> Join {
    PartitionedJoinNode::new(
        JoinComparison::LessThanOrEqual,
        |r: &Person| (r.1, r.2),
        |r: &(char, i32)| *r,
    )
}
fn put(join: &mut Join, row: Person) {
    join.apply_left(TupleChange::put(row.0, row));
}
fn query(join: &mut Join, gender: char, age: i32) {
    join.apply_right(TupleChange::put(0, (gender, age)));
}
fn names(join: &Join) -> Vec<&'static str> {
    let mut out = join.matches().map(|(_, (r, _))| r.0).collect::<Vec<_>>();
    out.sort();
    out
}
#[test]
fn i_empty() {
    let mut j = index();
    query(&mut j, 'F', 40);
    assert!(names(&j).is_empty());
}
#[test]
fn put_case() {
    let mut j = index();
    query(&mut j, 'F', 40);
    assert_eq!(j.matches().count(), 0);
    put(&mut j, ("Ann-F-40", 'F', 40));
    assert_eq!(j.matches().count(), 1);
}
#[test]
fn for_each() {
    let mut j = index();
    for r in [
        ("Ann-F-40", 'F', 40),
        ("Beth-F-30", 'F', 30),
        ("Carl-M-40", 'M', 40),
        ("Dan-M-30", 'M', 30),
        ("Edna-F-40", 'F', 40),
    ] {
        put(&mut j, r);
    }
    for (age, expected) in [
        (40, vec!["Ann-F-40", "Beth-F-30", "Edna-F-40"]),
        (35, vec!["Beth-F-30"]),
        (30, vec!["Beth-F-30"]),
        (20, vec![]),
    ] {
        query(&mut j, 'F', age);
        assert_eq!(names(&j), expected);
    }
}
#[test]
fn iterator_empty() {
    let mut j = index();
    query(&mut j, 'F', 40);
    assert!(j.matches().peekable().peek().is_none());
}
#[test]
fn iterator_single_element() {
    let mut j = index();
    put(&mut j, ("Ann-F-40", 'F', 40));
    query(&mut j, 'F', 40);
    let mut it = j.matches().map(|(_, (r, _))| *r).peekable();
    assert!(it.peek().is_some());
    assert_eq!(it.next(), Some(("Ann-F-40", 'F', 40)));
    assert!(it.peek().is_none());
}
#[test]
fn iterator_multiple_elements_same_key() {
    let mut j = index();
    put(&mut j, ("Ann-F-40", 'F', 40));
    put(&mut j, ("Beth-F-40", 'F', 40));
    query(&mut j, 'F', 40);
    assert_eq!(names(&j), vec!["Ann-F-40", "Beth-F-40"]);
}
#[test]
fn iterator_comparison_boundary() {
    let mut j = index();
    for r in [
        ("Ann-F-40", 'F', 40),
        ("Beth-F-30", 'F', 30),
        ("Carol-F-50", 'F', 50),
    ] {
        put(&mut j, r);
    }
    query(&mut j, 'F', 40);
    assert_eq!(names(&j), vec!["Ann-F-40", "Beth-F-30"]);
}
#[test]
fn iterator_comparison_exact_boundary() {
    let mut j = index();
    put(&mut j, ("Ann-F-40", 'F', 40));
    put(&mut j, ("Beth-F-30", 'F', 30));
    query(&mut j, 'F', 30);
    assert_eq!(names(&j), vec!["Beth-F-30"]);
}
#[test]
fn iterator_comparison_none_match() {
    let mut j = index();
    put(&mut j, ("Ann-F-40", 'F', 40));
    put(&mut j, ("Beth-F-50", 'F', 50));
    query(&mut j, 'F', 20);
    assert!(j.matches().peekable().peek().is_none());
}
#[test]
fn iterator_different_gender() {
    let mut j = index();
    put(&mut j, ("Ann-F-40", 'F', 40));
    put(&mut j, ("Bob-M-30", 'M', 30));
    query(&mut j, 'M', 40);
    let found = names(&j);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0], "Bob-M-30");
}
