//! Original assignment-scope and self-exclusion comparisons; no upstream credit.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Fact {
    id: usize,
    code: i32,
    assigned: Option<i32>,
}
trait Facts: Clone + PartialEq + 'static {
    const ARITY: usize;
    fn first(&self) -> &Fact;
    fn fields(&self) -> Vec<Fact>;
}
impl Facts for Fact {
    const ARITY: usize = 1;
    fn first(&self) -> &Fact {
        self
    }
    fn fields(&self) -> Vec<Fact> {
        vec![self.clone()]
    }
}
impl Facts for (Fact, Fact) {
    const ARITY: usize = 2;
    fn first(&self) -> &Fact {
        &self.0
    }
    fn fields(&self) -> Vec<Fact> {
        vec![self.0.clone(), self.1.clone()]
    }
}
impl Facts for ((Fact, Fact), Fact) {
    const ARITY: usize = 3;
    fn first(&self) -> &Fact {
        &self.0.0
    }
    fn fields(&self) -> Vec<Fact> {
        vec![self.0.0.clone(), self.0.1.clone(), self.1.clone()]
    }
}
impl Facts for (((Fact, Fact), Fact), Fact) {
    const ARITY: usize = 4;
    fn first(&self) -> &Fact {
        &self.0.0.0
    }
    fn fields(&self) -> Vec<Fact> {
        vec![
            self.0.0.0.clone(),
            self.0.0.1.clone(),
            self.0.1.clone(),
            self.1.clone(),
        ]
    }
}
fn full_tuples(rows: &BTreeMap<usize, Fact>, arity: usize) -> Vec<Vec<Fact>> {
    let assigned: Vec<_> = rows.values().filter(|v| v.assigned.is_some()).collect();
    let mut tuples = vec![vec![]];
    for _ in 0..arity {
        tuples = tuples
            .into_iter()
            .flat_map(|prefix| {
                assigned.iter().map(move |v| {
                    let mut next = prefix.clone();
                    next.push((*v).clone());
                    next
                })
            })
            .collect();
    }
    tuples
}
fn exercise<I: Ord + Clone + 'static, T: Facts>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Fact>,
    assigned: StreamPort<usize, Fact>,
    parent: StreamPort<I, T>,
) {
    let mut outputs = Vec::new();
    for including in [false, true] {
        for other in [false, true] {
            for absent in [false, true] {
                let mode = if absent {
                    ExistenceMode::NotExists
                } else {
                    ExistenceMode::Exists
                };
                let d = SharedExistence::new(mode, |v: &T| v.first().code, |v: &Fact| v.code);
                let d = if other {
                    d.with_filter(|a, b| a.first().id != b.id)
                } else {
                    d
                };
                let right = if including { source.port() } else { &assigned };
                let out = d.apply(&mut b, &parent, right).unwrap();
                let again = d.clone().apply(&mut b, &parent, right).unwrap();
                assert!(out.same_node(&again));
                let dataset = b.dataset(&out).unwrap();
                for (suffix, p) in [("a", &out), ("b", &again)] {
                    b.score(format!("{including}-{other}-{absent}-{suffix}"), p, |_| {
                        SimpleScore::new(1)
                    })
                    .unwrap();
                }
                outputs.push((including, other, absent, dataset));
            }
        }
    }
    let mut graph = b.build();
    let mut rows = BTreeMap::new();
    let mut seed = 4321987u64;
    for step in 0usize..400 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (seed >> 32) as usize % 5;
        let value = (!step.is_multiple_of(7)).then_some(Fact {
            id,
            code: ((seed >> 16) % 3) as i32,
            assigned: (!step.is_multiple_of(4)).then_some((seed % 11) as i32),
        });
        let mut next = rows.clone();
        if let Some(ref v) = value {
            next.insert(id, v.clone());
        } else {
            next.remove(&id);
        }
        let tuples = full_tuples(&next, T::ARITY);
        let old_score = graph.score();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange { id, value }).unwrap();
        let mut expected_score = 0;
        for (including, other, absent, dataset) in &outputs {
            let expected: BTreeSet<_> = tuples
                .iter()
                .filter(|tuple| {
                    let a = &tuple[0];
                    let matched = next.values().any(|r| {
                        (*including || r.assigned.is_some())
                            && (!other || a.id != r.id)
                            && a.code == r.code
                    });
                    matched != *absent
                })
                .cloned()
                .collect();
            let actual = tx.graph().unwrap().dataset_rows(dataset).unwrap();
            assert_eq!(actual.len(), expected.len());
            assert_eq!(
                actual
                    .iter()
                    .map(|(_, v)| v.fields())
                    .collect::<BTreeSet<_>>(),
                expected,
                "arity={}, step={step}, include={including}, other={other}, absent={absent}",
                T::ARITY
            );
            expected_score += 2 * expected.len() as i64;
        }
        assert_eq!(tx.score().unwrap(), SimpleScore::new(expected_score));
        if step.is_multiple_of(3) {
            drop(tx);
            assert_eq!(graph.score(), old_score);
        } else {
            tx.commit().unwrap();
            rows = next;
        }
    }
    graph.assert_consistent().unwrap();
}
#[test]
fn assignment_changes_and_self_exclusion_match_full_rebuilds_for_every_arity() {
    for arity in 1..=4 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Fact>();
        let assigned = SharedFilter::new(|v: &Fact| v.assigned.is_some())
            .apply(&mut b, source.port())
            .unwrap();
        if arity == 1 {
            exercise(b, source, assigned.clone(), assigned);
            continue;
        }
        let bi = b.join_equal(&assigned, &assigned, |_| (), |_| ()).unwrap();
        if arity == 2 {
            exercise(b, source, assigned, bi);
            continue;
        }
        let tri = b.join_equal(&bi, &assigned, |_| (), |_| ()).unwrap();
        if arity == 3 {
            exercise(b, source, assigned, tri);
            continue;
        }
        let quad = b.join_equal(&tri, &assigned, |_| (), |_| ()).unwrap();
        exercise(b, source, assigned, quad);
    }
}

#[test]
fn assigned_and_including_unassigned_outputs_stay_separate_for_one_definition() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let all = b.source::<usize, Fact>();
    let assigned = SharedFilter::new(|v: &Fact| v.assigned.is_some())
        .apply(&mut b, all.port())
        .unwrap();
    let d = SharedExistence::new(ExistenceMode::Exists, |v: &Fact| v.code, |v: &Fact| v.code)
        .with_filter(|a, b| a.id != b.id);
    let assigned_other = d.apply(&mut b, &assigned, &assigned).unwrap();
    let all_other = d.apply(&mut b, &assigned, all.port()).unwrap();
    assert!(!assigned_other.same_node(&all_other));
    b.score("assigned", &assigned_other, |_| SimpleScore::new(10))
        .unwrap();
    b.score("all", &all_other, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    for (id, value, score) in [
        (
            0,
            Some(Fact {
                id: 0,
                code: 7,
                assigned: Some(3),
            }),
            0,
        ),
        (
            1,
            Some(Fact {
                id: 1,
                code: 7,
                assigned: None,
            }),
            1,
        ),
        (
            1,
            Some(Fact {
                id: 1,
                code: 7,
                assigned: Some(4),
            }),
            22,
        ),
        (
            0,
            Some(Fact {
                id: 0,
                code: 7,
                assigned: None,
            }),
            1,
        ),
        (0, None, 0),
    ] {
        let mut tx = g.transaction();
        tx.apply(&all, TupleChange { id, value }).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(score));
        tx.commit().unwrap();
    }
    g.assert_consistent().unwrap();
}
