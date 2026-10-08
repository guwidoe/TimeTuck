//! Original multi-variable entity construction regressions; no upstream credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn value(a: &[Option<i32>]) -> i64 {
    a.chunks(2)
        .map(|r| match r {
            [Some(1), Some(1)] => 4,
            [Some(0), None] | [Some(0), Some(0)] => 1,
            _ => 0,
        })
        .sum()
}
#[derive(Default)]
struct Calc(i64);
impl IncrementalCalculator<Assignment<i32>> for Calc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &Assignment<i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.0 = value(a.values());
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &Assignment<i32>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i32>,
        _: &Vec<usize>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a, r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
impl IncrementalCalculator<MixedAssignment<i32, i32>> for Calc {
    type Score = SimpleScore;
    type Justification = ();
    type Change = MixedCalculatorChange;
    fn reset(
        &mut self,
        a: &MixedAssignment<i32, i32>,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a.basic(), r)
    }
    fn before_change(
        &mut self,
        _: &MixedAssignment<i32, i32>,
        _: &MixedCalculatorChange,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &MixedAssignment<i32, i32>,
        _: &MixedCalculatorChange,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(a.basic(), r)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(self.0)
    }
}
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]); 4]).unwrap()
}
fn network() -> ConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("pairs", vec![0, 1, 2, 3], |v| {
        SimpleScore::new(value(&v.iter().map(|v| v.copied()).collect::<Vec<_>>()))
    })
    .unwrap();
    n
}
fn rows() -> Vec<Vec<usize>> {
    vec![vec![0, 1], vec![2, 3]]
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits::default(),
        phases: vec![SolverPhase::Construction(ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        })],
    }
}
fn graph() -> Result<(StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>), GraphError> {
    let mut b = StreamGraphBuilder::new();
    let source = b.source();
    let schema = BasicEntityRows::new(4, rows()).unwrap();
    let entities = basic_entity_stream(&mut b, &source, &schema, true)?;
    b.score("pairs", &entities, |(_, r)| SimpleScore::new(value(r)))?;
    Ok((b.build(), source))
}
#[test]
fn joint_rows_choose_combinations_that_sequential_columns_miss_on_all_six_paths() {
    for placement in [
        EntityConstructionPlacement::SeparateVariables,
        EntityConstructionPlacement::JointVariables,
    ] {
        let p = basic();
        let n = network();
        let joint = placement == EntityConstructionPlacement::JointVariables;
        macro_rules! basic_run {
            ($solve:ident,$resource:expr) => {{
                let mut provider =
                    EntityConstruction::new(&p, rows(), placement, OptionalConstruction::Preserve)
                        .unwrap();
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let r = $solve(&p, $resource, plan(), ext).unwrap();
                assert!(r.complete);
                assert_eq!(r.score, Some(SimpleScore::new(if joint { 8 } else { 2 })));
                assert_eq!(r.assignment.values(), &[Some(i32::from(joint)); 4]);
                assert_eq!(r.work.construction_steps, if joint { 2 } else { 4 });
                assert_eq!(r.work.move_evaluations, 8);
            }};
        }
        basic_run!(solve_with_extensions, &n);
        basic_run!(solve_calculator_with_extensions, &Calc::default);
        basic_run!(solve_stream_with_extensions, graph);
        let p = MixedProblem::new(
            p,
            ListProblem::new(vec![7], vec![ListEntity::new(vec![7])]).unwrap(),
        );
        let mut n = MixedConstraintNetwork::new(4, 1, 1);
        n.add_constraint("pairs", (0..4).map(MixedScope::Basic).collect(), |facts| {
            SimpleScore::new(value(
                &facts
                    .iter()
                    .map(|v| {
                        let MixedFact::Basic(v) = v else { panic!() };
                        v.copied()
                    })
                    .collect::<Vec<_>>(),
            ))
        })
        .unwrap();
        let plan = MixedSolverSequence {
            limits: SolverLimits::default(),
            phases: vec![MixedSolverPhase::Construction(MixedConstructionConfig {
                construction: ConstructionConfig {
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            })],
        };
        macro_rules! mixed_run {
            ($solve:ident,$resource:expr) => {{
                let mut provider = EntityConstruction::new(
                    p.basic(),
                    rows(),
                    placement,
                    OptionalConstruction::Preserve,
                )
                .unwrap();
                let mut ext = SolverExtensions::default();
                ext.move_providers.bind(0, &mut provider);
                let r = $solve(&p, $resource, plan.clone(), ext).unwrap();
                assert!(r.complete);
                assert_eq!(r.score, Some(SimpleScore::new(if joint { 8 } else { 2 })));
                assert_eq!(r.assignment.basic().values(), &[Some(i32::from(joint)); 4]);
                assert_eq!(r.assignment.lists().lists(), &[vec![7]]);
                assert_eq!(r.work.construction_steps, if joint { 2 } else { 4 });
                assert_eq!(r.work.move_evaluations, 8);
            }};
        }
        mixed_run!(solve_mixed_with_extensions, &n);
        mixed_run!(solve_mixed_calculator_with_extensions, &Calc::default);
        mixed_run!(solve_mixed_stream_with_extensions, || {
            let mut b = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut b);
            let schema = BasicEntityRows::new(4, rows()).unwrap();
            let entities = basic_entity_stream(&mut b, &source.basic, &schema, true)?;
            b.score("pairs", &entities, |(_, r)| SimpleScore::new(value(r)))?;
            Ok((b.build(), source))
        });
    }
}
#[test]
fn row_order_optional_choices_partial_assignments_and_pins_are_preserved() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(2),
        Variable::new(vec![3, 4]),
        Variable::new(vec![5, 6]).allowing_unassigned(),
        Variable::new(vec![7])
            .allowing_unassigned()
            .with_pinned(true),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let mut provider = EntityConstruction::new(
        &p,
        vec![vec![3], vec![2, 0, 1]],
        EntityConstructionPlacement::JointVariables,
        OptionalConstruction::EvaluateUnassigned,
    )
    .unwrap();
    let moves = provider.moves(&p, &a, 0).collect::<Vec<_>>();
    let states = moves
        .iter()
        .map(|m| {
            let mut a = a.clone();
            p.apply(&mut a, m).unwrap().commit();
            a.values().to_vec()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        states,
        vec![
            vec![Some(2), Some(3), Some(5), None],
            vec![Some(2), Some(4), Some(5), None],
            vec![Some(2), Some(3), Some(6), None],
            vec![Some(2), Some(4), Some(6), None],
            vec![Some(2), Some(3), None, None],
            vec![Some(2), Some(4), None, None]
        ]
    );
    assert_eq!(provider.moves(&p, &a, 0).collect::<Vec<_>>(), moves);
    assert!(provider.moves(&p, &a, 1).next().is_none());
    provider.reset();
    assert_eq!(provider.moves(&p, &a, 0).count(), 6);
    for (policy, count) in [
        (OptionalConstruction::Preserve, 2),
        (OptionalConstruction::NonWorsening, 6),
    ] {
        let mut provider = EntityConstruction::new(
            &p,
            vec![vec![2, 0, 1, 3]],
            EntityConstructionPlacement::JointVariables,
            policy,
        )
        .unwrap();
        assert_eq!(provider.moves(&p, &a, 0).count(), count);
    }
}
#[test]
fn atomic_interruption_retries_the_same_row_and_zero_budget_does_not_advance_it() {
    let p = basic();
    let n = network();
    let mut provider = EntityConstruction::new(
        &p,
        rows(),
        EntityConstructionPlacement::JointVariables,
        OptionalConstruction::Preserve,
    )
    .unwrap();
    let c = ProviderConstructionConfig {
        max_move_evaluations: Some(2),
        interruption: ConstructionInterruption::DiscardIncompleteStep,
        ..Default::default()
    };
    let r =
        construction_from_with_provider(&p, &n, p.initial_assignment(), c, &mut provider).unwrap();
    assert_eq!(r.assignment.values(), &[None; 4]);
    assert_eq!(r.completed_steps, 0);
    let r = construction_from_with_provider(
        &p,
        &n,
        r.assignment,
        ProviderConstructionConfig {
            max_move_evaluations: Some(0),
            ..c
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.completed_steps, 0);
    let r = construction_from_with_provider(
        &p,
        &n,
        r.assignment,
        ProviderConstructionConfig {
            max_steps: Some(1),
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1), Some(1), None, None]);
    assert_eq!(r.completed_steps, 1);
    let r = construction_from_with_provider(
        &p,
        &n,
        r.assignment,
        ProviderConstructionConfig::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), &[Some(1); 4]);
    assert_eq!(r.completed_steps, 1);
}
#[test]
fn validates_schema_and_problem_identity_before_enumerating() {
    let p = basic();
    let a = p.initial_assignment();
    for rows in [
        vec![vec![]],
        vec![vec![4]],
        vec![vec![0, 1], vec![1, 2]],
        vec![vec![0, 0]],
    ] {
        assert!(
            EntityConstruction::new(
                &p,
                rows,
                EntityConstructionPlacement::JointVariables,
                OptionalConstruction::Preserve
            )
            .is_err()
        );
    }
    let mut empty = EntityConstruction::new(
        &p,
        vec![],
        EntityConstructionPlacement::JointVariables,
        OptionalConstruction::Preserve,
    )
    .unwrap();
    assert!(empty.moves(&p, &a, 0).next().is_none());
    let other = basic();
    let mut provider = EntityConstruction::new(
        &p,
        rows(),
        EntityConstructionPlacement::JointVariables,
        OptionalConstruction::Preserve,
    )
    .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            provider.moves(&other, &a, 0).next();
        }))
        .is_err()
    );
    assert!(provider.moves(&p, &a, 0).next().is_some());
}
#[derive(Debug)]
struct Counted(u8, Rc<Cell<usize>>);
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Counted {}
#[test]
fn huge_joint_products_clone_only_the_requested_prefix() {
    let count = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..32)
            .map(|_| Variable::new(vec![Counted(0, count.clone()), Counted(1, count.clone())]))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut provider = EntityConstruction::new(
        &p,
        vec![(0..32).collect()],
        EntityConstructionPlacement::JointVariables,
        OptionalConstruction::Preserve,
    )
    .unwrap();
    assert_eq!(count.get(), 0);
    let mut iter = provider.moves(&p, &a, 0);
    assert!(iter.next().is_some());
    assert!(iter.next().is_some());
    assert!(
        count.get() < 200,
        "{} clones for two of 2^32 combinations",
        count.get()
    );
}

#[test]
fn mixed_component_order_and_solve_wide_limits_apply_to_joint_rows() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![2]); 4]).unwrap(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![]); 2]).unwrap(),
    );
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        for limit in [1, 4] {
            let condition = SearchTermination::Condition(SearchLimit::ConstructionSteps(limit));
            let plan = MixedSolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![MixedSolverPhase::Construction(
                    MixedConstructionConfig::default(),
                )],
            };
            macro_rules! run {
                ($solve:ident,$resource:expr) => {{
                    let basic = EntityConstruction::new(
                        p.basic(),
                        rows(),
                        EntityConstructionPlacement::JointVariables,
                        OptionalConstruction::Preserve,
                    )
                    .unwrap();
                    let lists = SortedListConstruction::new(
                        p.lists(),
                        ConstructionPlacement::Sequential,
                        OptionalConstruction::Preserve,
                        |a, b| a.cmp(&b),
                        |a, b| a.cmp(b),
                    );
                    let mut provider = MixedConstructionProvider::new(&p, basic, lists, order);
                    let mut ext = SolverExtensions::default();
                    ext.move_providers.bind(0, &mut provider);
                    let r = $solve(&p, $resource, plan.clone(), ext).unwrap();
                    assert_eq!(r.work.construction_steps, limit);
                    assert_eq!(r.termination, SolverTermination::Condition);
                    let basic = r.assignment.basic().values().iter().flatten().count();
                    let lists = r
                        .assignment
                        .lists()
                        .lists()
                        .iter()
                        .map(Vec::len)
                        .sum::<usize>();
                    assert_eq!(
                        (basic, lists),
                        if limit == 4 {
                            (4, 2)
                        } else if order == MixedConstructionOrder::BasicThenLists {
                            (2, 0)
                        } else {
                            (0, 1)
                        }
                    );
                    assert_eq!(r.complete, limit == 4);
                }};
            }
            run!(
                solve_mixed_with_extensions,
                &MixedConstraintNetwork::<_, _, SimpleScore>::new(4, 2, 2)
            );
            run!(solve_mixed_calculator_with_extensions, &Calc::default);
            run!(solve_mixed_stream_with_extensions, || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = MixedStreamSources::new(&mut b);
                Ok((b.build(), s))
            });
        }
    }
}

#[test]
fn joint_optional_columns_can_stay_empty_without_blocking_required_initialization() {
    for empty in [false, true] {
        for policy in [
            OptionalConstruction::NonWorsening,
            OptionalConstruction::EvaluateUnassigned,
        ] {
            let p = FiniteProblem::new(vec![
                Variable::new(vec![1]),
                Variable::new(if empty { vec![] } else { vec![2] }).allowing_unassigned(),
                Variable::new(vec![3]).allowing_unassigned(),
            ])
            .unwrap();
            let mut n = ConstraintNetwork::new(3);
            n.add_constraint("optional", vec![1, 2], |v| {
                SimpleScore::new(-100 * i64::from(v[0].is_some()) + 10 * i64::from(v[1].is_some()))
            })
            .unwrap();
            let mut provider = EntityConstruction::new(
                &p,
                vec![vec![0, 1, 2]],
                EntityConstructionPlacement::JointVariables,
                policy,
            )
            .unwrap();
            let r = construction_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                ProviderConstructionConfig {
                    optional: policy,
                    assert_scores: true,
                    ..Default::default()
                },
                &mut provider,
            )
            .unwrap();
            assert!(r.complete);
            // The existing engine scores a final no-change probe after the row
            // queue ends when EvaluateUnassigned still sees optional slots.
            assert_eq!(
                r.completed_steps,
                if policy == OptionalConstruction::EvaluateUnassigned {
                    2
                } else {
                    1
                }
            );
            assert_eq!(r.assignment.values(), &[Some(1), None, Some(3)]);
            assert_eq!(r.score, Some(SimpleScore::new(10)));
        }
    }
}
