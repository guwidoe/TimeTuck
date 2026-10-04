//! Original composition, atomic delegation and graph/search regressions.
use std::{
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Identity;
impl<T: Clone> CollectorFinisher<T> for Identity {
    type Output = T;
    fn finish(v: T) -> T {
        v
    }
}
#[derive(Clone, Default)]
struct Fragile(i64);
impl ReversibleCollector<i64> for Fragile {
    type Output = i64;
    fn insert(&mut self, v: &i64) {
        self.0 += v;
        assert_ne!(*v, 13);
    }
    fn remove(&mut self, v: &i64) -> bool {
        self.0 -= v;
        *v != 99
    }
    fn replace(&mut self, old: &i64, new: &i64) -> bool {
        self.0 += 100 + new - old;
        assert_ne!(*new, 13);
        *old != 99
    }
    fn finish(&self) -> i64 {
        self.0
    }
}
#[test]
fn wrapper_isolates_failed_inner_mutations_and_preserves_specialized_replace() {
    let mut c = FinishingCollector::<Fragile, Identity>::default();
    c.insert(&2);
    let snapshot = c.clone();
    assert!(catch_unwind(AssertUnwindSafe(|| c.insert(&13))).is_err());
    assert_eq!(c.finish(), 2);
    assert!(!c.remove(&99));
    assert_eq!(c.finish(), 2);
    assert!(!c.replace(&99, &5));
    assert_eq!(c.finish(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| c.replace(&2, &13))).is_err());
    assert_eq!(c.finish(), 2);
    assert!(c.replace(&2, &5));
    assert_eq!(c.finish(), 105);
    assert_eq!(snapshot.finish(), 2);
    assert_eq!(c.into_inner().0, 105);
}
struct PeriodFields;
impl CollectorFinisher<CalendarPeriod> for PeriodFields {
    type Output = [i32; 3];
    fn finish(v: CalendarPeriod) -> Self::Output {
        v.components()
    }
}
#[test]
fn checked_same_value_replacement_rejection_is_not_optimized_away() {
    let mut c = FinishingCollector::<PeriodSumCollector, PeriodFields>::default();
    for v in [-1, i32::MAX, 1] {
        c.insert(&CalendarPeriod::from_days(v));
    }
    assert!(
        catch_unwind(AssertUnwindSafe(|| c.replace(
            &CalendarPeriod::from_days(-1),
            &CalendarPeriod::from_days(-1)
        )))
        .is_err()
    );
    assert_eq!(c.finish(), [0, 0, i32::MAX]);
    c.remove(&CalendarPeriod::from_days(1));
    assert_eq!(c.finish(), [0, 0, i32::MAX - 1]);
}
struct Summary;
#[derive(Clone, Debug, PartialEq)]
struct Observation {
    count: usize,
    min: Option<i64>,
    max: Option<i64>,
    sum: i64,
    average: Option<f64>,
}
impl CollectorFinisher<(IntSummary, Option<f64>)> for Summary {
    type Output = Observation;
    fn finish((s, a): (IntSummary, Option<f64>)) -> Observation {
        Observation {
            count: s.count,
            min: s.min,
            max: s.max,
            sum: s.sum,
            average: a,
        }
    }
}
type C = FinishingCollector<PairCollector<IntCollector, AverageCollector>, Summary>;
fn independent(v: &[i64]) -> Observation {
    Observation {
        count: v.len(),
        min: v.iter().min().copied(),
        max: v.iter().max().copied(),
        sum: v.iter().sum(),
        average: (!v.is_empty()).then(|| v.iter().sum::<i64>() as f64 / v.len() as f64),
    }
}
fn score(v: &Observation) -> SimpleScore {
    SimpleScore::new(v.sum - v.count as i64 * (v.max.unwrap_or(0) - v.min.unwrap_or(0)))
}
#[test]
fn nested_finishing_composition_matches_full_group_rebuild_through_undo() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (u8, i64)>();
    let p = b
        .group_collect_with::<_, _, _, _, C>(s.port(), |r| (r.0, (r.1, r.1)))
        .unwrap();
    let h = b.score("summary", &p, |(_, v)| score(v)).unwrap();
    let mut g = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 20261226u64;
    for step in 0..600 {
        let saved = g.constraint_matches(&h).unwrap().clone();
        let mut next = rows.clone();
        let mut t = g.transaction();
        for _ in 0..3 {
            let mut draw = || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                seed >> 32
            };
            let id = (draw() % 24) as usize;
            let value = (!draw().is_multiple_of(4))
                .then(|| ((draw() % 4) as u8, (draw() % 99) as i64 - 49));
            if let Some(v) = value {
                next.insert(id, v);
            } else {
                next.remove(&id);
            }
            t.apply(&s, TupleChange { id, value }).unwrap();
            let mut groups = BTreeMap::<u8, Vec<i64>>::new();
            for &(k, v) in next.values() {
                groups.entry(k).or_default().push(v);
            }
            let actual = t.graph().unwrap().constraint_matches(&h).unwrap();
            assert_eq!(actual.len(), groups.len());
            let mut total = SimpleScore::ZERO;
            for (k, v) in groups {
                let expected = independent(&v);
                assert_eq!(actual[&k].0.1, expected);
                total = total + score(&expected);
            }
            assert_eq!(t.score().unwrap(), total);
        }
        if step % 17 == 0 {
            t.assert_consistent().unwrap();
        }
        if step % 3 == 0 {
            drop(t);
            assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
        } else {
            t.commit().unwrap();
            rows = next;
        }
    }
    g.assert_consistent().unwrap();
}
struct RejectThirteen;
impl CollectorFinisher<IntSummary> for RejectThirteen {
    type Output = i64;
    fn finish(v: IntSummary) -> i64 {
        assert_ne!(v.sum, 13);
        v.sum
    }
}
#[test]
fn finisher_panic_leaves_standalone_state_and_graph_checkpoint_intact() {
    type Reject = FinishingCollector<IntCollector, RejectThirteen>;
    let mut c = Reject::default();
    c.insert(&13);
    assert!(catch_unwind(AssertUnwindSafe(|| c.finish())).is_err());
    assert!(c.remove(&13));
    assert_eq!(c.finish(), 0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let p = b.collect::<_, _, _, Reject>(s.port(), |v| *v).unwrap();
    let h = b.score("reject", &p, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 2)).unwrap();
    t.commit().unwrap();
    let saved = g.constraint_matches(&h).unwrap().clone();
    let mut t = g.transaction();
    assert!(catch_unwind(AssertUnwindSafe(|| t.apply(&s, TupleChange::put(1, 11)))).is_err());
    assert!(t.score().is_err());
    drop(t);
    assert_eq!(g.constraint_matches(&h).unwrap(), &saved);
    g.assert_consistent().unwrap();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(1, 7)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(9));
}
#[test]
fn transformed_compositions_drive_construction_and_saved_candidate_search() {
    let p = FiniteProblem::new(vec![Variable::new(vec![-2i64, 1, 3]); 4]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(4);
    n.add_constraint("summary", (0..4).collect(), |v| {
        score(&independent(
            &v.iter().filter_map(|v| v.copied()).collect::<Vec<_>>(),
        ))
    })
    .unwrap();
    let factory = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        let c = b
            .collect_conditionally::<_, _, _, C>(
                s.port(),
                |r| r.1.is_some(),
                |r| (r.1.unwrap(), r.1.unwrap()),
            )
            .unwrap();
        b.score("summary", &c, score).unwrap();
        (b.build(), s)
    };
    let initial = Assignment::new(vec![Some(-2); 4]);
    for cap in 1..=32 {
        let cfg = LocalSearchConfig {
            max_evaluations: cap,
            max_steps: 8,
            assert_scores: true,
            ..Default::default()
        };
        let expected = local_search_from(&p, &n, initial.clone(), cfg).unwrap();
        let (g, s) = factory();
        let actual = stream_local_search_from(&p, g, s, initial.clone(), cfg).unwrap();
        assert_eq!(actual.best, expected.best);
        assert_eq!(actual.evaluations, expected.evaluations);
        assert_eq!(actual.termination, expected.termination);
    }
    let cfg = ConstructionConfig {
        assert_scores: true,
        ..Default::default()
    };
    let (g, s) = factory();
    let actual = greedy_stream_construction(&p, g, s, cfg).unwrap();
    let expected = greedy_construction(&p, &n, cfg).unwrap();
    assert_eq!(actual.assignment, expected.assignment);
    assert_eq!(actual.score, expected.score);
}
