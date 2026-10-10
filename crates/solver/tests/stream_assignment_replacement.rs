//! Original replacement failure and payload regressions; no upstream credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn mixed_replacement_validates_before_callbacks_and_recovers_from_errors_and_panics() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap(),
    );
    let fact_calls = Cell::new(0);
    let mode = Cell::new(0);
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::<i32, i32>::new(&mut b);
    let facts = b.source::<usize, i64>();
    b.score("facts", facts.port(), |weight| {
        fact_calls.set(fact_calls.get() + 1);
        SimpleScore::new(*weight)
    })
    .unwrap();
    b.score("basic", sources.basic.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    b.score("last", sources.lists.values.port(), |r| {
        calls.set(calls.get() + 1);
        if r.value == 2 {
            match mode.get() {
                1 => return SimpleScore::ZERO.with_structural(-1),
                2 => panic!("last shadow callback"),
                _ => (),
            }
        }
        SimpleScore::new(r.position.map_or(0, |p| p.index as i64))
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&facts, TupleChange::put(0, 10)).unwrap();
    tx.commit().unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, graph, sources.clone(), p.initial_assignment()).unwrap();
    let next = MixedAssignment::new(
        Assignment::new(vec![Some(1)]),
        ListAssignment::new(vec![vec![2, 1]]),
    );
    let before_score = d.score();
    let work = calls.get();
    let invalid = MixedAssignment::new(
        Assignment::new(vec![Some(1)]),
        ListAssignment::new(vec![vec![2, 2]]),
    );
    assert!(d.replace_assignment(invalid).is_err());
    assert_eq!(calls.get(), work);
    mode.set(1);
    assert!(matches!(
        d.replace_assignment(next.clone()),
        Err(MixedStreamScoringError::Graph(
            GraphError::StructuralContribution(_)
        ))
    ));
    mode.set(2);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.replace_assignment(next.clone());
        }))
        .is_err()
    );
    mode.set(0);
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), before_score);
    assert_eq!(
        d.graph().source_rows(&sources.basic).unwrap()[&0],
        (0, Some(0))
    );
    assert_eq!(
        d.graph().source_rows(&sources.lists.entities).unwrap()[&0],
        (0, vec![1, 2])
    );
    assert_eq!(
        d.graph().source_rows(&sources.lists.values).unwrap()[&1].position,
        Some(ListPosition {
            entity: 0,
            index: 1
        })
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let trial = d.replace_assignment(next.clone()).unwrap();
            assert_eq!(trial.score(), SimpleScore::new(12));
            panic!("caller unwind");
        }))
        .is_err()
    );
    assert_eq!(d.assignment(), &p.initial_assignment());
    assert_eq!(d.score(), before_score);
    let trial = d.replace_assignment(next.clone()).unwrap();
    let work = calls.get();
    trial.commit();
    assert_eq!(calls.get(), work);
    assert_eq!(d.assignment(), &next);
    assert_eq!(
        fact_calls.get(),
        1,
        "replacements must retain immutable fact scores"
    );
    assert_eq!(d.graph().source_rows(&facts).unwrap()[&0], 10);
    d.assert_consistent().unwrap();
}

#[derive(Clone, Debug)]
struct Value {
    id: u8,
    weight: i64,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}

#[test]
fn equal_identity_replacement_refreshes_payload_in_basic_and_list_rows() {
    let old = Value { id: 1, weight: 2 };
    let next = Value { id: 1, weight: 7 };
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![old.clone()]).with_initial(old.clone()),
        ])
        .unwrap(),
        ListProblem::new(vec![old.clone()], vec![ListEntity::new(vec![old])]).unwrap(),
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::<Value, Value>::new(&mut b);
    b.score("basic", sources.basic.port(), |(_, v)| {
        SimpleScore::new(v.as_ref().map_or(0, |v| v.weight))
    })
    .unwrap();
    b.score("list", sources.lists.entities.port(), |(_, v)| {
        SimpleScore::new(v.iter().map(|v| v.weight).sum())
    })
    .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    let a = MixedAssignment::new(
        Assignment::new(vec![Some(next.clone())]),
        ListAssignment::new(vec![vec![next]]),
    );
    assert_eq!(&a, d.assignment());
    let trial = d.replace_assignment(a.clone()).unwrap();
    assert_eq!(trial.score(), SimpleScore::new(14));
    drop(trial);
    assert_eq!(d.score(), SimpleScore::new(4));
    assert_eq!(
        d.assignment().basic().values()[0].as_ref().unwrap().weight,
        2
    );
    d.replace_assignment(a).unwrap().commit();
    assert_eq!(d.score(), SimpleScore::new(14));
    d.assert_consistent().unwrap();
}
