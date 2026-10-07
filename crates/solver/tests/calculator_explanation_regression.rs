use std::{
    cell::Cell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[test]
fn owned_snapshots_keep_occurrences_custom_justifications_and_empty_constraints() {
    struct Payload {
        label: &'static str,
        facts: Vec<u32>,
    }
    let mut r = MatchRegistry::new(true);
    let first = r
        .register(
            "cost",
            SimpleScore::new(-5),
            Payload {
                label: "same",
                facts: vec![4, 4, 8],
            },
        )
        .unwrap();
    let second = r
        .register(
            "cost",
            SimpleScore::new(-5),
            Payload {
                label: "same",
                facts: vec![4, 4, 8],
            },
        )
        .unwrap();
    let bonus = r
        .register(
            "bonus",
            SimpleScore::new(3),
            Payload {
                label: "bonus",
                facts: vec![8],
            },
        )
        .unwrap();
    let zero = r
        .register(
            "zero",
            SimpleScore::ZERO,
            Payload {
                label: "zero",
                facts: vec![],
            },
        )
        .unwrap();
    let mut order = vec![];
    let before = explain_calculator_matches(&r, |m| {
        order.push(m.constraint.clone());
        (
            m.justification.label.to_owned(),
            m.justification.facts.clone(),
        )
    })
    .unwrap();
    assert_eq!(order, ["bonus", "cost", "cost", "zero"]);
    assert_eq!(before.score, SimpleScore::new(-7));
    assert_eq!(before.indictments[&4].score, SimpleScore::new(-10));
    assert_eq!(before.indictments[&8].score, SimpleScore::new(-7));
    assert_eq!(before.indictments[&8].matches.len(), 3);
    assert_eq!(before.constraints["cost"].matches.len(), 2);
    assert_ne!(first.id(), second.id());
    assert_eq!(before.constraints["zero"].matches.len(), 1);
    first.cancel(&mut r).unwrap();
    let after = explain_calculator_matches(&r, |m| {
        (
            m.justification.label.to_owned(),
            m.justification.facts.clone(),
        )
    })
    .unwrap();
    let delta = after.diff(&before);
    assert_eq!(delta.score_delta, SimpleScore::new(5));
    assert_eq!(delta.indictments[&4].score_delta, SimpleScore::new(5));
    assert_eq!(
        delta.constraints["cost"]
            .after
            .as_ref()
            .unwrap()
            .matches
            .len(),
        1
    );
    second.cancel(&mut r).unwrap();
    bonus.cancel(&mut r).unwrap();
    zero.cancel(&mut r).unwrap();
    let empty = explain_calculator_matches_default(&r, |m| m.justification.facts.clone()).unwrap();
    assert_eq!(empty.constraints.len(), 3);
    assert!(
        empty
            .constraints
            .values()
            .all(|c| c.matches.is_empty() && c.score == SimpleScore::ZERO)
    );
    assert!(empty.indictments.is_empty());
    r.reset();
    assert!(
        explain_calculator_matches_default(&r, |m| m.justification.facts.clone())
            .unwrap()
            .constraints
            .is_empty()
    );
    assert_eq!(before.constraints["cost"].matches.len(), 2);
    assert_eq!(before.score, SimpleScore::new(-7));
}

#[test]
fn fact_totals_are_checked_without_partial_registry_mutation() {
    for same_constraint in [false, true] {
        let mut r = MatchRegistry::new(true);
        r.register("a", SimpleScore::new(i64::MAX), 1).unwrap();
        r.register(
            if same_constraint { "a" } else { "b" },
            SimpleScore::new(-i64::MAX),
            2,
        )
        .unwrap();
        r.register(
            if same_constraint { "a" } else { "c" },
            SimpleScore::new(1),
            1,
        )
        .unwrap();
        let result = explain_calculator_matches_default(&r, |m| vec![m.justification]);
        assert_eq!(result.unwrap_err(), MatchRegistryError::ScoreOverflow);
        assert_eq!(*r.total().unwrap(), SimpleScore::new(1));
        assert_eq!(
            r.constraints()
                .unwrap()
                .values()
                .map(|c| c.matches().len())
                .sum::<usize>(),
            3
        );
    }
    // Snapshot order is a,b,c; the final representable total must survive an
    // intermediate i64 overflow when aggregating a fact across constraints.
    let mut r = MatchRegistry::new(true);
    for (name, score) in [("a", i64::MAX), ("c", -i64::MAX), ("b", 1)] {
        r.register(name, SimpleScore::new(score), ()).unwrap();
    }
    assert_eq!(
        explain_calculator_matches_default(&r, |_| vec![9])
            .unwrap()
            .indictments[&9]
            .score,
        SimpleScore::new(1)
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ =
                explain_calculator_matches::<_, _, (), u32>(&r, |_| panic!("explanation failed"));
        }))
        .is_err()
    );
    assert_eq!(*r.total().unwrap(), SimpleScore::new(1));
    let disabled = MatchRegistry::<SimpleScore, ()>::new(false);
    assert_eq!(
        explain_calculator_matches::<_, _, (), u32>(&disabled, |_| panic!("disabled callback"))
            .unwrap_err(),
        MatchRegistryError::Disabled
    );
}

trait Rows {
    type Change;
    fn rows(&self) -> Vec<i64>;
}
impl Rows for Assignment<i64> {
    type Change = Vec<usize>;
    fn rows(&self) -> Vec<i64> {
        self.values().iter().map(|v| v.unwrap_or(0)).collect()
    }
}
impl Rows for ListAssignment<i64> {
    type Change = Vec<usize>;
    fn rows(&self) -> Vec<i64> {
        self.lists().iter().map(|l| l.iter().sum()).collect()
    }
}
impl Rows for MixedAssignment<i64, i64> {
    type Change = MixedCalculatorChange;
    fn rows(&self) -> Vec<i64> {
        self.basic()
            .rows()
            .into_iter()
            .chain(self.lists().rows())
            .collect()
    }
}
struct Recording<P> {
    score: i64,
    offset: i64,
    calls: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
    handles: Vec<MatchRegistration<SimpleScore, usize>>,
    marker: PhantomData<P>,
}
impl<P> Recording<P> {
    fn new(calls: Rc<Cell<usize>>, fail: Rc<Cell<bool>>, offset: i64) -> Self {
        Self {
            score: 0,
            offset,
            calls,
            fail,
            handles: vec![],
            marker: PhantomData,
        }
    }
}
impl<P: Rows> IncrementalCalculator<P> for Recording<P> {
    type Score = SimpleScore;
    type Justification = usize;
    type Change = P::Change;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        p: &P,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.handles.clear();
        self.score = p.rows().iter().sum();
        if r.is_enabled() {
            for (id, score) in p.rows().into_iter().enumerate() {
                self.handles
                    .push(r.register("rows", SimpleScore::new(score), id)?);
            }
        }
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &P,
        _: &P::Change,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        if self.fail.get() {
            return Err(MatchRegistryError::Calculator("callback failed".into()));
        }
        for h in self.handles.drain(..) {
            h.cancel(r)?;
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        p: &P,
        _: &P::Change,
        r: &mut MatchRegistry<SimpleScore, usize>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(p, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.calls.set(self.calls.get() + 1);
        SimpleScore::new(self.score + self.offset)
    }
}
macro_rules! director_case {
    ($test:ident,$P:ty,$director:ident,$problem:expr,$movement:expr) => {
        #[test]
        fn $test() {
            let calls = Rc::new(Cell::new(0));
            let fail = Rc::new(Cell::new(false));
            let p = $problem;
            let factory = || Recording::<$P>::new(calls.clone(), fail.clone(), 0);
            let mut d = $director::new(&p, p.initial_assignment(), factory, true).unwrap();
            let initial = d.assignment().clone();
            let before = d
                .explain_default(|m| vec![m.justification, m.justification])
                .unwrap();
            assert!(before.matches_agree());
            assert!(before.score.structural() < 0);
            let n = calls.get();
            let custom = d
                .explain(|m| (format!("row {}", m.justification), vec![m.justification]))
                .unwrap();
            assert_eq!(calls.get(), n);
            assert_eq!(custom.matches.score, before.matches.score);
            assert!(
                before.matches.constraints["rows"]
                    .matches
                    .values()
                    .all(|m| m.justification.facts.len() == 2 && m.indicted_facts.len() == 1)
            );
            drop(d.apply(&$movement).unwrap());
            assert_eq!(d.assignment(), &initial);
            let restored = d.explain_default(|m| vec![m.justification]).unwrap();
            assert_eq!(restored.score, before.score);
            assert_eq!(restored.matches.score, before.matches.score);
            d.apply(&$movement).unwrap().commit();
            assert_ne!(d.score().unwrap(), before.score);
            assert!(
                d.explain_default(|m| vec![m.justification])
                    .unwrap()
                    .matches_agree()
            );
            d.reset(initial.clone()).unwrap();
            fail.set(true);
            assert!(d.apply(&$movement).is_err());
            assert!(
                d.explain::<(), usize>(|_| panic!("unhealthy callback"))
                    .is_err()
            );
            fail.set(false);
            d.reset(initial).unwrap();
            assert!(
                d.explain_default(|m| vec![m.justification])
                    .unwrap()
                    .matches_agree()
            );
            let disabled = $director::new(&p, p.initial_assignment(), factory, false).unwrap();
            assert!(
                disabled
                    .explain::<(), usize>(|_| panic!("disabled callback"))
                    .is_err()
            );
            let partial = $director::new(
                &p,
                p.initial_assignment(),
                || Recording::<$P>::new(calls.clone(), fail.clone(), 17),
                true,
            )
            .unwrap();
            assert!(
                !partial
                    .explain_default(|m| vec![m.justification])
                    .unwrap()
                    .matches_agree()
            );
        }
    };
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![3, 7]); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![3, 7], vec![ListEntity::new(vec![]); 2]).unwrap()
}
fn assign() -> ListMove<i64> {
    ListMove::Assign {
        value: 3,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    }
}
director_case!(
    basic_snapshots,
    Assignment<i64>,
    CalculatorScoreDirector,
    basic(),
    Move::Change {
        variable: 0,
        value: Some(3)
    }
);
director_case!(
    list_snapshots,
    ListAssignment<i64>,
    ListCalculatorScoreDirector,
    lists(),
    assign()
);
director_case!(mixed_snapshots,MixedAssignment<i64,i64>,MixedCalculatorScoreDirector,MixedProblem::new(basic(),lists()),MixedMove{basic:Move::Change{variable:0,value:Some(3)},lists:assign()});
