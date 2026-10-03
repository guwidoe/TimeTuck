// Adapted from Timefold Community collection indexer tests; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{MembershipJoinNode, TupleChange};
type Key = Option<&'static str>;
type Row = (&'static str, Vec<Key>, &'static str);
type Query = (Vec<Key>, &'static str);
type Join = MembershipJoinNode<'static, &'static str, usize, Row, Query, Key, &'static str>;
fn index() -> Join {
    MembershipJoinNode::partitioned(|r: &Row| (r.2, r.1.clone()), |q: &Query| (q.1, q.0.clone()))
}
fn keys(values: &[&'static str]) -> Vec<Key> {
    values.iter().copied().map(Some).collect()
}
fn put(j: &mut Join, name: &'static str, values: &[&'static str], dept: &'static str) {
    j.apply_left(TupleChange::put(name, (name, keys(values), dept)));
}
fn query(j: &mut Join, values: Vec<Key>, dept: &'static str) -> Vec<&'static str> {
    j.apply_right(TupleChange::put(0, (values, dept)));
    j.matches().map(|(_, (r, _))| r.0).collect()
}
fn count(j: &mut Join, values: &[&'static str], dept: &'static str, expected: usize) {
    assert_eq!(query(j, keys(values), dept).len(), expected);
}
#[test]
fn containing_size() {
    let mut j = index();
    count(&mut j, &["X"], "1", 0);
    put(&mut j, "Ann", &["X", "Y"], "1");
    for (k, d, n) in [("X", "1", 1), ("Y", "1", 1), ("Z", "1", 0), ("X", "999", 0)] {
        count(&mut j, &[k], d, n);
    }
    put(&mut j, "Beth", &["X", "Z"], "1");
    put(&mut j, "Carl", &["X", "Y"], "2");
    for (k, d, n) in [
        ("X", "1", 2),
        ("Y", "1", 1),
        ("Z", "1", 1),
        ("AAA", "1", 0),
        ("X", "999", 0),
    ] {
        count(&mut j, &[k], d, n);
    }
    put(&mut j, "Zero", &[], "1");
    assert!(query(&mut j, vec![None], "1").is_empty());
    count(&mut j, &["X"], "1", 2);
}
#[test]
fn containing_for_each() {
    let mut j = index();
    for (name, ks, d) in [
        ("Ann", vec!["X", "Y"], "1"),
        ("Beth", vec!["X", "Z"], "1"),
        ("Carl", vec!["X", "Y"], "2"),
        ("Edna", vec!["Y", "Z"], "1"),
        ("Zero", vec![], "1"),
    ] {
        put(&mut j, name, &ks, d);
    }
    for (k, d, names) in [
        ("X", "1", vec!["Ann", "Beth"]),
        ("Y", "1", vec!["Ann", "Edna"]),
        ("X", "2", vec!["Carl"]),
        ("Y", "2", vec!["Carl"]),
        ("Z", "1", vec!["Beth", "Edna"]),
        ("AAA", "1", vec![]),
        ("X", "999", vec![]),
    ] {
        assert_eq!(query(&mut j, keys(&[k]), d), names);
    }
    for d in ["1", "999"] {
        assert!(query(&mut j, vec![None], d).is_empty());
    }
}
fn reuse() {
    let mut j = index();
    put(&mut j, "Ann", &["X", "Y"], "1");
    j.apply_left(TupleChange::retract("Ann"));
    for k in ["X", "Y"] {
        count(&mut j, &[k], "1", 0);
    }
    assert_eq!(j.left_len(), 0);
    put(&mut j, "Beth", &["X", "Y"], "1");
    for k in ["X", "Y"] {
        count(&mut j, &[k], "1", 1);
    }
}
#[test]
fn containing_remove_last_tuple_then_reuse_keys() {
    reuse();
}
#[test]
fn any_of_remove_last_tuple_then_reuse_keys() {
    reuse();
}
#[test]
fn any_of_size() {
    let mut j = index();
    count(&mut j, &[], "1", 0);
    count(&mut j, &["X", "Y"], "1", 0);
    put(&mut j, "Ann", &["X", "Y"], "1");
    for (ks, d, n) in [
        (vec![], "1", 0),
        (vec!["X"], "1", 1),
        (vec!["X", "AAA"], "1", 1),
        (vec!["Y"], "1", 1),
        (vec!["X", "Y"], "1", 1),
        (vec!["AAA"], "1", 0),
        (vec!["X"], "999", 0),
    ] {
        count(&mut j, &ks, d, n);
    }
    put(&mut j, "Beth", &["X", "Z"], "1");
    put(&mut j, "Carl", &["X", "Y"], "2");
    for (ks, n) in [
        (vec![], 0),
        (vec!["X"], 2),
        (vec!["X", "AAA"], 2),
        (vec!["X", "Y"], 2),
        (vec!["X", "Y", "Z"], 2),
        (vec!["Y"], 1),
        (vec!["Z"], 1),
        (vec!["AAA"], 0),
    ] {
        count(&mut j, &ks, "1", n);
    }
    count(&mut j, &["X"], "999", 0);
    put(&mut j, "Zero", &[], "1");
    count(&mut j, &[], "1", 0);
    count(&mut j, &["X"], "1", 2);
}
#[test]
fn any_of_for_each() {
    let mut j = index();
    for (name, ks, d) in [
        ("Ann", vec!["X", "Y"], "1"),
        ("Beth", vec!["X", "Z"], "1"),
        ("Carl", vec!["X", "Y"], "2"),
        ("Zero", vec![], "1"),
    ] {
        put(&mut j, name, &ks, d);
    }
    for (ks, d, names) in [
        (vec!["X"], "1", vec!["Ann", "Beth"]),
        (vec!["X"], "2", vec!["Carl"]),
        (vec!["Y"], "2", vec!["Carl"]),
        (vec!["X", "AAA"], "1", vec!["Ann", "Beth"]),
        (vec!["Y"], "1", vec!["Ann"]),
        (vec!["Z"], "1", vec!["Beth"]),
        (vec!["X", "Y"], "1", vec!["Ann", "Beth"]),
        (vec!["X", "Z"], "1", vec!["Ann", "Beth"]),
        (vec!["X", "Y", "Z"], "1", vec!["Ann", "Beth"]),
        (vec!["AAA"], "1", vec![]),
        (vec!["X"], "999", vec![]),
        (vec![], "1", vec![]),
        (vec![], "999", vec![]),
    ] {
        assert_eq!(query(&mut j, keys(&ks), d), names);
    }
}
#[test]
fn contained_in_size() {
    let mut j = index();
    count(&mut j, &["X"], "1", 0);
    put(&mut j, "Ann", &["X"], "1");
    for (ks, d, n) in [
        (vec!["X"], "1", 1),
        (vec!["X", "Y"], "1", 1),
        (vec!["X", "AAA"], "1", 1),
        (vec!["Y"], "1", 0),
        (vec!["Y", "AAA"], "1", 0),
        (vec!["X"], "999", 0),
    ] {
        count(&mut j, &ks, d, n);
    }
    put(&mut j, "Beth", &["Y"], "1");
    put(&mut j, "Carl", &["X"], "2");
    for (ks, d, n) in [
        (vec!["X"], "1", 1),
        (vec!["X", "Y"], "1", 2),
        (vec!["X", "AAA"], "1", 1),
        (vec!["Y"], "1", 1),
        (vec!["AAA"], "1", 0),
        (vec!["X"], "999", 0),
    ] {
        count(&mut j, &ks, d, n);
    }
    j.apply_left(TupleChange::put("Zero", ("Zero", vec![None], "1")));
    count(&mut j, &[], "1", 0);
    assert_eq!(query(&mut j, vec![None], "1").len(), 1);
    count(&mut j, &["X"], "1", 1);
}
#[test]
fn contained_in_for_each() {
    let mut j = index();
    for (name, k, d) in [
        ("Ann", "X", "1"),
        ("Beth", "Y", "1"),
        ("Carl", "X", "2"),
        ("Edna", "X", "1"),
    ] {
        put(&mut j, name, &[k], d);
    }
    j.apply_left(TupleChange::put("Zero", ("Zero", vec![None], "1")));
    for (ks, d, names) in [
        (vec!["X"], "1", vec!["Ann", "Edna"]),
        (vec!["X"], "2", vec!["Carl"]),
        (vec!["X", "Y"], "1", vec!["Ann", "Beth", "Edna"]),
        (vec!["Y"], "1", vec!["Beth"]),
        (vec!["Y", "AAA"], "1", vec!["Beth"]),
        (vec!["AAA"], "1", vec![]),
        (vec!["X"], "999", vec![]),
        (vec![], "1", vec![]),
        (vec![], "999", vec![]),
    ] {
        assert_eq!(query(&mut j, keys(&ks), d), names);
    }
}
