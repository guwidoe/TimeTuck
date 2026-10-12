// Complete adapted pinned Community configuration rejection TEST methods.
// Apache-2.0; explicit native declarations replace annotations/class loading.
use timetuck::*;
struct Solution;
struct ListEntityType;
struct OtherEntityType;
struct DummyValueComparator;
struct DummyValueComparatorFactory;
fn list_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<ListEntityType>(vec![
        PlanningVariableDefinition::list::<i32>("valueList", false),
    ])])
    .unwrap()
}
fn mixed_schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<ListEntityType>(vec![
            PlanningVariableDefinition::list::<i32>("valueList", false),
            PlanningVariableDefinition::basic::<i32>("basicValue", false),
            PlanningVariableDefinition::basic::<i32>("secondBasicValue", false),
        ]),
        PlanningEntityDefinition::of::<OtherEntityType>(vec![
            PlanningVariableDefinition::shadow::<usize>("owner"),
        ]),
    ])
    .unwrap()
}
fn reject(
    schema: &PlanningSchema,
    settings: &SolverBuildSettings,
    phases: Option<SolverSequence<'static>>,
) -> SolverPreflightError {
    settings
        .build::<(), (), String, SimpleScore>(schema, phases, |_, _| {
            panic!("invalid declarations reached runtime factory")
        })
        .err()
        .unwrap()
}
#[test]
fn fail_construction_heuristic_entity_range() {
    // Source creates3 entities and3 values with entity-provided ranges. Both
    // repeated source declarations must fail before factory/scoring execution.
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![]).with_value_range(vec![0, 1, 2]); 3],
    )
    .unwrap();
    for _ in 0..2 {
        let settings = SolverBuildSettings {
            selectors: vec![ConstructionSelectorDeclaration::for_list_problem(
                PlanningFieldKey::of::<ListEntityType>("valueList"),
                &p,
                true,
                SelectionCacheScope::Phase,
                false,
            )],
            ..Default::default()
        };
        let e = reject(&list_schema(), &settings, Some(SolverSequence::default())).to_string();
        assert!(e.contains(
            "resolvedSelectionOrder (SORTED) which does not support the resolvedCacheType (PHASE)"
        ));
        assert!(e.contains("Maybe set the \"cacheType\" to STEP."));
    }
}
#[test]
fn fail_construction_heuristic_list_mixed_properties() {
    let settings = SolverBuildSettings {
        sorters: vec![VariableSorterDeclaration {
            field: PlanningFieldKey::of::<ListEntityType>("valueList"),
            comparator: Some(PlanningType::of::<DummyValueComparator>()),
            comparator_factory: Some(PlanningType::of::<DummyValueComparatorFactory>()),
        }],
        ..Default::default()
    };
    let e = reject(&list_schema(), &settings, None).to_string();
    assert!(e.contains(&format!(
        "The entityClass ({}) property (valueList)",
        std::any::type_name::<ListEntityType>()
    )));
    assert!(e.contains(&format!(
        "cannot have a comparatorClass ({})",
        std::any::type_name::<DummyValueComparator>()
    )));
    assert!(e.contains(&format!(
        "comparatorFactoryClass ({}) at the same time.",
        std::any::type_name::<DummyValueComparatorFactory>()
    )));
}
#[test]
fn fail_construction_heuristic_both_nearby_and_sorting() {
    let p = ListProblem::new(vec![], vec![]).unwrap();
    let settings = SolverBuildSettings {
        selectors: vec![ConstructionSelectorDeclaration::for_list_problem(
            PlanningFieldKey::of::<ListEntityType>("valueList"),
            &p as &ListProblem<i32>,
            true,
            SelectionCacheScope::Phase,
            true,
        )],
        ..Default::default()
    };
    let e = reject(&list_schema(), &settings, Some(SolverSequence::default())).to_string();
    for part in [
        "The nearbySelectorConfig",
        "Maybe remove difficultyComparatorClass or difficultyWeightFactoryClass from your @PlanningEntity annotation.",
        "Maybe remove strengthComparatorClass or strengthWeightFactoryClass from your @PlanningVariable annotation.",
        "Maybe disable nearby selection.",
    ] {
        assert!(e.contains(part));
    }
}
#[test]
fn fail_mixed_model_default_configuration() {
    let e = reject(&mixed_schema(), &SolverBuildSettings::default(), None);
    assert!(
        e.to_string()
            .contains("has both basic and list variables and cannot be deduced automatically")
    );
}
#[test]
fn fail_missing_basic_variable_configuration() {
    let schema = mixed_schema();
    for cartesian in [false, true] {
        // One Change selector, directly or as the sole product child, resolves
        // one field. Neither covers the other basic field and list variable.
        let settings = SolverBuildSettings {
            exhaustive_selection: Some(if cartesian {
                ExhaustiveFieldSelection::Cartesian(vec![ExhaustiveFieldSelection::BasicChange(
                    None,
                )])
            } else {
                ExhaustiveFieldSelection::BasicChange(None)
            }),
            ..Default::default()
        };
        let sequence = SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        };
        assert!(
            reject(&schema, &settings, Some(sequence))
                .to_string()
                .contains("does not include all the expected planning variables")
        );
    }
}
const CUSTOM: &str =
    include_str!("fixtures/preflight/testSolverConfigWithNonexistentCustomPhase.xml");
fn custom_config() -> (SolverSequence<'static>, SolverXmlConfiguration) {
    (
        SolverSequence {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        SolverXmlConfiguration::parse(CUSTOM).unwrap(),
    )
}
#[test]
fn non_existent_custom_phase_command() {
    let (sequence, xml) = custom_config();
    let registry =
        PhaseCommandRegistry::<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>::default();
    assert!(
        registry
            .bind(&sequence, &CustomCommandNames::from_xml(&xml))
            .err()
            .unwrap()
            .to_string()
            .contains("customPhaseCommandClass")
    );
}
#[test]
fn null_custom_phase_commands() {
    let (_, xml) = custom_config();
    assert_eq!(xml.phase_names(), vec!["customPhase"]);
    let commands =
        CustomPhaseCommands::<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>::from_optional(
            [None],
        );
    assert!(
        commands
            .err()
            .unwrap()
            .to_string()
            .contains("Custom phase commands")
    );
}
#[test]
fn null_custom_phase_command_list() {
    let (_, xml) = custom_config();
    assert_eq!(xml.phase_names(), vec!["customPhase"]);
    let commands =
        CustomPhaseCommands::<Assignment<i32>, Move<i32>, SimpleScore, ScoringError>::from_optional(
            vec![None],
        );
    assert!(
        commands
            .err()
            .unwrap()
            .to_string()
            .contains("Custom phase commands")
    );
}
