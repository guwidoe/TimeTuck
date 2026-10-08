// Adapted from Timefold Community testdomain/shadow/concurrent fixtures.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json
// and THIRD_PARTY_NOTICES.md. Times are minutes relative to BASE_START_TIME.
use std::cell::Cell;
use timetuck::*;
pub const VALUES: [&str; 4] = ["a1", "a2", "b1", "b2"];
pub struct Solution;
pub struct Visit;
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Schedule {
    pub ready: [Option<i64>; 4],
    pub start: [Option<i64>; 4],
    pub finish: [Option<i64>; 4],
    pub cascading: [Option<i64>; 4],
    pub inconsistent: [bool; 4],
}
pub fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Visit>(vec![
        PlanningVariableDefinition::shadow::<usize>("entity"),
        PlanningVariableDefinition::shadow::<usize>("index"),
        PlanningVariableDefinition::shadow::<i64>("serviceReadyTime"),
        PlanningVariableDefinition::shadow::<i64>("serviceStartTime"),
        PlanningVariableDefinition::shadow::<i64>("serviceFinishTime"),
        PlanningVariableDefinition::shadow::<i64>("cascadingTime"),
    ])])
    .unwrap()
}
pub fn problem() -> ListProblem<&'static str> {
    ListProblem::new(
        VALUES.to_vec(),
        vec![
            ListEntity::new(vec!["a1", "b1"]),
            ListEntity::new(vec!["b2", "a2"]),
        ],
    )
    .unwrap()
}
pub type Manager<'a> = ListDeclarativeSolutionManager<
    'a,
    'a,
    &'static str,
    &'static str,
    Schedule,
    HardSoftScore,
    &'static str,
>;
pub fn manager<'a>(
    schema: &'a PlanningSchema,
    p: &'a ListProblem<&'static str>,
    calls: &'a Cell<usize>,
    fail: &'a Cell<bool>,
) -> Manager<'a> {
    let entity = schema.entity::<Visit>().unwrap();
    let owner_field = entity.variable("entity").unwrap();
    let ready_field = entity.variable("serviceReadyTime").unwrap();
    let start_field = entity.variable("serviceStartTime").unwrap();
    let finish_field = entity.variable("serviceFinishTime").unwrap();
    let mut b = ShadowSessionBuilder::<&'static str, ListAssignment<&'static str>>::new(schema);
    let mut owners = vec![];
    let mut indices = vec![];
    let mut ready = vec![];
    for id in VALUES {
        owners.push(
            b.input(owner_field, id, move |a| {
                calls.set(calls.get() + 1);
                a.element(&id).map(|e| e.position.entity)
            })
            .unwrap(),
        );
        indices.push(
            b.input(entity.variable("index").unwrap(), id, move |a| {
                a.element(&id).map(|e| e.position.index)
            })
            .unwrap(),
        );
        ready.push(
            b.derive(ready_field, id, move |_, i| {
                let owner = i.for_field::<usize>(owner_field).unwrap();
                owner.first().copied().flatten()?;
                let previous = i.for_field::<i64>(finish_field).unwrap();
                Some(match previous.first() {
                    Some(Some(v)) => *v + 30,
                    Some(None) => return None,
                    None => 0,
                })
            })
            .unwrap(),
        );
    }
    let mut start = vec![];
    for (index, id) in VALUES.into_iter().enumerate() {
        let own = ready[index].clone();
        start.push(
            b.derive(start_field, id, move |_, i| {
                i.get(&own).unwrap()?;
                i.for_field::<i64>(ready_field)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .max()
            })
            .unwrap(),
        );
    }
    let mut finish = vec![];
    let mut cascade = vec![];
    for (index, id) in VALUES.into_iter().enumerate() {
        let source = start[index].clone();
        finish.push(
            b.derive(finish_field, id, move |_, i| {
                i.get(&source).unwrap().map(|v| v + 30)
            })
            .unwrap(),
        );
        let source = indices[index].clone();
        cascade.push(
            b.derive(
                entity.variable("cascadingTime").unwrap(),
                id,
                move |_, i| i.get(&source).unwrap().map(|v| *v as i64 * 1440),
            )
            .unwrap(),
        );
    }
    for (index, id) in VALUES.into_iter().enumerate() {
        let owner = owners[index].erase();
        let ends = finish.clone();
        b.bind_dependencies(&ready[index], move |a| {
            let mut deps = vec![owner.clone()];
            if let Some(previous) = a.element(&id).and_then(|e| e.previous) {
                deps.push(ends[VALUES.iter().position(|id| id == previous).unwrap()].erase());
            }
            Ok(deps)
        })
        .unwrap();
        let group = if index < 2 { 0..2 } else { 2..4 };
        b.set_dependencies(
            &start[index],
            &group.map(|i| ready[i].erase()).collect::<Vec<_>>(),
        )
        .unwrap();
        b.set_dependencies(&finish[index], &[start[index].erase()])
            .unwrap();
        b.set_dependencies(&cascade[index], &[indices[index].erase()])
            .unwrap();
    }
    list_declarative_solution_manager(
        p,
        b.build(),
        move |_, graph| {
            let mut out = Schedule::default();
            for index in 0..4 {
                let r = graph.read(&ready[index]).unwrap();
                let s = graph.read(&start[index]).unwrap();
                let f = graph.read(&finish[index]).unwrap();
                out.ready[index] = r.value.copied();
                out.start[index] = s.value.copied();
                out.finish[index] = f.value.copied();
                out.cascading[index] = graph.read(&cascade[index]).unwrap().value.copied();
                out.inconsistent[index] = r.inconsistent || s.inconsistent || f.inconsistent;
            }
            Ok(out)
        },
        |_, _| Ok(()),
        move |_, h| {
            if fail.get() {
                return Err("scoring");
            }
            let hard = -(h.derived.inconsistent.iter().filter(|&&v| v).count() as i64);
            let soft = -(0..4)
                .filter(|&i| h.builtin[i].owner.is_some() && !h.derived.inconsistent[i])
                .map(|i| h.derived.finish[i].expect("assigned consistent finish"))
                .sum::<i64>();
            Ok(HardSoftScore::new(hard, soft))
        },
    )
}
