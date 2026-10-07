// Adapted from pinned SingleConstraintAssertionTest and shadow/dependency fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Task {
    id: &'static str,
    previous: Option<&'static str>,
    dependencies: Vec<&'static str>,
    start: Option<i64>,
    end: Option<i64>,
    invalid: Option<bool>,
    assigned: bool,
}
fn task(id: &'static str) -> Task {
    Task {
        id,
        previous: None,
        dependencies: vec![],
        start: None,
        end: None,
        invalid: None,
        assigned: true,
    }
}
fn reference(id: &'static str, variable: &str) -> VerificationShadowRef<&'static str> {
    VerificationShadowRef {
        entity: id,
        variable: variable.into(),
    }
}
fn metadata(tasks: &[Task]) -> Vec<VerificationEntity<&'static str>> {
    tasks
        .iter()
        .map(|t| VerificationEntity {
            id: t.id,
            inconsistent: t.invalid,
            shadows: vec![
                VerificationShadow {
                    variable: "startTime".into(),
                    dependencies: t
                        .dependencies
                        .iter()
                        .copied()
                        .chain(t.previous)
                        .map(|id| reference(id, "endTime"))
                        .collect(),
                },
                VerificationShadow {
                    variable: "endTime".into(),
                    dependencies: vec![reference(t.id, "startTime")],
                },
            ],
        })
        .collect()
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Solution {
    tasks: Vec<Task>,
    list: Vec<&'static str>,
}
fn snapshot(
    tasks: &[Task],
    given: Vec<&'static str>,
    unfiltered: bool,
) -> Result<
    ConstraintAssertion<HardSoftScore, &'static str>,
    VerificationConsistencyError<&'static str>,
> {
    let flags = verification_consistency(&metadata(tasks), given)?;
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<&'static str, (Task, bool)>();
    let visible = b
        .filter(source.port(), move |(t, invalid)| {
            if unfiltered {
                *invalid
            } else {
                t.assigned && !invalid
            }
        })
        .unwrap();
    let c = track_constraint(
        &mut b,
        if unfiltered {
            "Invalid task"
        } else {
            "Finish tasks as early as possible"
        },
        &visible,
        if unfiltered {
            HardSoftScore::new(1, 0)
        } else {
            HardSoftScore::new(0, 1)
        },
        move |(t, _)| {
            if unfiltered {
                -1
            } else {
                -t.end
                    .expect("consistent assigned tasks need supplied end time")
            }
        },
    )
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for t in tasks {
        if let Some(invalid) = flags.is_inconsistent(&t.id) {
            tx.apply(&source, TupleChange::put(t.id, (t.clone(), invalid)))
                .unwrap();
        }
    }
    tx.commit().unwrap();
    Ok(constraint_assertion(&graph, &c, |(t, _), _| t.id).unwrap())
}
fn solution_snapshot(
    solution: &Solution,
    unfiltered: bool,
) -> Result<
    ConstraintAssertion<HardSoftScore, &'static str>,
    VerificationConsistencyError<&'static str>,
> {
    snapshot(
        &solution.tasks,
        solution.tasks.iter().map(|t| t.id).collect(),
        unfiltered,
    )
}
fn check(solution: &Solution, whole: bool, unfiltered: bool, amount: usize, count: bool) {
    let before = solution.clone();
    // Both native entry forms explicitly select all task entities. The solution's
    // supplied list is retained; preparation must not synchronize or repair it.
    let assertion = if whole {
        solution_snapshot(solution, unfiltered)
    } else {
        snapshot(
            &solution.tasks,
            solution.tasks.iter().map(|t| t.id).collect(),
            unfiltered,
        )
    }
    .unwrap();
    assertion
        .check(
            if count {
                ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(amount))
            } else {
                ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly(Decimal::from(
                    amount as i64,
                )))
            },
            None,
        )
        .unwrap();
    assert_eq!(*solution, before);
}
fn cycle() -> Solution {
    let mut dependency = task("dependency");
    let mut dependent = task("dependent");
    dependent.dependencies.push("dependency");
    dependency.previous = Some("dependent");
    Solution {
        tasks: vec![dependency, dependent],
        list: vec!["dependent", "dependency"],
    }
}
fn missing_flag(whole: bool) {
    let mut solution = cycle();
    check(&solution, whole, false, 0, true);
    assert!(solution.tasks.iter().all(|t| t.invalid.is_none()));
    solution.list = vec!["dependency", "dependent"];
    solution.tasks[0].previous = None;
    solution.tasks[1].previous = Some("dependency");
    solution.tasks[1].start = Some(0);
    solution.tasks[1].end = Some(60);
    solution.tasks[0].start = Some(60);
    solution.tasks[0].end = Some(120);
    check(&solution, whole, false, 180, false);
    assert!(solution.tasks.iter().all(|t| t.invalid.is_none()));
}
fn explicit_flag(whole: bool) {
    let mut solution = cycle();
    if whole {
        solution.list.clear();
    }
    solution.tasks[0].invalid = Some(false);
    solution.tasks[0].start = Some(60);
    solution.tasks[0].end = Some(120);
    check(&solution, whole, false, 120, false);
}
fn unfiltered(whole: bool) {
    let mut single = task("single");
    single.assigned = whole;
    single.invalid = Some(false);
    let mut solution = Solution {
        tasks: vec![single],
        list: vec![],
    };
    check(&solution, whole, true, 0, true);
    solution.tasks[0].invalid = Some(true);
    check(&solution, whole, true, 1, true);
}
#[test]
fn should_update_internal_consistency_state_if_inconsistent_is_null() {
    missing_flag(false);
}
#[test]
fn should_not_update_internal_consistency_state_if_inconsistent_is_specified() {
    explicit_flag(false);
    unfiltered(false);
}
#[test]
fn should_update_internal_consistency_state_if_inconsistent_is_null_given_solution() {
    missing_flag(true);
}
#[test]
fn should_update_internal_consistency_state_if_inconsistent_is_specified_given_solution() {
    explicit_flag(true);
}
#[test]
fn should_update_internal_consistency_state_if_inconsistent_is_specified_given_solution_unfiltered()
{
    unfiltered(true);
}
#[test]
fn fails_if_non_given_entities_are_referenced() {
    let previous = task("previous");
    let mut dependency = task("dependency");
    dependency.previous = Some("previous");
    let mut current = task("current");
    current.previous = Some("dependency");
    current.dependencies.push("dependency");
    let facts = vec![current, dependency, previous];
    let err = snapshot(&facts, vec!["current"], false).unwrap_err();
    let text = err.to_string();
    assert!(text.contains("Found referenced entities that were not given"));
    let VerificationConsistencyError::MissingEntities(refs) = err else {
        panic!("wrong error")
    };
    assert_eq!(
        refs,
        vec![
            MissingVerificationEntity {
                from: reference("current", "startTime"),
                reference: reference("dependency", "endTime")
            },
            MissingVerificationEntity {
                from: reference("dependency", "startTime"),
                reference: reference("previous", "endTime")
            },
        ]
    );
    for name in ["current", "dependency", "previous", "startTime"] {
        assert!(text.contains(name));
    }
}
