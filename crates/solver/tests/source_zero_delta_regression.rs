//! Original source propagation identity controls; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[test]
fn ordinary_sources_keep_payload_lifecycle_equal_score_work_and_panic_rollback() {
    struct Row(Rc<RefCell<Vec<&'static str>>>, bool);
    impl Clone for Row {
        fn clone(&self) -> Self {
            self.0.borrow_mut().push("clone");
            assert!(!self.1, "injected payload clone panic");
            Self(self.0.clone(), self.1)
        }
    }
    impl Drop for Row {
        fn drop(&mut self) {
            self.0.borrow_mut().push("drop");
        }
    }
    for arithmetic in [ScoringArithmetic::Checked, ScoringArithmetic::Wrapping] {
        let trace = Rc::new(RefCell::new(Vec::new()));
        let panic = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(arithmetic);
        let s = b.source::<usize, Row>();
        b.score("constant", s.port(), |row| {
            row.0.borrow_mut().push("score");
            assert!(!panic.get(), "injected");
            SimpleScore::new(7)
        })
        .unwrap();
        let unused = b.source::<usize, Row>();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, Row(trace.clone(), false)))
                .unwrap();
            assert_eq!(tx.score_evaluations(), 1);
            tx.commit().unwrap();
        }
        assert_eq!(&*trace.borrow(), &["clone", "score", "clone", "drop"]);
        trace.borrow_mut().clear();
        {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, Row(trace.clone(), false)))
                .unwrap();
            assert_eq!(tx.score_evaluations(), 1);
            assert_eq!(tx.score().unwrap(), SimpleScore::new(7));
            tx.commit().unwrap();
        }
        assert_eq!(&trace.borrow()[..4], &["clone", "score", "clone", "drop"]);
        let held = g.source_rows(&s).unwrap().clone();
        trace.borrow_mut().clear();
        {
            let mut tx = g.transaction();
            tx.apply(&unused, TupleChange::put(9, Row(trace.clone(), false)))
                .unwrap();
            assert_eq!(tx.score_evaluations(), 0);
            assert_eq!(tx.score().unwrap(), SimpleScore::new(7));
            tx.commit().unwrap();
        }
        assert_eq!(&*trace.borrow(), &["clone", "drop"]);
        let held_unused = g.source_rows(&unused).unwrap().clone();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply(&unused, TupleChange::put(9, Row(trace.clone(), true)))
                    .unwrap();
            }))
            .is_err()
        );
        assert_eq!(g.score(), SimpleScore::new(7));
        assert!(std::ptr::eq(
            &g.source_rows(&unused).unwrap()[&9],
            &held_unused[&9]
        ));
        {
            let mut tx = g.transaction();
            tx.apply(&unused, TupleChange::retract(9)).unwrap();
            assert!(tx.graph().unwrap().source_rows(&unused).unwrap().is_empty());
            assert_eq!(tx.score_evaluations(), 0);
        }
        assert!(std::ptr::eq(
            &g.source_rows(&unused).unwrap()[&9],
            &held_unused[&9]
        ));
        panic.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply(&s, TupleChange::put(0, Row(trace.clone(), false)))
                    .unwrap();
            }))
            .is_err()
        );
        assert_eq!(g.score(), SimpleScore::new(7));
        assert!(std::ptr::eq(&g.source_rows(&s).unwrap()[&0], &held[&0]));
        panic.set(false);
        {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::retract(0)).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(g.score(), SimpleScore::ZERO);
        assert_eq!(held.len(), 1);
    }
}

#[test]
fn bound_planning_source_equal_impacts_recover_and_install_retained_search_state() {
    for arithmetic in [ScoringArithmetic::Checked, ScoringArithmetic::Wrapping] {
        let calls = Cell::new(0);
        let panic = Cell::new(false);
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(arithmetic);
        let s = b.source::<usize, (usize, Option<i32>)>();
        let inspect = s.clone();
        b.score("value", s.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            assert!(!panic.get(), "injected");
            SimpleScore::new(i64::from(v.unwrap_or(0) / 2))
        })
        .unwrap();
        let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let held = d.graph().source_rows(&inspect).unwrap().clone();
        calls.set(0);
        {
            let trial = d
                .apply(&Move::Change {
                    variable: 0,
                    value: Some(1),
                })
                .unwrap();
            assert_eq!(trial.score(), SimpleScore::ZERO);
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(d.assignment().values(), &[Some(0)]);
        panic.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _trial = d
                    .apply(&Move::Change {
                        variable: 0,
                        value: Some(2),
                    })
                    .unwrap();
            }))
            .is_err()
        );
        panic.set(false);
        assert_eq!(d.score(), SimpleScore::ZERO);
        assert_eq!(d.graph().source_rows(&inspect).unwrap(), &held);
        let result = d
            .run_local_search(LocalSearchConfig {
                max_steps: 1,
                max_evaluations: 10,
                ..Default::default()
            })
            .unwrap();
        assert!(result.evaluations > 0);
        assert_eq!(d.assignment().values(), &[Some(2)]);
        assert_eq!(d.score(), SimpleScore::new(1));
        assert_eq!(held[&0], (0, Some(0)));
        d.assert_consistent().unwrap();
    }
}
