// Adapted from pinned Community DefaultSolverTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn model() -> ListProblem<&'static str> {
    ListProblem::new(
        vec!["Generated Value 0", "Generated Value 1"],
        (0..2)
            .map(|i| {
                ListEntity::new(vec![]).with_range_diagnostics(
                    format!("Generated Entity {i}"),
                    "valueList",
                    |v: &&str| v.to_string(),
                )
            })
            .collect(),
    )
    .unwrap()
}
fn invalid_assignment(index: usize) -> ListMove<&'static str> {
    ListMove::Assign {
        value: "bad value",
        destination: ListPosition { entity: 0, index },
    }
}
fn assert_message(error: impl std::fmt::Display) {
    assert!(error.to_string().contains("entity Generated Entity 0, variable valueList: value bad value outside allowed range [Generated Value 0, Generated Value 1]"), "{error}");
}
#[test]
fn fail_custom_phase_value_range_assertion() {
    let p = model();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
    let initial = greedy_list_construction(&p, &n, Default::default()).unwrap();
    assert!(initial.complete);
    let error = list_custom_phase(&p, &n, initial.assignment, Default::default(), |c| {
        c.execute(&invalid_assignment(0))?;
        Ok(())
    })
    .unwrap_err();
    assert_message(error);
}
#[test]
fn fail_local_search_value_range_assertion() {
    let p = model();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 2);
    let initial = greedy_list_construction(&p, &n, Default::default()).unwrap();
    assert!(initial.complete);
    let mut provider = |_: &ListProblem<&'static str>, a: &ListAssignment<&'static str>, _: u64| {
        vec![invalid_assignment(a.lists()[0].len())]
    };
    let error = list_local_search_from_with_provider(
        &p,
        &n,
        initial.assignment,
        LocalSearchConfig {
            max_steps: 1,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap_err();
    assert_message(error);
}
