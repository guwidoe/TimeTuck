//! Original transactional dataset capture and scoring-director integration checks.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn capture_shares_only_the_same_port_and_rejects_foreign_handles() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let d = b.dataset(s.port()).unwrap();
    assert_eq!(d, b.dataset(s.port()).unwrap());
    let mapped = b.map(s.port(), |v| *v).unwrap();
    assert_ne!(d, b.dataset(&mapped).unwrap());
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        other.dataset(s.port()),
        Err(GraphError::ForeignPort)
    ));
    let foreign = other.source::<usize, i32>();
    let foreign_dataset = other.dataset(foreign.port()).unwrap();
    let other = other.build();
    let mut g = b.build();
    assert!(matches!(
        g.dataset_rows(&foreign_dataset),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        other.dataset_rows(&d),
        Err(GraphError::ForeignPort)
    ));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(1, 7)).unwrap();
    assert!(t.apply(&foreign, TupleChange::put(0, 4)).is_err());
    assert!(matches!(t.graph(), Err(GraphError::FailedTransaction)));
    drop(t);
    assert!(g.dataset_rows(&d).unwrap().is_empty());
}
#[test]
fn dataset_only_branches_survive_disabled_constraints_without_score_callbacks() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let doubled = b
        .map(s.port(), |v| {
            calls.set(calls.get() + 1);
            v * 2
        })
        .unwrap();
    let d = b.dataset(&doubled).unwrap();
    b.score_weighted("disabled", &doubled, SimpleScore::ZERO, |_| {
        panic!("disabled score ran")
    })
    .unwrap();
    let dead = b
        .map(s.port(), |_: &i32| -> i32 {
            panic!("unused disabled branch ran")
        })
        .unwrap();
    b.score_weighted("dead", &dead, SimpleScore::ZERO, |v| i64::from(*v))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(2, 4)).unwrap();
    assert_eq!(t.score_evaluations(), 0);
    assert_eq!(t.graph().unwrap().dataset_rows(&d).unwrap()[&2], 8);
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert_eq!(calls.get(), 1);
    g.assert_consistent().unwrap();
    assert_eq!(calls.get(), 2);
}
#[test]
fn trial_commit_rollback_and_panics_keep_held_snapshots_immutable() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let mapped = b
        .map(s.port(), |v| {
            assert_ne!(*v, 13);
            v * 2
        })
        .unwrap();
    let d = b.dataset(&mapped).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 2)).unwrap();
    t.commit().unwrap();
    let old = g.dataset_rows(&d).unwrap().clone();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 5)).unwrap();
    t.apply(&s, TupleChange::put(1, 7)).unwrap();
    let held = t.graph().unwrap().dataset_snapshot(&d).unwrap();
    drop(t);
    assert_eq!(g.dataset_rows(&d).unwrap(), &old);
    assert_eq!(held.iter().copied().collect::<Vec<_>>(), vec![10, 14]);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.apply(&s, TupleChange::put(2, 8)).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| t.apply(&s, TupleChange::put(3, 13)))).is_err());
    assert!(t.graph().is_err());
    drop(t);
    assert_eq!(g.dataset_rows(&d).unwrap(), &old);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 9)).unwrap();
    t.commit().unwrap();
    assert_eq!(old[&0], 4);
    assert_eq!(g.dataset_rows(&d).unwrap()[&0], 18);
    g.assert_consistent().unwrap();
}
#[test]
fn diagnostic_rebuild_compares_dataset_payloads_even_without_scores() {
    let multiplier = Cell::new(2);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let mapped = b.map(s.port(), |v| v * multiplier.get()).unwrap();
    let d = b.dataset(&mapped).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    multiplier.set(5);
    assert_eq!(g.assert_consistent(), Err(GraphError::CorruptedScore));
    assert_eq!(g.dataset_rows(&d).unwrap()[&0], 6);
    multiplier.set(2);
    g.assert_consistent().unwrap();
}
#[test]
fn basic_scored_moves_update_and_restore_captured_groups_for_every_partial_state() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1])
            .allowing_unassigned()
            .with_initial(0);
        3
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    let present = b.filter(s.port(), |(_, v)| v.is_some()).unwrap();
    let grouped = b.group_list(&present, |(id, v)| (v.unwrap(), *id)).unwrap();
    let d = b.dataset(&grouped).unwrap();
    b.score("sum", s.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut director = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let original = director.graph().dataset_rows(&d).unwrap().clone();
    for n in 0..27 {
        let mut digits = n;
        let values = (0..3)
            .map(|_| {
                let x = digits % 3;
                digits /= 3;
                match x {
                    0 => None,
                    1 => Some(0),
                    _ => Some(1),
                }
            })
            .collect::<Vec<_>>();
        let movement = Move::Composite(
            values
                .iter()
                .enumerate()
                .map(|(variable, &value)| Move::Change { variable, value })
                .collect(),
        );
        let trial = director.apply(&movement).unwrap();
        let mut expected: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
        for (i, v) in values.iter().enumerate() {
            if let Some(v) = v {
                expected.entry(*v).or_default().push(i);
            }
        }
        let actual = trial
            .graph()
            .dataset_rows(&d)
            .unwrap()
            .iter()
            .map(|(k, (_, list))| (*k, list.iter().copied().collect::<Vec<_>>()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual, expected);
        drop(trial);
        assert_eq!(director.graph().dataset_rows(&d).unwrap(), &original);
    }
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(1),
        })
        .unwrap()
        .commit();
    assert_eq!(
        director.graph().dataset_rows(&d).unwrap()[&1]
            .1
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![0]
    );
    director.assert_consistent().unwrap();
}

#[test]
fn list_and_mixed_directors_capture_final_lists_shadows_and_joint_rows_atomically() {
    let lists = ListProblem::new(
        vec![10, 20],
        vec![ListEntity::new(vec![10]), ListEntity::new(vec![20])],
    )
    .unwrap();
    let swap = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = ListStreamSources::new(&mut b);
    let entities = b.dataset(s.entities.port()).unwrap();
    let values = b.dataset(s.values.port()).unwrap();
    let mut d =
        ListStreamScoreDirector::new(&lists, b.build(), s, lists.initial_assignment()).unwrap();
    let old = d.graph().dataset_rows(&values).unwrap().clone();
    {
        let t = d.apply(&swap).unwrap();
        assert_eq!(t.graph().dataset_rows(&entities).unwrap()[&0].1, vec![20]);
        assert_eq!(
            t.graph().dataset_rows(&values).unwrap()[&0]
                .position
                .unwrap()
                .entity,
            1
        );
    }
    assert_eq!(d.graph().dataset_rows(&values).unwrap(), &old);
    d.apply(&swap).unwrap().commit();
    assert_eq!(
        d.graph().dataset_rows(&values).unwrap()[&1]
            .position
            .unwrap()
            .entity,
        0
    );
    d.assert_consistent().unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        lists,
    );
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = MixedStreamSources::new(&mut b);
    let joined = b
        .join_equal(
            s.basic.port(),
            s.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    let data = b.dataset(&joined).unwrap();
    b.score("joint", &joined, |((_, v), (_, list))| {
        SimpleScore::new(i64::from(v.unwrap()) * list.len() as i64)
    })
    .unwrap();
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let old = d.graph().dataset_rows(&data).unwrap().clone();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(1),
        },
        lists: swap,
    };
    {
        let t = d.apply(&movement).unwrap();
        assert_eq!(
            t.graph().dataset_rows(&data).unwrap()[&(0, 0)],
            ((0, Some(1)), (0, vec![20]))
        );
        assert_eq!(t.score(), SimpleScore::new(1));
    }
    assert_eq!(d.graph().dataset_rows(&data).unwrap(), &old);
    d.apply(&movement).unwrap().commit();
    d.assert_consistent().unwrap();
    assert_eq!(
        d.graph().dataset_rows(&data).unwrap()[&(0, 0)].1.1,
        vec![20]
    );
}
