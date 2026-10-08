//! Original validation-boundary and comparison-work regressions; no upstream credit.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    foreign: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        if self.id >= 8 && other.id >= 8 {
            self.foreign.set(self.foreign.get() + 1);
        }
        self.id == other.id
    }
}
impl Eq for Value {}
fn fixture() -> (ListProblem<Value>, Rc<Cell<usize>>) {
    let counter = Rc::new(Cell::new(0));
    let make = |id| Value {
        id,
        foreign: counter.clone(),
    };
    (
        ListProblem::new(
            (0..64).map(make).collect(),
            (0..16)
                .map(|e| ListEntity::new((e * 4..e * 4 + 4).map(make).collect()))
                .collect(),
        )
        .unwrap(),
        counter,
    )
}
fn movement<V>() -> ListMove<V> {
    ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn score(a: &ListAssignment<Value>) -> SimpleScore {
    SimpleScore::new(
        a.lists()
            .iter()
            .enumerate()
            .flat_map(|(e, list)| {
                list.iter()
                    .enumerate()
                    .map(move |(i, v)| ((e + 1) * (i + 1) * (v.id + 1)) as i64)
            })
            .sum(),
    )
}
#[derive(Default)]
struct ListCalc(SimpleScore);
impl IncrementalCalculator<ListAssignment<Value>> for ListCalc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &ListAssignment<Value>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = score(a);
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &ListAssignment<Value>,
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
struct MixedCalc(ListCalc);
impl IncrementalCalculator<MixedAssignment<i32, Value>> for MixedCalc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = MixedCalculatorChange;
    fn reset(
        &mut self,
        a: &MixedAssignment<i32, Value>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0.reset(a.lists(), r)
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<i32, Value>,
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
fn all_six_list_and_mixed_backends_avoid_comparing_unrelated_values() {
    let (p, count) = fixture();
    let initial = p.initial_assignment();
    let old = score(&initial);
    let mut expected = initial.clone();
    p.apply(&mut expected, &movement()).unwrap().commit();
    let new = score(&expected);
    assert_ne!(old, new);
    let net = ListConstraintNetwork::<_, SimpleScore>::new(16, 64);
    let mut d = ListScoreDirector::new(&p, &net, initial.clone()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement()).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().lists()[0][0].id, 4);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().lists()[0][0].id, 0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let src = ListStreamSources::new(&mut builder);
    let mut d = ListStreamScoreDirector::new(&p, builder.build(), src, initial.clone()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement()).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().lists()[0][0].id, 4);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().lists()[0][0].id, 0);
    let mut d = ListCalculatorScoreDirector::new(&p, initial, ListCalc::default, false).unwrap();
    count.set(0);
    {
        let trial = d.apply(&movement()).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.score(), new);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.score().unwrap(), old);
    d.apply(&movement()).unwrap().commit();
    assert_eq!(d.score().unwrap(), new);
    // Reset is an external-input boundary and still performs full validation.
    let mut bad = p.initial_assignment();
    let mut rows = bad.lists().to_vec();
    rows[15][0] = rows[14][0].clone();
    bad = ListAssignment::new(rows);
    assert!(d.reset(bad).is_err());
    assert_eq!(d.score().unwrap(), new);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
        p,
    );
    let mixed = MixedMove {
        basic: Move::Composite(vec![]),
        lists: movement(),
    };
    let net = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 16, 64);
    let mut d = MixedScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&mixed).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().lists().lists()[0][0].id, 4);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().lists().lists()[0][0].id, 0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let src = MixedStreamSources::new(&mut builder);
    let mut d =
        MixedStreamScoreDirector::new(&p, builder.build(), src, p.initial_assignment()).unwrap();
    count.set(0);
    {
        let trial = d.apply(&mixed).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.assignment().lists().lists()[0][0].id, 4);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.assignment().lists().lists()[0][0].id, 0);
    let mut d =
        MixedCalculatorScoreDirector::new(&p, p.initial_assignment(), MixedCalc::default, false)
            .unwrap();
    count.set(0);
    {
        let trial = d.apply(&mixed).unwrap();
        assert_eq!(count.get(), 0);
        assert_eq!(trial.score(), new);
    }
    assert_eq!(count.get(), 0);
    assert_eq!(d.score().unwrap(), old);
    d.apply(&mixed).unwrap().commit();
    assert_eq!(d.score().unwrap(), new);
}
#[test]
fn public_moves_validate_external_input_even_for_empty_composites() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1]).with_pinned_prefix(1),
            ListEntity::new(vec![2, 3]),
        ],
    )
    .unwrap();
    for rows in [
        vec![vec![0, 1]],
        vec![vec![0, 1], vec![2, 2]],
        vec![vec![0, 1], vec![2, 99]],
        vec![vec![1, 0], vec![2, 3]],
    ] {
        let mut a = ListAssignment::new(rows);
        let before = a.clone();
        let expected = p.validate(&a).unwrap_err();
        assert!(matches!(p.apply(&mut a,&ListMove::Composite(vec![])),Err(e) if e==expected));
        assert_eq!(a, before);
        let mixed = MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), p.clone());
        let mut a = MixedAssignment::new(Assignment::new(vec![]), a);
        let before = a.clone();
        assert!(
            matches!(mixed.apply(&mut a,&MixedMove{basic:Move::Composite(vec![]),lists:ListMove::Composite(vec![])}),Err(MixedError::Lists(e)) if e==expected)
        );
        assert_eq!(a, before);
    }
}
#[test]
fn final_pins_ranges_and_cross_list_dependency_cycles_remain_enforced() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![2, 3]).with_value_range(vec![1, 2, 3]),
        ],
    )
    .unwrap();
    let net = ListConstraintNetwork::<_, SimpleScore>::new(2, 4);
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let before = d.assignment().clone();
    let pinned = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0,
        },
        right: ListPosition {
            entity: 1,
            index: 1,
        },
    };
    assert!(matches!(
        d.apply(&pinned),
        Err(ListScoringError::Model(ListError::PinChanged { entity: 0 }))
    ));
    assert_eq!(d.assignment(), &before);
    let out_of_range = ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 1,
        },
        right: ListPosition {
            entity: 1,
            index: 1,
        },
    };
    assert!(matches!(
        d.apply(&out_of_range),
        Err(ListScoringError::Model(
            ListError::ValueOutsideEntityRange { entity: 0 }
        ))
    ));
    assert_eq!(d.assignment(), &before);
    // Intermediate pin/range violations are allowed when the composite restores them.
    d.apply(&ListMove::Composite(vec![pinned.clone(), pinned]))
        .unwrap()
        .commit();
    assert_eq!(d.assignment(), &before);
    d.apply(&ListMove::Composite(vec![
        out_of_range.clone(),
        out_of_range,
    ]))
    .unwrap()
    .commit();
    assert_eq!(d.assignment(), &before);
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![1, 0]), ListEntity::new(vec![3, 2])],
    )
    .unwrap()
    .with_dependencies(vec![vec![3], vec![], vec![1], vec![]])
    .unwrap();
    // Reversing both lists adds 1 -> 0 and 3 -> 2, creating the
    // cross-list cycle 0 -> 3 -> 2 -> 1 -> 0. Either reversal alone is valid.
    let net = ListConstraintNetwork::<_, SimpleScore>::new(2, 4);
    let initial = p.initial_assignment();
    p.validate(&initial).unwrap();
    let mut d = ListScoreDirector::new(&p, &net, initial.clone()).unwrap();
    let cyclic = ListMove::Composite(
        (0..2)
            .map(|entity| ListMove::Swap {
                left: ListPosition { entity, index: 0 },
                right: ListPosition { entity, index: 1 },
            })
            .collect(),
    );
    assert!(matches!(
        d.apply(&cyclic),
        Err(ListScoringError::Model(ListError::DependencyCycle { .. }))
    ));
    assert_eq!(d.assignment(), &initial);
}
