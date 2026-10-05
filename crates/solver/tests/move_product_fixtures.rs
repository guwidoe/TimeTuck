// Partial enumeration fixtures adapted from CartesianProductMoveSelectorTest.
// Apache-2.0; compat/provenance.json records helper provenance. These omit Java
// selector lifecycle assertions and earn no upstream-method coverage credit.
use timetuck::MoveProduct;
fn codes(children: &[&[&str]], ignore_empty: bool) -> Vec<String> {
    MoveProduct::new(children.len(), ignore_empty, |i| {
        children[i].iter().copied()
    })
    .map(|parts| parts.join("+"))
    .collect()
}
#[test]
fn ordered_products_and_empty_children_match_upstream_fixtures() {
    for ignore in [false, true] {
        assert_eq!(
            codes(&[&["a1", "a2", "a3"], &["b1", "b2"]], ignore),
            ["a1+b1", "a1+b2", "a2+b1", "a2+b2", "a3+b1", "a3+b2"]
        );
        assert_eq!(
            codes(&[&["a1", "a2"], &["b1", "b2"], &["c1", "c2"]], ignore),
            [
                "a1+b1+c1", "a1+b1+c2", "a1+b2+c1", "a1+b2+c2", "a2+b1+c1", "a2+b1+c2", "a2+b2+c1",
                "a2+b2+c2"
            ]
        );
        for children in [
            vec![&[][..], &["a1", "a2", "a3"][..]],
            vec![&["a1", "a2", "a3"][..], &[][..]],
        ] {
            assert_eq!(
                codes(&children, ignore),
                if ignore {
                    vec!["a1", "a2", "a3"]
                } else {
                    vec![]
                }
            );
        }
        assert!(codes(&[&[], &[]], ignore).is_empty());
        assert_eq!(
            codes(&[&["a1", "a2"], &[], &["c1", "c2"]], ignore),
            if ignore {
                vec!["a1+c1", "a1+c2", "a2+c1", "a2+c2"]
            } else {
                vec![]
            }
        );
    }
}
