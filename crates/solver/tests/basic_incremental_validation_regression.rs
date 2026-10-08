//! Original validation-boundary/work regressions; no upstream method credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    unrelated: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.id >= 2 && other.id >= 2 {
            self.unrelated.set(self.unrelated.get() + 1);
        }
        self.id == other.id
    }
}
impl Eq for Value {}
fn fixture() -> (FiniteProblem<Value>, Rc<Cell<usize>>) {
    let count = Rc::new(Cell::new(0));
    let value = |id| Value {
        id,
        unrelated: count.clone(),
    };
    let p = FiniteProblem::new(
        (0..64)
            .map(|i| {
                let domain = if i < 2 {
                    vec![value(0), value(1)]
                } else {
                    vec![value(i), value(i + 64)]
                };
                Variable::new(domain).with_initial(value(i))
            })
            .collect(),
    )
    .unwrap();
    (p, count)
}
fn score(a: &Assignment<Value>) -> SimpleScore {
    SimpleScore::new(
        a.values()
            .iter()
            .enumerate()
            .map(|(i, v)| ((i + 1) * v.as_ref().map_or(0, |v| v.id)) as i64)
            .sum(),
    )
}
#[derive(Default)]
struct Calc(SimpleScore);
impl IncrementalCalculator<Assignment<Value>> for Calc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &Assignment<Value>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = score(a);
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<Value>,
        _: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.0
    }
}
#[derive(Default)]
struct MixedCalc(Calc);
impl IncrementalCalculator<MixedAssignment<Value, i32>> for MixedCalc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = MixedCalculatorChange;
    fn reset(
        &mut self,
        a: &MixedAssignment<Value, i32>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0.reset(a.basic(), r)
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<Value, i32>,
        _: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.0.0
    }
}
#[test]
fn six_basic_and_mixed_backends_skip_unrelated_domain_comparisons() {
    let (p, count) = fixture();
    let movement = Move::Swap { left: 0, right: 1 };
    let initial = p.initial_assignment();
    let old = score(&initial);
    let mut expected = initial.clone();
    p.apply(&mut expected, &movement).unwrap().commit();
    let new = score(&expected);
    assert_ne!(old, new);
    let net = ConstraintNetwork::<_, SimpleScore>::new(64);
    let mut d = ScoreDirector::new(&p, &net, initial.clone()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().values()[0].as_ref().unwrap().id, 1);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().values()[0].as_ref().unwrap().id, 0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let src = b.source::<usize, (usize, Option<Value>)>();
    let mut d = StreamScoreDirector::new(&p, b.build(), src, initial.clone()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().values()[0].as_ref().unwrap().id, 1);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().values()[0].as_ref().unwrap().id, 0);
    let mut d = CalculatorScoreDirector::new(&p, initial, Calc::default, false).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.score(), new);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.score().unwrap(), old);
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score().unwrap(), new);
    let mut bad = p.initial_assignment().values().to_vec();
    bad[63] = bad[62].clone();
    assert!(d.reset(Assignment::new(bad)).is_err());
    assert_eq!(d.score().unwrap(), new);
    let p = MixedProblem::new(
        p,
        ListProblem::new(vec![0], vec![ListEntity::new(vec![0])]).unwrap(),
    );
    let movement = MixedMove {
        basic: movement,
        lists: ListMove::Composite(vec![]),
    };
    let net = MixedConstraintNetwork::<_, _, SimpleScore>::new(64, 1, 1);
    let mut d = MixedScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(
            trial.assignment().basic().values()[0].as_ref().unwrap().id,
            1
        );
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().basic().values()[0].as_ref().unwrap().id, 0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let src = MixedStreamSources::new(&mut b);
    let mut d = MixedStreamScoreDirector::new(&p, b.build(), src, p.initial_assignment()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(
            trial.assignment().basic().values()[0].as_ref().unwrap().id,
            1
        );
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().basic().values()[0].as_ref().unwrap().id, 0);
    let mut d =
        MixedCalculatorScoreDirector::new(&p, p.initial_assignment(), MixedCalc::default, false)
            .unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.score(), new);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.score().unwrap(), old);
    d.apply(&movement).unwrap().commit();
    assert_eq!(d.score().unwrap(), new);
}
#[test]
fn external_input_is_checked_even_when_no_variables_are_written() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![2, 3]).with_initial(2),
    ])
    .unwrap();
    for values in [
        vec![Some(0)],
        vec![Some(0), Some(9)],
        vec![Some(1), Some(2)],
    ] {
        let mut a = Assignment::new(values);
        let before = a.clone();
        assert!(p.apply(&mut a, &Move::Composite(vec![])).is_err());
        assert_eq!(a, before);
        let mixed = MixedProblem::new(p.clone(), ListProblem::<i32>::new(vec![], vec![]).unwrap());
        let mut a = MixedAssignment::new(a, ListAssignment::new(vec![]));
        let before = a.clone();
        assert!(
            mixed
                .apply(
                    &mut a,
                    &MixedMove {
                        basic: Move::Composite(vec![]),
                        lists: ListMove::Composite(vec![])
                    }
                )
                .is_err()
        );
        assert_eq!(a, before);
    }
}
#[test]
fn final_touched_validation_preserves_error_order_and_rolls_back_composites() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
        Variable::new(vec![2, 3]).with_initial(2),
    ])
    .unwrap();
    let net = ConstraintNetwork::<_, SimpleScore>::new(3);
    let mut d = ScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let initial = d.assignment().clone();
    for movement in [
        Move::Change {
            variable: 0,
            value: Some(9),
        },
        Move::Change {
            variable: 1,
            value: Some(1),
        },
        Move::Swap { left: 2, right: 0 },
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 9,
                value: None,
            },
        ]),
    ] {
        assert!(d.apply(&movement).is_err());
        assert_eq!(d.assignment(), &initial);
    }
    let mut a = initial.clone();
    let bad = Move::Composite(vec![
        Move::Change {
            variable: 2,
            value: Some(8),
        },
        Move::Change {
            variable: 0,
            value: Some(9),
        },
    ]);
    let error = match p.apply(&mut a, &bad) {
        Err(e) => e,
        Ok(_) => panic!("invalid final assignment accepted"),
    };
    assert!(matches!(
        error.without_range_report(),
        ModelError::ValueOutsideDomain { variable: 0 }
    ));
    assert_eq!(a, initial);
    let restored = Move::Composite(vec![
        Move::Change {
            variable: 1,
            value: Some(9),
        },
        Move::Change {
            variable: 0,
            value: Some(9),
        },
        Move::Composite(vec![
            Move::Change {
                variable: 1,
                value: Some(0),
            },
            Move::Change {
                variable: 0,
                value: Some(1),
            },
        ]),
    ]);
    {
        let trial = d.apply(&restored).unwrap();
        assert_eq!(trial.assignment().values(), [Some(1), Some(0), Some(2)]);
    }
    assert_eq!(d.assignment(), &initial);
    d.apply(&restored).unwrap().commit();
    assert_eq!(d.assignment().values(), [Some(1), Some(0), Some(2)]);
    d.apply(&Move::Change {
        variable: 0,
        value: None,
    })
    .unwrap()
    .commit();
    assert_eq!(d.assignment().values()[0], None);
}
