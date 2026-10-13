// TEST-derived inheritance declarations/member layout, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::collections::BTreeMap;
use timetuck::*;
pub struct Solution;
pub struct Base;
pub struct Child;
pub fn composition() -> ComposedPlanningSchema {
    PlanningComposition::new(vec![
        PlanningComponent::registered::<Base>(vec![PlanningVariableDefinition::basic::<i32>(
            "value", false,
        )]),
        PlanningComponent::registered::<Child>(vec![PlanningVariableDefinition::basic::<i32>(
            "subValue", false,
        )])
        .with_parent::<Base>(PlanningParentKind::Owned),
    ])
    .unwrap()
    .compile::<Solution>(
        &[PlanningType::of::<Base>(), PlanningType::of::<Child>()],
        PlanningCompositionPolicy::default(),
    )
    .unwrap()
}
pub struct Layout {
    pub base: Vec<usize>,
    pub child: Vec<usize>,
    pub child_sub: Vec<usize>,
    pub entity_count: usize,
}
pub fn layout(base_list: usize, child_list: usize, raw_list: usize) -> Layout {
    // Actual singleton/list/raw members: base singleton0, child singleton1,
    // base list, child list, raw child list; the Object list remains empty.
    let entity_count = 2 + base_list + child_list + raw_list;
    let base = std::iter::once(0)
        .chain(2..2 + base_list)
        .collect::<Vec<_>>();
    let child = std::iter::once(1)
        .chain(2 + base_list..entity_count)
        .collect::<Vec<_>>();
    let child_sub = (entity_count..entity_count + child.len()).collect();
    Layout {
        base,
        child,
        child_sub,
        entity_count,
    }
}
pub fn problem(layout: &Layout) -> FiniteProblem<i32> {
    FiniteProblem::new(
        (0..layout.entity_count + layout.child.len())
            .map(|_| Variable::new(vec![1, 2]).with_initial(1))
            .collect(),
    )
    .unwrap()
}
pub fn tracker<'a, 's>(
    composition: &'s ComposedPlanningSchema,
    problem: &'a FiniteProblem<i32>,
    layout: &Layout,
) -> SolutionTracker<'a, 's, usize, Assignment<i32>> {
    let schema = composition.schema();
    let base = schema.entity::<Base>().unwrap();
    let child = schema.entity::<Child>().unwrap();
    let mut tracker = SolutionTracker::new(schema);
    for (field, ids, slots) in [
        (
            base.variable("value").unwrap(),
            layout.base.clone(),
            layout.base.clone(),
        ),
        (
            child.variable("value").unwrap(),
            layout.child.clone(),
            layout.child.clone(),
        ),
        (
            child.variable("subValue").unwrap(),
            layout.child.clone(),
            layout.child_sub.clone(),
        ),
    ] {
        let binding = SchemaVariableSlots {
            variable: field,
            slots,
        }
        .bind_basic(problem)
        .unwrap();
        tracker
            .scalar(
                field,
                move |state: &Assignment<i32>| {
                    ids.iter()
                        .enumerate()
                        .map(|(row, &id)| (id, binding.read(state, row).unwrap().copied()))
                        .collect::<BTreeMap<_, _>>()
                },
                i32::to_string,
            )
            .unwrap();
    }
    tracker
}
