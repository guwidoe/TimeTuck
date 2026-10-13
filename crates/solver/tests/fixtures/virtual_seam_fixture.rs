//! Pinned Community TEST fixture/helper adaptation, Apache-2.0.
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};
use timetuck::*;
#[derive(Debug)]
pub struct Value {
    pub code: u8,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self, state)
    }
}
pub type Token = Arc<Value>;
pub fn values(n: u8) -> Vec<Token> {
    (1..=n).map(|code| Arc::new(Value { code })).collect()
}
pub fn row(values: &[Token], ids: &[u8]) -> Vec<Token> {
    ids.iter()
        .map(|&id| values[usize::from(id - 1)].clone())
        .collect()
}
pub fn problem(values: &[Token], rows: &[&[u8]], prefixes: &[usize]) -> ListProblem<Token> {
    ListProblem::new_indexed(
        values.to_vec(),
        rows.iter()
            .zip(prefixes)
            .map(|(ids, &prefix)| ListEntity::new(row(values, ids)).with_pinned_prefix(prefix))
            .collect(),
    )
    .unwrap()
}
pub fn from_edges(
    problem: &ListProblem<Token>,
    assignment: &ListAssignment<Token>,
    removed: &[Token],
    added: &[Token],
) -> VirtualSeam3OptPlan<Token> {
    assert_eq!(removed.len(), added.len());
    assert_eq!(removed.len() % 2, 0);
    let positions = removed
        .iter()
        .map(|v| {
            assignment
                .lists()
                .iter()
                .enumerate()
                .find_map(|(entity, list)| {
                    list.iter()
                        .position(|x| Arc::ptr_eq(x, v))
                        .map(|index| ListPosition { entity, index })
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let removed_pairs = positions
        .chunks_exact(2)
        .map(|p| (p[0], p[1]))
        .collect::<Vec<_>>();
    let added_pairs = added
        .chunks_exact(2)
        .map(|p| {
            (
                removed.iter().position(|v| Arc::ptr_eq(v, &p[0])).unwrap(),
                removed.iter().position(|v| Arc::ptr_eq(v, &p[1])).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    VirtualSeam3OptPlan::prepare(problem, assignment, &removed_pairs, &added_pairs).unwrap()
}
pub fn schema() -> PlanningSchema {
    PlanningSchema::new::<ListAssignment<Token>>(vec![PlanningEntityDefinition::of::<
        ListEntity<Token>,
    >(vec![
        PlanningVariableDefinition::list::<Token>("values", false),
    ])])
    .unwrap()
}
pub fn execute(
    p: &ListProblem<Token>,
    a: &mut ListAssignment<Token>,
    plan: &VirtualSeam3OptPlan<Token>,
    field: PlanningVariableRef<'_>,
    expected: &[Vec<Token>],
    ranges: &[(usize, usize, usize, usize)],
) {
    let original = a.clone();
    let recorder = ListChangeRecorder::<usize, Token>::new(field).unwrap();
    let mut events = Vec::new();
    assert!(plan.is_doable(p, a));
    plan.apply_notifying(p, a, |event, observed| {
        let notification = match event {
            VirtualSeamListChange::Before { entity, from, to } => {
                recorder.before(entity, from, to, &observed.lists()[entity])
            }
            VirtualSeamListChange::After { entity, from, to } => {
                recorder.after(entity, from, to, &observed.lists()[entity])
            }
        }
        .unwrap()
        .unwrap();
        events.push(notification);
    })
    .unwrap()
    .commit();
    assert_eq!(a.lists(), expected);
    let expected_events = ranges
        .iter()
        .map(
            |&(entity, from, before, _)| ListChangeNotification::Before {
                variable: field,
                entity,
                from,
                to: before,
            },
        )
        .chain(
            ranges
                .iter()
                .map(|&(entity, from, _, after)| ListChangeNotification::After {
                    variable: field,
                    entity,
                    from,
                    to: after,
                }),
        )
        .collect::<Vec<_>>();
    assert_eq!(events, expected_events);
    let recorded = recorder.recorded_before().unwrap();
    assert_eq!(recorded.len(), 2);
    for (edit, &(entity, from, before, _)) in recorded.iter().zip(ranges) {
        assert_eq!(edit.entity, entity);
        assert_eq!(edit.from, from);
        assert_eq!(edit.original_to, before);
        assert_eq!(edit.old_values.as_ref(), &original.lists()[entity]);
    }
}
