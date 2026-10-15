//! Original bounded planning-group contracts; no upstream method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn problem(count: usize) -> FiniteProblem<i64> {
    FiniteProblem::new(
        (0..count)
            .map(|i| {
                let v = Variable::new(vec![1, 2, 3, 4]).allowing_unassigned();
                if i == 0 {
                    v.with_initial(1)
                } else if i + 1 == count {
                    v.with_initial(2)
                } else {
                    v
                }
            })
            .collect(),
    )
    .unwrap()
}
fn scan(a: &Assignment<i64>) -> BTreeMap<usize, IntSummary> {
    let mut groups = BTreeMap::<usize, Vec<i64>>::new();
    for v in a.values().iter().flatten() {
        groups.entry((*v % 2) as usize).or_default().push(*v);
    }
    groups
        .into_iter()
        .map(|(key, mut values)| {
            values.sort_unstable();
            let count = values.len();
            let sum = values.iter().sum();
            let min = values.first().copied();
            let max = values.last().copied();
            values.dedup();
            (
                key,
                IntSummary {
                    count,
                    distinct_count: values.len(),
                    sum,
                    min,
                    max,
                },
            )
        })
        .collect()
}
fn weight(s: &IntSummary) -> i64 {
    s.sum + s.count as i64 + s.distinct_count as i64 + s.min.unwrap_or(0) + s.max.unwrap_or(0)
}
fn check(
    g: &StreamGraph<'_, SimpleScore>,
    data: &StreamDataset<usize, (usize, IntSummary)>,
    a: &Assignment<i64>,
) {
    let want = scan(a);
    let actual = g
        .dataset_rows(data)
        .unwrap()
        .iter()
        .map(|(&k, (_, summary))| (k, *summary))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, want);
    assert_eq!(g.score(), SimpleScore::new(want.values().map(weight).sum()));
}

#[test]
fn optional_filtered_groups_match_complete_scans_through_nested_and_saved_restores() {
    for count in [2, 256, 257] {
        let p = problem(count);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        let filtered = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let mapped = b.map(&filtered, |&(id, v)| (id, v.unwrap())).unwrap();
        let groups = b
            .group_collect_batched(&mapped, |&(_, v)| ((v % 2) as usize, v))
            .unwrap();
        let data = b.dataset(&groups).unwrap();
        b.score("summary", &groups, |(_, s)| SimpleScore::new(weight(s)))
            .unwrap();
        let initial = p.initial_assignment();
        let mut d = StreamScoreDirector::new(&p, b.build(), source, initial.clone()).unwrap();
        check(d.graph(), &data, d.assignment());
        let held = d.graph().dataset_rows(&data).unwrap().clone();
        let movement = Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: None,
            },
            Move::Change {
                variable: count - 1,
                value: Some(3),
            },
        ]);
        {
            let trial = d.apply(&movement).unwrap();
            check(trial.graph(), &data, trial.assignment());
        }
        assert_eq!(d.assignment(), &initial);
        {
            let mut outer = d.edit_session().unwrap();
            outer.apply(&movement).unwrap();
            {
                let mut inner = outer.nested().unwrap();
                inner
                    .apply(&Move::Change {
                        variable: 0,
                        value: Some(4),
                    })
                    .unwrap();
                check(inner.director().graph(), &data, inner.assignment());
                inner.commit();
            }
            check(outer.director().graph(), &data, outer.assignment());
        }
        assert_eq!(d.assignment(), &initial);
        check(d.graph(), &data, d.assignment());
        d.apply(&movement).unwrap().commit();
        let saved = d.assignment().clone();
        {
            let replacement = d.replace_assignment(initial.clone()).unwrap();
            check(replacement.graph(), &data, replacement.assignment());
        }
        assert_eq!(d.assignment(), &saved);
        d.replace_assignment(initial.clone()).unwrap().commit();
        check(d.graph(), &data, d.assignment());
        assert_eq!(held.len(), 2);
        assert_eq!(held[&1].1, scan(&initial)[&1]);
        d.assert_consistent().unwrap();
    }
}

#[test]
fn group_output_namespaces_stop_bounds_and_arbitrary_fact_ids_remain_editable() {
    let p = problem(2);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let first = b
        .group_collect_batched(source.port(), |&(_, v)| {
            let v = v.unwrap();
            (1000 + v as usize, v)
        })
        .unwrap();
    let mapped = b.map(&first, |&(id, s)| (id, s.sum)).unwrap();
    let filtered = b.filter(&mapped, |(_, sum)| *sum > 0).unwrap();
    let second = b
        .group_collect_batched(&filtered, |&(id, sum)| (id % 2, sum))
        .unwrap();
    let data = b.dataset(&second).unwrap();
    b.score("second", &second, |(_, s)| SimpleScore::new(s.sum))
        .unwrap();
    let facts = b.source::<usize, (usize, i64)>();
    let fact_map = b.map(facts.port(), |v| *v).unwrap();
    let fact_groups = b.group_collect_batched(&fact_map, |v| *v).unwrap();
    b.score("facts", &fact_groups, |(_, s)| SimpleScore::new(s.sum))
        .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(3));
    d.apply(&Move::Change {
        variable: 0,
        value: Some(3),
    })
    .unwrap()
    .commit();
    let rows = d.graph().dataset_rows(&data).unwrap();
    assert_eq!(rows[&1].1.sum, 3);
    assert_eq!(rows[&0].1.sum, 2);
    {
        let mut edit = d.edit_facts();
        edit.apply(&facts, TupleChange::put(900_000, (7000, 11)))
            .unwrap();
        edit.apply(&facts, TupleChange::put(1_000_000, (7000, 17)))
            .unwrap();
        assert_eq!(edit.score(), Ok(SimpleScore::new(33)));
        edit.commit().unwrap();
    }
    {
        let mut edit = d.edit_facts();
        edit.apply(&facts, TupleChange::retract(900_000)).unwrap();
        assert_eq!(edit.score(), Ok(SimpleScore::new(22)));
    }
    assert_eq!(d.score(), SimpleScore::new(33));
    d.assert_consistent().unwrap();
}

#[derive(Clone, Debug)]
struct Key(i64, usize, Rc<Cell<usize>>);
impl PartialEq for Key {
    fn eq(&self, rhs: &Self) -> bool {
        self.0 == rhs.0
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Key {
    fn cmp(&self, rhs: &Self) -> Ordering {
        let remaining = self.2.get();
        if remaining > 0 {
            self.2.set(remaining - 1);
            assert_ne!(remaining, 1, "deliberate ordered-key failure");
        }
        self.0.cmp(&rhs.0)
    }
}

#[test]
fn representative_tags_and_each_comparison_failure_survive_real_director_swap() {
    for count in [2, 257] {
        let p = problem(count);
        let budget = Rc::new(Cell::new(0));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<i64>)>();
        let filtered = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
        let groups = b
            .group_collect_batched(&filtered, |&(id, v)| {
                (Key(v.unwrap(), id, budget.clone()), v.unwrap())
            })
            .unwrap();
        let data = b.dataset(&groups).unwrap();
        b.score("sum", &groups, |(_, s)| SimpleScore::new(s.sum))
            .unwrap();
        let initial = p.initial_assignment();
        let mut d = StreamScoreDirector::new(&p, b.build(), source, initial.clone()).unwrap();
        let observe = |g: &StreamGraph<'_, SimpleScore>| {
            g.dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(k, (_, s))| (k.0, k.1, *s))
                .collect::<Vec<_>>()
        };
        let baseline = observe(d.graph());
        let held = d.graph().dataset_rows(&data).unwrap().clone();
        let movement = Move::Swap {
            left: 0,
            right: count - 1,
        };
        budget.set(usize::MAX);
        let trial = d.apply(&movement).unwrap();
        let comparisons = usize::MAX - budget.get();
        budget.set(0);
        let after = observe(trial.graph());
        assert_eq!(
            after
                .iter()
                .map(|(k, tag, _)| (*k, *tag))
                .collect::<Vec<_>>(),
            [(1, 0), (2, count - 1)]
        );
        drop(trial);
        assert!(comparisons > 0);
        for failure in 1..=comparisons {
            budget.set(failure);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = d.apply(&movement).unwrap();
                }))
                .is_err()
            );
            budget.set(0);
            assert_eq!(d.assignment(), &initial);
            assert_eq!(d.score(), SimpleScore::new(3));
            assert_eq!(observe(d.graph()), baseline);
            {
                let retry = d.apply(&movement).unwrap();
                assert_eq!(observe(retry.graph()), after);
            }
            d.assert_consistent().unwrap();
        }
        d.apply(&movement).unwrap().commit();
        assert_eq!(observe(d.graph()), after);
        assert_eq!(
            held.iter().map(|(k, _)| (k.0, k.1)).collect::<Vec<_>>(),
            [(1, 0), (2, count - 1)]
        );
        d.assert_consistent().unwrap();
    }
}
