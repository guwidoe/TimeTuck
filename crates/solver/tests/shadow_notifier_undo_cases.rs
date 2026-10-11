// Adapted from Timefold Community MoveDirectorUndoShadowVariableTest.java,
// undoCases/undoRestoresListShadowVariablesAndScore, and the test fixture
// TestdataListWithShadowHistoryConstraintProvider.defineConstraints.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::RefCell, collections::BTreeMap};
use timetuck::*;
struct Model;
struct Entity;
type Lists = ListAssignment<&'static str>;
type Shadows = BTreeMap<
    &'static str,
    (
        Option<usize>,
        Option<usize>,
        Option<&'static str>,
        Option<&'static str>,
    ),
>;
fn shadow_state(assignment: &Lists, values: &[&'static str]) -> Shadows {
    values
        .iter()
        .map(|&value| {
            let item = assignment.element(&value);
            (
                value,
                (
                    item.map(|v| v.position.entity),
                    item.map(|v| v.position.index),
                    item.and_then(|v| v.previous.copied()),
                    item.and_then(|v| v.next.copied()),
                ),
            )
        })
        .collect()
}
fn calculator<'a>(
    schema: &'a PlanningSchema,
    problem: &'a ListProblem<&'static str>,
    observed: &'a RefCell<Shadows>,
) -> ShadowCalculator<'a, 'a, usize, Lists, Lists, SimpleScore, &'static str> {
    let fields = schema.entity::<Entity>().unwrap();
    let rows_field = fields.variable("rows").unwrap();
    let mut builder = ShadowSessionBuilder::<usize, Lists>::new(schema);
    let rows: Vec<_> = (0..problem.entities().len())
        .map(|i| {
            builder
                .list_input(rows_field, i, move |a| a.lists()[i].clone())
                .unwrap()
        })
        .collect();
    let output = builder
        .derive(
            fields.variable("snapshot").unwrap(),
            usize::MAX,
            move |_, inputs| {
                Some(ListAssignment::new(
                    inputs
                        .for_field::<Vec<&'static str>>(rows_field)
                        .unwrap()
                        .into_iter()
                        .map(|row| row.unwrap().clone())
                        .collect(),
                ))
            },
        )
        .unwrap();
    builder
        .set_dependencies(
            &output,
            &rows.iter().map(ShadowNode::erase).collect::<Vec<_>>(),
        )
        .unwrap();
    let manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            let snapshot = session.read(&output).unwrap().value.unwrap().clone();
            *observed.borrow_mut() = shadow_state(&snapshot, problem.values());
            Ok(snapshot)
        },
        |_, _| Ok(()),
        |_, snapshot| {
            Ok(SimpleScore::new(
                -snapshot
                    .lists()
                    .iter()
                    .map(|row| (row.len() as i64).pow(2))
                    .sum::<i64>(),
            ))
        },
    );
    ShadowCalculator::new(manager, || ListAssignment::new(vec![])).with_change_notifier(
        move |_, _, changed, session| {
            for &index in changed {
                session.mark_dirty(&rows[index]).map_err(|_| "dirty row")?;
            }
            Ok(())
        },
    )
}
fn movement(problem: &ListProblem<&'static str>, case: usize) -> ListMove<&'static str> {
    let initial = problem.initial_assignment();
    if case == 0 {
        return problem
            .gather_move(
                &initial,
                &["v1", "v3"],
                Some(ListPosition {
                    entity: 0,
                    index: 3,
                }),
            )
            .unwrap();
    }
    let mut builder = ListMoveBuilder::new(problem, &initial).unwrap();
    let pos = |entity, index| ListPosition { entity, index };
    let span = |entity, start, end| ListRange { entity, start, end };
    match case {
        1 => {
            builder
                .relocate_span(span(0, 0, 2), pos(1, 0), false)
                .unwrap();
            builder
                .relocate_span(span(1, 0, 1), pos(0, 0), false)
                .unwrap();
        }
        2 => {
            builder.replace(pos(0, 1), pos(1, 0)).unwrap();
        }
        3 => {
            builder
                .swap_spans(span(0, 0, 1), span(0, 2, 4), false)
                .unwrap();
        }
        4 => {
            builder
                .swap_spans(span(0, 0, 1), span(1, 0, 2), false)
                .unwrap();
        }
        5 => {
            builder
                .relocate_span(span(0, 3, 5), pos(0, 1), true)
                .unwrap();
        }
        6 => {
            let removed = initial.lists()[0][1..3].to_vec();
            builder.unassign(span(0, 1, 3)).unwrap();
            builder.assign_many(removed, pos(0, 1)).unwrap();
        }
        _ => unreachable!(),
    }
    builder.finish()
}
#[test]
fn undo_restores_list_shadow_variables_and_score() {
    let cases = [
        (
            "massMoveWithinTheSameEntity",
            vec![vec!["v1", "v2", "v3", "v4"]],
        ),
        (
            "twoOppositeCrossEntityMovesSharingIndexZero",
            vec![vec!["v1", "v2", "v3"], vec!["v4"]],
        ),
        (
            "crossEntityReplaceValue",
            vec![vec!["v1", "v2"], vec!["v3"]],
        ),
        (
            "swapUnequalSpansWithinOneList",
            vec![vec!["v1", "v2", "v3", "v4", "v5"]],
        ),
        (
            "swapUnequalSpansBetweenTwoLists",
            vec![vec!["v1", "v2", "v3"], vec!["v4", "v5"]],
        ),
        (
            "reversingBackwardSubListMove",
            vec![vec!["v1", "v2", "v3", "v4", "v5"]],
        ),
        (
            "unassignThenReassignTheSameIndexRange",
            vec![vec!["v1", "v2", "v3", "v4"]],
        ),
    ];
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::list::<&'static str>("rows", false),
        PlanningVariableDefinition::shadow::<Lists>("snapshot"),
    ])])
    .unwrap();
    for (case, (name, rows)) in cases.into_iter().enumerate() {
        let values = rows.iter().flatten().copied().collect::<Vec<_>>();
        let problem =
            ListProblem::new(values, rows.into_iter().map(ListEntity::new).collect()).unwrap();
        let observed = RefCell::new(Shadows::new());
        let factory = || calculator(&schema, &problem, &observed);
        let mut director = ListCalculatorScoreDirector::new(
            &problem,
            problem.initial_assignment(),
            factory,
            false,
        )
        .unwrap();
        let expected_lists = director.assignment().clone();
        let expected_shadows = observed.borrow().clone();
        {
            let _trial = director.apply(&movement(&problem, case)).unwrap();
        } // Native ephemeral move rollback sends inverse notifications.
        assert_eq!(director.assignment(), &expected_lists, "{name}: lists");
        assert_eq!(
            *observed.borrow(),
            expected_shadows,
            "{name}: owner/index/previous/next"
        );
        // Native equivalent of the source's incremental-vs-from-scratch check.
        director.assert_consistent().unwrap();
    }
}
