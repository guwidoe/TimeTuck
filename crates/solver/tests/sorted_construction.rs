//! Original order, lazy-candidate, pin/range and interruption integration tests.
use std::{cell::Cell, cmp::Ordering, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn desc(a: usize, b: usize) -> Ordering {
    b.cmp(&a)
}
#[test]
fn all_placements_sort_candidates_preserve_ids_and_can_reset_after_step_limit() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("distinct", vec![0, 1, 2], |rows| {
        let values = rows.iter().flatten().collect::<Vec<_>>();
        SimpleScore::new(
            -((values.len() - values.iter().copied().collect::<BTreeSet<_>>().len()) as i64),
        )
    })
    .unwrap();
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let calls = Cell::new(0);
        let mut order = SortedBasicConstruction::new(
            &p,
            placement,
            OptionalConstruction::Preserve,
            |a, b| {
                calls.set(calls.get() + 1);
                desc(a, b)
            },
            |a, b| b.cmp(a),
        );
        let sorted_calls = calls.get();
        let first = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                max_steps: Some(1),
                assert_scores: true,
                ..Default::default()
            },
            &mut order,
        )
        .unwrap();
        assert_eq!(first.assignment.values(), &[None, None, Some(2)]);
        assert_eq!(
            first.termination,
            ProviderConstructionTermination::StepLimit
        );
        order.reset();
        let full = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            &mut order,
        )
        .unwrap();
        assert_eq!(full.assignment.values(), &[Some(0), Some(1), Some(2)]);
        assert_eq!(full.score, Some(SimpleScore::ZERO));
        assert!(full.complete);
        assert_eq!(
            calls.get(),
            sorted_calls,
            "candidate sorting is cached for the phase"
        );
        assert_eq!(p.variables()[0].domain(), &[0, 1, 2]);
    }
}
#[test]
fn rejected_optional_keys_advance_and_pins_ranges_and_prefixes_survive() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
        Variable::new(vec![2, 3]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("avoid optional", vec![0], |v| {
        SimpleScore::new(-i64::from(v[0].is_some()))
    })
    .unwrap();
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let mut order = SortedBasicConstruction::new(
            &p,
            placement,
            OptionalConstruction::NonWorsening,
            |_, _| Ordering::Equal,
            |a, b| a.cmp(b),
        );
        let r = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                assert_scores: true,
                ..Default::default()
            },
            &mut order,
        )
        .unwrap();
        assert_eq!(r.assignment.values(), &[None, Some(9), Some(2)]);
        assert!(r.complete);
        assert_eq!(r.rejected_moves, 0);
    }
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4],
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1])
                .with_pinned_prefix(1)
                .with_value_range(vec![1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![3]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(3, 5);
    for placement in [
        ConstructionPlacement::Sequential,
        ConstructionPlacement::ValueQueue,
        ConstructionPlacement::Pooled,
    ] {
        let mut order = SortedListConstruction::new(
            &p,
            placement,
            OptionalConstruction::NonWorsening,
            desc,
            |a, b| b.cmp(a),
        );
        let r = list_construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                optional: OptionalConstruction::NonWorsening,
                assert_scores: true,
                ..Default::default()
            },
            &mut order,
        )
        .unwrap();
        assert_eq!(r.assignment.lists(), &[vec![0], vec![1, 2], vec![3]]);
        assert!(r.assignment.location(&4).is_none());
        assert_eq!(r.rejected_moves, 0);
        p.validate(&r.assignment).unwrap();
    }
}
#[derive(Debug)]
struct Value {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn phase_sort_is_stable_and_atomic_interruption_retries_the_same_queue_key() {
    let clones = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(vec![Variable::new(
        (0..1000)
            .map(|id| Value {
                id,
                clones: clones.clone(),
            })
            .collect(),
    )])
    .unwrap();
    let mut order = SortedBasicConstruction::new(
        &p,
        ConstructionPlacement::Sequential,
        OptionalConstruction::Preserve,
        |_, _| Ordering::Equal,
        |_, _| Ordering::Equal,
    );
    clones.set(0);
    let a = p.initial_assignment();
    let first = order.moves(&p, &a, 0).next().unwrap();
    assert!(matches!(
        first,
        Move::Change {
            value: Some(Value { id: 0, .. }),
            ..
        }
    ));
    assert_eq!(clones.get(), 1, "unused sorted domain tail stays un-cloned");
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let mut order = SortedBasicConstruction::new(
        &p,
        ConstructionPlacement::Sequential,
        OptionalConstruction::Preserve,
        desc,
        |a, b| b.cmp(a),
    );
    let interrupted = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig {
            max_move_evaluations: Some(1),
            interruption: ConstructionInterruption::DiscardIncompleteStep,
            ..Default::default()
        },
        &mut order,
    )
    .unwrap();
    assert_eq!(interrupted.assignment.values(), &[None, None]);
    assert_eq!(interrupted.completed_steps, 0);
    let resumed = construction_from_with_provider(
        &p,
        &n,
        interrupted.assignment,
        ProviderConstructionConfig {
            max_steps: Some(1),
            ..Default::default()
        },
        &mut order,
    )
    .unwrap();
    assert_eq!(resumed.assignment.values(), &[None, Some(2)]);
}
