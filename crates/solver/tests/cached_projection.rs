//! Original owned-cache projection checks, not upstream method adaptations.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone, Default)]
struct Memo(Option<(i64, i64)>);
fn project(memo: &mut Memo, value: i64, work: &Cell<usize>) -> i64 {
    if memo.0.is_none_or(|(old, _)| old != value) {
        work.set(work.get() + 1);
        memo.0 = Some((value, value * value));
    }
    memo.0.unwrap().1
}
#[test]
fn caches_are_per_identity_and_restore_on_undo_and_rebuild() {
    let work = Rc::new(Cell::new(0));
    let count = work.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let p = b
        .map_cached(s.port(), Memo::default(), move |c, v| {
            project(c, *v, &count)
        })
        .unwrap();
    let out = b.score("square", &p, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.apply(&s, TupleChange::put(1, 5)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), 2);
    assert_eq!(g.score(), SimpleScore::new(34));
    let original = g.constraint_matches(&out).unwrap().clone();
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(0, 4)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(41));
    }
    assert_eq!(work.get(), 3);
    assert_eq!(g.constraint_matches(&out).unwrap(), &original);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.apply(&s, TupleChange::put(1, 5)).unwrap();
    t.commit().unwrap();
    assert_eq!(
        work.get(),
        3,
        "undo restored both cache states without recomputing"
    );
    g.assert_consistent().unwrap();
    assert_eq!(work.get(), 5, "cold rebuild evaluates both current rows");
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), 5, "rebuild restored warm caches");
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::retract(0)).unwrap();
        assert_eq!(t.score().unwrap(), SimpleScore::new(25));
    }
    assert_eq!(work.get(), 5);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), 5);
    let mut t = g.transaction();
    t.apply(&s, TupleChange::retract(0)).unwrap();
    t.apply(&s, TupleChange::retract(99)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), 5);
    assert_eq!(g.score(), SimpleScore::new(25));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(
        work.get(),
        6,
        "committed retraction discards that identity's cache"
    );
    assert_eq!(g.constraint_matches(&out).unwrap(), &original);
}
#[test]
fn callback_panic_and_downstream_failure_restore_mutated_cache() {
    let work = Rc::new(Cell::new(0));
    let count = work.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let p = b
        .map_cached(s.port(), Memo::default(), move |c, v| {
            let result = project(c, *v, &count);
            assert!(*v >= 0, "panic after modifying cache");
            result
        })
        .unwrap();
    let out = b
        .score("square", &p, |v| {
            SimpleScore::new(*v).with_structural(i64::from(*v == 49))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    let before = g.constraint_matches(&out).unwrap().clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut t = g.transaction();
            t.apply(&s, TupleChange::put(0, -1)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(g.constraint_matches(&out).unwrap(), &before);
    let after_panic = work.get();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), after_panic);
    {
        let mut t = g.transaction();
        assert!(matches!(
            t.apply(&s, TupleChange::put(0, 7)),
            Err(GraphError::StructuralContribution(_))
        ));
        assert_eq!(t.score(), Err(GraphError::FailedTransaction));
    }
    assert_eq!(g.constraint_matches(&out).unwrap(), &before);
    let after_error = work.get();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 3)).unwrap();
    t.commit().unwrap();
    assert_eq!(work.get(), after_error);
    assert_eq!(g.source_rows(&s).unwrap()[&0], 3);
    g.assert_consistent().unwrap();
}
#[test]
fn checkpointing_and_updating_one_identity_do_not_clone_other_caches() {
    struct Cache {
        value: i64,
        clones: Rc<Cell<usize>>,
    }
    impl Clone for Cache {
        fn clone(&self) -> Self {
            self.clones.set(self.clones.get() + 1);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }
    let clones = Rc::new(Cell::new(0));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let p = b
        .map_cached(
            s.port(),
            Cache {
                value: 0,
                clones: clones.clone(),
            },
            |c, v| {
                c.value = *v * 2;
                c.value
            },
        )
        .unwrap();
    let out = b.score("double", &p, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for i in 0..1000 {
        t.apply(&s, TupleChange::put(i, i as i64)).unwrap();
    }
    t.commit().unwrap();
    let before = g.constraint_matches(&out).unwrap().clone();
    clones.set(0);
    {
        let mut t = g.transaction();
        t.apply(&s, TupleChange::put(512, 12)).unwrap();
        assert_eq!(clones.get(), 1);
    }
    assert_eq!(clones.get(), 1);
    assert_eq!(g.constraint_matches(&out).unwrap(), &before);
}
#[test]
fn diagnostic_rebuild_rejects_history_dependent_output_and_restores_state() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    // Deliberately violate map_cached's current-input-only contract.
    let p = b
        .map_cached(s.port(), 0i64, |state, v| {
            *state += 1;
            v + *state
        })
        .unwrap();
    b.score("invalid history", &p, |v| SimpleScore::new(*v))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 10)).unwrap();
    t.apply(&s, TupleChange::put(0, 20)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(22));
    assert!(matches!(
        g.assert_consistent(),
        Err(GraphError::CorruptedScore(_))
    ));
    assert_eq!(g.score(), SimpleScore::new(22));
    let mut t = g.transaction();
    t.apply(&s, TupleChange::put(0, 30)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(33));
}
#[test]
fn cached_mapping_rejects_foreign_ports_before_installing_a_node() {
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let x = a.source::<usize, i64>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let y = b.source::<usize, i64>();
    assert!(matches!(
        a.map_cached(y.port(), (), |_, v| *v),
        Err(GraphError::ForeignPort)
    ));
    let p = a.map_cached(x.port(), (), |_, v| *v).unwrap();
    a.score("identity", &p, |v| SimpleScore::new(*v)).unwrap();
    let mut g = a.build();
    let mut t = g.transaction();
    t.apply(&x, TupleChange::put(0, 5)).unwrap();
    t.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(5));
}
