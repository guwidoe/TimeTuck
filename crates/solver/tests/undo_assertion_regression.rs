//! Original fault-injection checks: candidate scoring may conceal a broken undo.
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use timetuck::*;
trait TestAssignment {
    type Change;
    fn value(&self) -> i64;
}
impl TestAssignment for Assignment<i64> {
    type Change = Vec<usize>;
    fn value(&self) -> i64 {
        self.values().iter().flatten().sum()
    }
}
impl TestAssignment for ListAssignment<i64> {
    type Change = Vec<usize>;
    fn value(&self) -> i64 {
        self.lists()
            .iter()
            .enumerate()
            .map(|(i, l)| i as i64 * l.iter().sum::<i64>())
            .sum()
    }
}
impl TestAssignment for MixedAssignment<i64, i64> {
    type Change = MixedCalculatorChange;
    fn value(&self) -> i64 {
        10 * self.basic().value() + self.lists().value()
    }
}
struct FaultyUndo<A> {
    root: i64,
    score: i64,
    changed: bool,
    fail: bool,
    changes: Rc<Cell<usize>>,
    marker: PhantomData<A>,
}
impl<A> FaultyUndo<A> {
    fn new(fail: bool, changes: Rc<Cell<usize>>) -> Self {
        Self {
            root: 0,
            score: 0,
            changed: false,
            fail,
            changes,
            marker: PhantomData,
        }
    }
}
impl<A: TestAssignment> IncrementalCalculator<A> for FaultyUndo<A> {
    type Score = SimpleScore;
    type Change = A::Change;
    type Justification = ();
    fn reset(
        &mut self,
        a: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.root = a.value();
        self.score = self.root;
        self.changed = false;
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &A::Change,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.changes.set(self.changes.get() + 1);
        let value = a.value();
        self.score = value + i64::from(self.fail && self.changed && value == self.root);
        self.changed = true;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.score)
    }
}
fn config(assert_scores: bool) -> LocalSearchConfig<'static, SimpleScore> {
    LocalSearchConfig {
        assert_scores,
        max_steps: 1,
        max_evaluations: 2,
        ..Default::default()
    }
}
fn list_problem() -> ListProblem<i64> {
    ListProblem::new(
        vec![1],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
    )
    .unwrap()
}
fn list_move() -> ListMove<i64> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
#[test]
fn basic_assertions_reject_a_bad_undo_before_replaying_the_first_accepted_winner() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    for (assertions, fault) in [(false, true), (true, false), (true, true)] {
        let changes = Rc::new(Cell::new(0));
        let count = changes.clone();
        let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(1),
            }]
        };
        let result = calculator_local_search_with_provider(
            &p,
            p.initial_assignment(),
            || FaultyUndo::<Assignment<i64>>::new(fault, count.clone()),
            config(assertions),
            &mut provider,
        );
        if assertions && fault {
            assert!(matches!(
                result,
                Err(LocalSearchError::Scoring(ScoringError::ScoreCorruption(_)))
            ));
            assert_eq!(changes.get(), 2);
        } else {
            let r = result.unwrap();
            assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.constraint_evaluations, 2);
            assert_eq!(changes.get(), 3);
        }
    }
}
#[test]
fn list_assertions_reject_a_bad_undo_before_replaying_the_first_accepted_winner() {
    let p = list_problem();
    for (assertions, fault) in [(false, true), (true, false), (true, true)] {
        let changes = Rc::new(Cell::new(0));
        let count = changes.clone();
        let mut provider =
            |_: &ListProblem<i64>, _: &ListAssignment<i64>, _: u64| vec![list_move()];
        let result = list_calculator_local_search_with_provider(
            &p,
            p.initial_assignment(),
            || FaultyUndo::<ListAssignment<i64>>::new(fault, count.clone()),
            config(assertions),
            &mut provider,
        );
        if assertions && fault {
            assert!(matches!(
                result,
                Err(ListLocalSearchError::Scoring(
                    ListScoringError::ScoreCorruption(_)
                ))
            ));
            assert_eq!(changes.get(), 2);
        } else {
            let r = result.unwrap();
            assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.constraint_evaluations, 2);
            assert_eq!(changes.get(), 3);
        }
    }
}
#[test]
fn mixed_assertions_reject_a_bad_undo_before_replaying_the_first_accepted_winner() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
    let p = MixedProblem::new(basic, list_problem());
    for (assertions, fault) in [(false, true), (true, false), (true, true)] {
        let changes = Rc::new(Cell::new(0));
        let count = changes.clone();
        let mut provider = |_: &MixedProblem<i64, i64>, _: &MixedAssignment<i64, i64>, _: u64| {
            vec![MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                lists: list_move(),
            }]
        };
        let result = mixed_calculator_local_search_with_provider(
            &p,
            p.initial_assignment(),
            || FaultyUndo::<MixedAssignment<i64, i64>>::new(fault, count.clone()),
            MixedLocalSearchConfig {
                search: config(assertions),
                ..Default::default()
            },
            &mut provider,
        );
        if assertions && fault {
            assert!(matches!(
                result,
                Err(MixedLocalSearchError::Scoring(
                    MixedScoringError::ScoreCorruption(_)
                ))
            ));
            assert_eq!(changes.get(), 2);
        } else {
            let r = result.unwrap();
            assert_eq!(r.best.unwrap().score, SimpleScore::new(11));
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.constraint_evaluations, 2);
            assert_eq!(changes.get(), 3);
        }
    }
}
