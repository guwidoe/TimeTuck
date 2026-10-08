// Adapted from Timefold Solver Community SolutionManagerTest and shadow test fixtures.
// Copyright Timefold BV and contributors. Apache-2.0; see THIRD_PARTY_NOTICES.md
// and compat/provenance.json for source identities and adaptations.
use timetuck::*;
struct Entity;
struct World(Vec<Option<String>>);
fn policy_case(policy: SolutionUpdatePolicy) {
    let schema = PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<String>("value", false),
        PlanningVariableDefinition::shadow::<String>("firstShadow"),
    ])])
    .unwrap();
    let values = (0..5)
        .map(|i| format!("Generated Value {i}"))
        .collect::<Vec<_>>();
    let mut solution = ManagedSolution::new(
        World((0..7).map(|i| Some(values[i % 5].clone())).collect()),
        vec![None::<String>; 7],
    );
    assert!(solution.score.is_none());
    assert!(solution.shadows[0].is_none());
    let entity = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, World>::new(&schema);
    let mut outputs = vec![];
    for index in 0..7 {
        let input = builder
            .input(entity.variable("value").unwrap(), index, move |w| {
                w.0[index].clone()
            })
            .unwrap();
        let source = input.clone();
        let output = builder
            .derive(
                entity.variable("firstShadow").unwrap(),
                index,
                move |_, inputs| {
                    inputs
                        .get(&source)
                        .unwrap()
                        .map(|s| format!("{s}/firstShadow"))
                },
            )
            .unwrap();
        builder.set_dependencies(&output, &[input.erase()]).unwrap();
        outputs.push(output);
    }
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            outputs
                .iter()
                .map(|node| session.read(node).map(|read| read.value.cloned()))
                .collect::<Result<Vec<_>, _>>()
        },
        |_, _| Ok(()),
        |w, _| {
            let mut pairs = 0;
            for left in w.0.iter().flatten() {
                for right in w.0.iter().flatten() {
                    if left == right {
                        pairs += 1;
                    }
                }
            }
            Ok(SimpleScore::new(-pairs))
        },
    );
    // A native constructor returns a manager directly; Java factory/solver-manager
    // creation variants and their redundant non-null assertion have no native split.
    if policy == SolutionUpdatePolicy::Everything {
        manager.update(&mut solution).unwrap();
    } else {
        manager.update_with_policy(&mut solution, policy).unwrap();
    }
    if policy == SolutionUpdatePolicy::ShadowsOnly {
        assert!(solution.score.is_none());
    } else {
        assert!(solution.score.is_some());
        assert_eq!(solution.score, Some(SimpleScore::new(-11)));
    }
    if policy == SolutionUpdatePolicy::ScoreOnly {
        assert!(solution.shadows[0].is_none());
    } else {
        assert!(solution.shadows[0].is_some());
    }
}
#[test]
fn update_everything() {
    policy_case(SolutionUpdatePolicy::Everything);
}
#[test]
fn update_only_shadow_variables() {
    policy_case(SolutionUpdatePolicy::ShadowsOnly);
}
#[test]
fn update_only_score() {
    policy_case(SolutionUpdatePolicy::ScoreOnly);
}
