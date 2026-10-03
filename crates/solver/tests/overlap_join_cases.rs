// Adapted from Timefold Community tests; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{BigInt, Decimal, OverlapJoinNode, TupleChange};
// Every literal assertion from each of the five source methods.
const CASES: [(i64, i64, i64, i64, bool); 13] = [
    (1, 5, 1, 5, true),
    (3, 7, 1, 5, true),
    (1, 5, 3, 7, true),
    (3, 5, 1, 7, true),
    (1, 7, 3, 5, true),
    (1, 7, 1, 3, true),
    (1, 3, 1, 7, true),
    (1, 7, 3, 7, true),
    (3, 7, 1, 7, true),
    (1, 3, 5, 7, false),
    (5, 7, 1, 3, false),
    (1, 3, 3, 7, false),
    (3, 7, 1, 3, false),
];
fn different(ignored: usize) {
    let mut join = OverlapJoinNode::new(
        |r: &(BigInt, BigInt, Vec<&str>)| {
            (i64::try_from(&r.0).unwrap(), i64::try_from(&r.1).unwrap())
        },
        |r: &(Decimal, Decimal)| {
            assert_eq!((r.0.scale(), r.1.scale()), (0, 0));
            (
                i64::try_from(r.0.coefficient()).unwrap(),
                i64::try_from(r.1.coefficient()).unwrap(),
            )
        },
    );
    for (a, b, c, d, yes) in CASES {
        let l = (
            BigInt::from(a),
            BigInt::from(b),
            vec!["Ignored Arg"; ignored],
        );
        let r = (
            Decimal::new(BigInt::from(c), 0),
            Decimal::new(BigInt::from(d), 0),
        );
        join.apply_left(TupleChange::put(0, l.clone()));
        join.apply_right(TupleChange::put(0, r.clone()));
        let actual = join
            .matches()
            .map(|((i, j), (a, b))| ((*i, *j), (a.clone(), b.clone())))
            .collect::<Vec<_>>();
        assert_eq!(actual, if yes { vec![((0, 0), (l, r))] } else { vec![] });
    }
}
#[test]
fn overlaps_bi_different_types() {
    different(0);
}
#[test]
fn overlaps_tri() {
    different(1);
}
#[test]
fn overlaps_quad() {
    different(2);
}
#[test]
fn overlaps_penta() {
    different(3);
}
#[test]
fn overlaps_bi_same_types() {
    let project =
        |r: &(BigInt, BigInt)| (i64::try_from(&r.0).unwrap(), i64::try_from(&r.1).unwrap());
    let mut join = OverlapJoinNode::new(project, project);
    for (a, b, c, d, yes) in CASES {
        let l = (BigInt::from(a), BigInt::from(b));
        let r = (BigInt::from(c), BigInt::from(d));
        join.apply_left(TupleChange::put(0, l.clone()));
        join.apply_right(TupleChange::put(0, r.clone()));
        assert_eq!(
            join.matches()
                .map(|((i, j), (a, b))| ((*i, *j), (a.clone(), b.clone())))
                .collect::<Vec<_>>(),
            if yes { vec![((0, 0), (l, r))] } else { vec![] }
        );
    }
}
