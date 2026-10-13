#[path = "fixtures/nullable_solution_weights_fixture.rs"]
mod fixture;
use fixture::*;
use std::cell::Cell;
use timetuck::*;
#[test]
fn nullable_reads_snapshot_weights_before_actual_empty_and_populated_graph_initialization() {
    let reads = Cell::new(0);
    let accessor = SolutionWeightAccessor::new_nullable(layout(false).unwrap(), |s: &Solution| {
        reads.set(reads.get() + 1);
        s.weights.clone()
    })
    .unwrap();
    assert_eq!(reads.get(), 0);
    let mut solution = Solution::default();
    let absent = accessor.read(&solution);
    assert_eq!(reads.get(), 1);
    assert!(absent.weights().iter().next().is_none());
    assert_eq!(
        first_constraint_assertion(&solution, &absent).score(),
        &SimpleScore::ZERO
    );
    solution.entities = vec![
        Entity { value: Some(1) },
        Entity { value: Some(2) },
        Entity { value: None },
    ];
    assert_eq!(
        first_constraint_assertion(&solution, &absent).score(),
        &SimpleScore::new(2)
    );
    solution.weights = Some(
        [("First weight".into(), SimpleScore::new(3))]
            .into_iter()
            .collect(),
    );
    let overridden = accessor.read(&solution);
    solution.weights = None;
    let cleared = accessor.read(&solution);
    assert_eq!(reads.get(), 3);
    assert_eq!(
        first_constraint_assertion(&solution, &overridden).score(),
        &SimpleScore::new(6)
    );
    assert_eq!(
        first_constraint_assertion(&solution, &cleared).score(),
        &SimpleScore::new(2)
    );
    assert_eq!(
        reads.get(),
        3,
        "graph initialization reads owned snapshots, not the live getter"
    );
    let disabled = Solution {
        weights: Some(
            [("First weight".into(), SimpleScore::ZERO)]
                .into_iter()
                .collect(),
        ),
        entities: solution.entities.clone(),
        ..Default::default()
    };
    let zero = accessor.read(&disabled);
    assert_eq!(
        first_constraint_assertion(&disabled, &zero).score(),
        &SimpleScore::ZERO
    );
}
#[test]
fn invalid_layout_or_score_metadata_never_invokes_nullable_readers() {
    let reads = Cell::new(0);
    let build = || -> Result<(), SolutionCompositionError> {
        let descriptor = layout(true)?;
        let accessor = SolutionWeightAccessor::new_nullable(descriptor, |_: &ExtendedSolution| {
            reads.set(reads.get() + 1);
            None::<ConstraintWeightOverrides<SimpleScore>>
        })
        .unwrap();
        accessor.read(&ExtendedSolution);
        Ok(())
    };
    assert!(build().is_err());
    assert_eq!(reads.get(), 0);
    let result = SolutionWeightAccessor::new_nullable(layout(false).unwrap(), |_: &Solution| {
        reads.set(reads.get() + 1);
        None::<ConstraintWeightOverrides<HardSoftScore>>
    });
    assert!(matches!(result, Err(SolutionWeightError::ScoreType { .. })));
    assert_eq!(reads.get(), 0);
}
