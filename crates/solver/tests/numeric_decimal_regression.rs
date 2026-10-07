//! Original integration and key-law checks; no upstream method credit.
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};
use timetuck::*;

fn n(s: &str) -> NumericDecimal {
    s.parse().unwrap()
}
fn hash(v: &NumericDecimal) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

#[test]
fn representations_round_trip_but_keys_share_numeric_identity() {
    let families = [
        vec!["0", "-0.00", "0E+2147483648", "0E-2147483647"],
        vec!["1", "1.0", "1.00", "100E-2"],
        vec!["-12", "-12.00", "-120E-1"],
        vec!["100", "1E+2", "10E+1", "100.0"],
    ];
    let mut all = vec![];
    for family in families {
        let values: Vec<_> = family.iter().map(|s| n(s)).collect();
        for (v, text) in values.iter().zip(&family) {
            assert_eq!(v, &values[0]);
            assert_eq!(hash(v), hash(&values[0]));
            let original: Decimal = text.parse().unwrap();
            assert_eq!(v.as_decimal(), &original);
            assert_eq!(v.clone().into_decimal(), original);
            assert_eq!(
                v.to_string()
                    .parse::<NumericDecimal>()
                    .unwrap()
                    .as_decimal(),
                &original
            );
        }
        all.extend(values);
    }
    assert_eq!(all.iter().cloned().collect::<HashSet<_>>().len(), 4);
    assert_eq!(all.iter().cloned().collect::<BTreeSet<_>>().len(), 4);
    assert_ne!(n("1.0").as_decimal(), n("1.00").as_decimal());
    assert!("NaN".parse::<NumericDecimal>().is_err());
    assert!("1E+2147483649".parse::<NumericDecimal>().is_err());
    for a in &all {
        for b in &all {
            assert_eq!(a == b, a.cmp(b).is_eq());
            assert_eq!(a.cmp(b), b.cmp(a).reverse());
        }
    }
}

#[test]
fn extreme_scales_and_trailing_zeros_do_not_require_expanding_exponents() {
    for sign in [-1, 1] {
        // Canonical numeric scale falls below i32::MIN; hashing must not wrap.
        let a = NumericDecimal::new(Decimal::new((sign * 10).into(), i32::MIN));
        let b = NumericDecimal::new(Decimal::new((sign * 100).into(), i32::MIN + 1));
        assert_eq!(a, b);
        assert_eq!(hash(&a), hash(&b));
        let tiny = NumericDecimal::new(Decimal::new(sign.into(), i32::MAX));
        assert_eq!(tiny.cmp(&a), 0.cmp(&sign));
        assert_eq!(tiny.is_negative(), sign < 0);
        assert!(!tiny.is_zero());
    }
    let mut ordered = BTreeSet::new();
    let mut hashed = HashSet::new();
    for integer in -50..=50_i64 {
        for scale in 0..=6_u32 {
            let v = NumericDecimal::new(Decimal::new(
                (integer * 10_i64.pow(scale)).into(),
                scale as i32,
            ));
            assert_eq!(v, NumericDecimal::from(integer));
            ordered.insert(v.clone());
            hashed.insert(v);
        }
    }
    assert_eq!(hashed.len(), 101);
    assert_eq!(
        ordered.into_iter().collect::<Vec<_>>(),
        (-50..=50).map(NumericDecimal::from).collect::<Vec<_>>()
    );
}

#[test]
fn candidate_buckets_and_ranges_follow_numeric_keys_after_replacement() {
    let mut index = CandidateIndex::new();
    for (id, text) in ["1", "1.00", "2.0", "10", "-1E+2"].into_iter().enumerate() {
        index.insert(id, n(text), id).unwrap();
    }
    assert_eq!(index.key_count(), 4);
    assert_eq!(index.bucket(&n("1.000")).len(), 2);
    assert_eq!(index.range(n("1.0")..=n("2.00")).len(), 3);
    index.replace(&1, n("1.0000"), 9).unwrap();
    assert_eq!(index.key_count(), 4);
    assert_eq!(index.bucket(&n("1")).len(), 2);
    index.replace(&1, n("2"), 10).unwrap();
    index.remove(&0).unwrap();
    assert!(index.bucket(&n("1.0")).is_empty());
    assert_eq!(index.key_count(), 3);
    assert_eq!(index.bucket(&n("2.000")).len(), 2);
}

#[test]
fn incremental_joins_and_groups_match_integer_oracle_through_rollback() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let left = builder.source::<usize, Decimal>();
    let right = builder.source::<usize, Decimal>();
    let joined = builder
        .join_equal(
            left.port(),
            right.port(),
            |x| NumericDecimal::new(x.clone()),
            |x| NumericDecimal::new(x.clone()),
        )
        .unwrap();
    builder
        .score("pairs", &joined, |_| SimpleScore::new(1))
        .unwrap();
    let grouped = builder
        .group_collect(left.port(), |x| (NumericDecimal::new(x.clone()), 1))
        .unwrap();
    builder
        .score("groups", &grouped, |(_, summary)| {
            SimpleScore::new(summary.sum * summary.sum)
        })
        .unwrap();
    let mut graph = builder.build();
    let mut rows = [BTreeMap::<usize, i64>::new(), BTreeMap::new()];
    let score = |rows: &[BTreeMap<usize, i64>; 2]| {
        let pairs = rows[0]
            .values()
            .flat_map(|a| rows[1].values().map(move |b| i64::from(a == b)))
            .sum::<i64>();
        let mut counts = BTreeMap::<i64, i64>::new();
        for v in rows[0].values() {
            *counts.entry(*v).or_default() += 1;
        }
        SimpleScore::new(pairs + counts.values().map(|n| n * n).sum::<i64>())
    };
    let mut seed = 78_u64;
    for step in 0..400 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 12;
        let side = step % 2;
        let integer = ((seed >> 40) % 7) as i64 - 3;
        let scale = ((seed >> 48) % 5) as u32;
        let value = (step % 7 != 0)
            .then(|| Decimal::new((integer * 10_i64.pow(scale)).into(), scale as i32));
        let mut candidate = rows.clone();
        if value.is_some() {
            candidate[side].insert(id, integer);
        } else {
            candidate[side].remove(&id);
        }
        let mut tx = graph.transaction();
        tx.apply(
            if side == 0 { &left } else { &right },
            TupleChange { id, value },
        )
        .unwrap();
        assert_eq!(tx.score().unwrap(), score(&candidate), "step {step}");
        if step % 3 == 0 {
            drop(tx);
        } else {
            tx.commit().unwrap();
            rows = candidate;
        }
        assert_eq!(graph.score(), score(&rows));
    }
    graph.assert_consistent().unwrap();
}
