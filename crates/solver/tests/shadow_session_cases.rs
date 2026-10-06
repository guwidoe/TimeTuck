// Adapted from ShadowVariableUpdateTest and basic/parameter shadow fixtures.
// Apache-2.0; see compat/provenance.json. Native typed registration replaces reflection.
use timetuck::*;
struct Solution;
struct ParameterSolution;
struct Entity;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Time {
    origin: [i32; 6],
    seconds: i64,
}
impl Time {
    fn days(days: i64) -> Self {
        Self {
            origin: [2025, 4, 29, 18, 40, 0],
            seconds: days * 86_400,
        }
    }
    fn plus(self, seconds: i64) -> Self {
        Self {
            seconds: self.seconds + seconds,
            ..self
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
enum Fact {
    Integer(i32),
    Text(&'static str),
    Opaque,
}
struct World {
    values: Vec<Option<usize>>,
    inverse: Vec<Vec<usize>>,
    durations: Vec<i64>,
    facts: Vec<Fact>,
}
fn schema(parameter: bool) -> PlanningSchema {
    let entities = vec![
        PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<usize>("value", false),
            PlanningVariableDefinition::shadow::<i64>("durationInDays"),
        ]),
        PlanningEntityDefinition::of::<usize>(vec![
            PlanningVariableDefinition::shadow::<Vec<usize>>("entityList"),
            PlanningVariableDefinition::shadow::<Time>("startTime"),
            PlanningVariableDefinition::shadow::<Time>("endTime"),
        ]),
    ];
    if parameter {
        PlanningSchema::new::<ParameterSolution>(entities).unwrap()
    } else {
        PlanningSchema::new::<Solution>(entities).unwrap()
    }
}
fn run_full(parameter: bool, solution: bool) {
    let s = schema(parameter);
    let value_field = s.entity::<Entity>().unwrap().variable("value").unwrap();
    let v = s.entity::<usize>().unwrap();
    let mut registry = VariableStateRegistry::new(&s);
    registry
        .reset_basic(
            value_field,
            [(0usize, Some(0usize)), (1, Some(1)), (2, Some(0))],
        )
        .unwrap();
    let mut supply =
        BasicStateSupply::new(registry.basic::<usize, usize>(value_field).unwrap(), [0, 1])
            .unwrap();
    supply
        .externalize(v.variable("entityList").unwrap())
        .unwrap();
    let mut world = World {
        values: vec![Some(0), Some(1), Some(0)],
        inverse: vec![vec![], vec![]],
        durations: vec![10, 20],
        facts: if solution {
            vec![Fact::Integer(1), Fact::Text("Data"), Fact::Opaque]
        } else {
            vec![]
        },
    };
    for change in supply.refresh_changed().unwrap().shadows {
        assert_eq!(world.inverse[change.value], change.before);
        world.inverse[change.value] = change.after;
    }
    let mut b = ShadowSessionBuilder::<usize, World>::new(&s);
    let mut starts = vec![];
    let mut ends = vec![];
    for value in 0..2 {
        let inverse = b
            .input(v.variable("entityList").unwrap(), value, move |w| {
                Some(w.inverse[value].clone())
            })
            .unwrap();
        let input = inverse.clone();
        let start = b
            .derive(
                v.variable("startTime").unwrap(),
                value,
                move |world, inputs| {
                    // A real non-null solution reference is available to parameter suppliers.
                    assert_eq!(world.durations.len(), 2);
                    let members = inputs.get(&input).unwrap().unwrap();
                    Some(Time::days(if members.is_empty() {
                        10
                    } else {
                        members.len() as i64
                    }))
                },
            )
            .unwrap();
        b.set_dependencies(&start, &[inverse.erase()]).unwrap();
        let input = start.clone();
        let end = b
            .derive(
                v.variable("endTime").unwrap(),
                value,
                move |world, inputs| {
                    inputs
                        .get(&input)
                        .unwrap()
                        .map(|time| time.plus(world.durations[value]))
                },
            )
            .unwrap();
        b.set_dependencies(&end, &[start.erase()]).unwrap();
        starts.push(start);
        ends.push(end);
    }
    let mut session = b.build();
    session.update(&world).unwrap();
    assert_eq!(world.inverse[0], [0, 2]);
    assert_eq!(world.inverse[1], [1]);
    for value in 0..2 {
        let start = session.read(&starts[value]).unwrap();
        let end = session.read(&ends[value]).unwrap();
        assert_eq!(
            start.value,
            Some(&Time::days(world.inverse[value].len() as i64))
        );
        assert_eq!(
            end.value,
            Some(&start.value.unwrap().plus(world.durations[value]))
        );
        assert!(!start.inconsistent && !end.inconsistent);
    }
    if solution {
        assert_eq!(
            world.facts,
            [Fact::Integer(1), Fact::Text("Data"), Fact::Opaque]
        );
    }
}
#[test]
fn update_basic_shadow_variables() {
    run_full(false, false);
}
#[test]
fn solution_update_basic_shadow_variables() {
    run_full(false, true);
}
#[test]
fn solution_update_basic_shadow_variables_with_parameter() {
    run_full(true, true);
}
#[test]
fn update_basic_shadow_variables_only_planning_entity() {
    let s = schema(false);
    let e = s.entity::<Entity>().unwrap();
    let world = World {
        values: vec![Some(0), Some(1)],
        inverse: vec![],
        durations: vec![10 * 86_400, 20 * 86_400],
        facts: vec![],
    };
    let mut b = ShadowSessionBuilder::<usize, World>::new(&s);
    let mut nodes = vec![];
    for entity in 0..2 {
        let value = b
            .input(e.variable("value").unwrap(), entity, move |w| {
                w.values[entity]
            })
            .unwrap();
        let input = value.clone();
        let days = b
            .derive(
                e.variable("durationInDays").unwrap(),
                entity,
                move |w, i| {
                    Some(
                        i.get(&input)
                            .unwrap()
                            .map_or(0, |&v| w.durations[v] / 86_400),
                    )
                },
            )
            .unwrap();
        b.set_dependencies(&days, &[value.erase()]).unwrap();
        nodes.push(days);
    }
    let mut session = b.build();
    session.update(&world).unwrap();
    assert_eq!(
        session.read(&nodes[0]).unwrap().value,
        Some(&(world.durations[0] / 86_400))
    );
    assert_eq!(
        session.read(&nodes[1]).unwrap().value,
        Some(&(world.durations[1] / 86_400))
    );
}
