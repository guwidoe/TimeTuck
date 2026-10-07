// Adapted from AlwaysLoopedShadowVariableTest and its three fixtures.
// Apache-2.0; see compat/provenance.json.
use std::{any::Any, cell::Cell};
use timetuck::*;
struct Solution;
#[derive(Clone)]
struct Entity {
    id: usize,
    value: Option<i32>,
}
#[test]
fn fails_if_always_looped_variable_present() {
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
            PlanningVariableDefinition::shadow::<bool>("odd"),
            PlanningVariableDefinition::shadow::<bool>("even"),
        ])])
        .unwrap();
    let mut access = SourcePathRegistry::new(&schema);
    access
        .one("value", SourceMemberRole::Variable, |v: &Entity| {
            v.value.as_ref()
        })
        .unwrap();
    for field in ["odd", "even"] {
        access
            .one(
                field,
                SourceMemberRole::Declarative,
                |_: &Entity| -> Option<&bool> { panic!("terminal getter") },
            )
            .unwrap();
    }
    let even_value = access.compile::<Entity>("even", "value").unwrap();
    let even_odd = access.compile::<Entity>("even", "odd").unwrap();
    let odd_value = access.compile::<Entity>("odd", "value").unwrap();
    let odd_even = access.compile::<Entity>("odd", "even").unwrap();
    let e = schema.entity::<Entity>().unwrap();
    let input_field = e.variable("value").unwrap();
    let odd_field = e.variable("odd").unwrap();
    let even_field = e.variable("even").unwrap();
    let calls = &Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2, 3, 4, 5]); 3]).unwrap();
    let result = solve_stream(
        &p,
        || {
            let mut shadows = ShadowSessionBuilder::<usize, Vec<Entity>>::new(&schema);
            let mut pairs = vec![];
            for id in 0..3 {
                shadows
                    .input(input_field, id, move |w| {
                        calls.set(calls.get() + 1);
                        w[id].value
                    })
                    .unwrap();
                let even = shadows
                    .derive(even_field, id, move |_, inputs| {
                        calls.set(calls.get() + 1);
                        let values = inputs.for_field::<i32>(input_field).unwrap();
                        let value = **values.first()?.as_ref()?;
                        let odds = inputs.for_field::<bool>(odd_field).unwrap();
                        Some(
                            odds.first()
                                .copied()
                                .flatten()
                                .map_or(value % 2 == 0, |odd| !odd),
                        )
                    })
                    .unwrap();
                let odd = shadows
                    .derive(odd_field, id, move |_, inputs| {
                        calls.set(calls.get() + 1);
                        let values = inputs.for_field::<i32>(input_field).unwrap();
                        let value = **values.first()?.as_ref()?;
                        let evens = inputs.for_field::<bool>(even_field).unwrap();
                        Some(
                            evens
                                .first()
                                .copied()
                                .flatten()
                                .map_or(value % 2 == 1, |even| !even),
                        )
                    })
                    .unwrap();
                let key = |o: &dyn Any| o.downcast_ref::<Entity>().map(|e| e.id);
                shadows
                    .bind_source_paths(&even, vec![&even_value, &even_odd], move |w| &w[id], key)
                    .unwrap();
                shadows
                    .bind_source_paths(&odd, vec![&odd_value, &odd_even], move |w| &w[id], key)
                    .unwrap();
                pairs.push((odd, even));
            }
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            let rows = b.collect_list(source.port(), |(id, value)| Entity {
                id: *id,
                value: *value,
            })?;
            let world = b.map(&rows, |rows| rows.iter().cloned().collect::<Vec<_>>())?;
            let flags = b.map_shadows(&world, shadows, move |_, session| {
                pairs
                    .iter()
                    .map(|(odd, even)| {
                        Ok((
                            session.read(odd)?.value.copied(),
                            session.read(even)?.value.copied(),
                        ))
                    })
                    .collect::<Result<Vec<_>, ShadowSessionError>>()
            })?;
            b.score("Odd even count", &flags, |flags| {
                let count = flags.iter().filter(|(_, even)| *even == Some(true)).count();
                SimpleScore::new(-i64::from(count > 0 && count % 2 != 0))
            })?;
            b.score("Even odd count", &flags, |flags| {
                let count = flags.iter().filter(|(odd, _)| *odd == Some(true)).count();
                SimpleScore::new(-i64::from(count > 0 && count % 2 != 1))
            })?;
            Ok((b.build(), source))
        },
        SolverConfig {
            limits: SolverLimits {
                best_score_limit: Some(SimpleScore::ZERO),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let message = result.unwrap_err().to_string();
    assert!(message.contains("fixed dependency loops"), "{message}");
    assert!(
        message.contains("odd") && message.contains("even"),
        "{message}"
    );
    assert_eq!(
        calls.get(),
        0,
        "reject permanent loops before evaluating any fields"
    );
}
