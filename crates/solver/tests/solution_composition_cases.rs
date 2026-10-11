// Adapted from Community SolverSolutionInheritanceTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
mod solution_composition_support;
use solution_composition_support::*;
use timetuck::*;
fn check<const N: usize>(shape: Shape, score: i64, limit: u64, count: usize, rewards: usize) {
    let layout = compile::<N>(shape, SolutionCompositionPolicy::single_level()).unwrap();
    let fixture = generate::<N>(shape);
    assert_eq!(fixture.actual_shape, shape);
    let bound = bind(&layout, fixture);
    assert_eq!(bound.entities().len(), count);
    for entity in bound.entities() {
        assert!(
            !entity
                .object_ref::<ChildEntity<N>>()
                .unwrap()
                .code
                .is_empty()
        );
    }
    for result in solve_fixture::<N>(&bound, rewards, limit) {
        assert_eq!(result.score, Some(SimpleScore::new(score)));
        assert!(result.assignment.values().iter().all(Option::is_some));
    }
}
#[test]
fn test_both_classes_annotated() {
    check::<1>(Shape::Both, 16, 100, 8, 2);
}
#[test]
fn test_both_classes_annotated_base_abstract() {
    check::<2>(Shape::Abstract, 8, 14, 4, 2);
}
#[test]
fn test_only_base_class_annotated() {
    check::<3>(Shape::BaseOnly, 2, 14, 2, 1);
}
#[test]
fn test_only_child_class_annotated() {
    // Source actually generates the Both fixture despite configuring ChildOnly.
    let problem = generate::<4>(Shape::Both);
    assert_eq!(problem.actual_shape, Shape::Both);
    let error =
        compile::<4>(Shape::ChildOnly, SolutionCompositionPolicy::single_level()).unwrap_err();
    assert!(
        matches!(&error,SolutionCompositionError::UnregisteredMembers { component, names } if *component == t::<Parent<4>>() && names == &["entityList","score","valueList"])
    );
    let message = error.to_string();
    for fragment in [
        "is not registered as a planning solution but defines planning members",
        "Maybe register",
        "as a planning solution",
        "Maybe remove the planning members ([entityList, score, valueList])",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
}
#[test]
fn test_replace_annotated_member() {
    let problem = generate::<5>(Shape::Replaced);
    assert_eq!(problem.actual_shape, Shape::Replaced);
    let error =
        compile::<5>(Shape::Replaced, SolutionCompositionPolicy::single_level()).unwrap_err();
    assert!(
        matches!(&error,SolutionCompositionError::DuplicateMember { name, original, redefining, .. } if name == "entityList" && *original == t::<Parent<5>>() && *redefining == t::<Root<5>>())
    );
    for fragment in [
        "has a planning entity collection member",
        "that is duplicated by a planning entity collection member",
        "Maybe the member is declared on both the field and its getter.",
    ] {
        assert!(error.to_string().contains(fragment));
    }
}
#[test]
fn test_multiple_inheritance() {
    // Source actually generates ReplaceMember, not the configured Multiple type.
    let problem = generate::<6>(Shape::Replaced);
    assert_eq!(problem.actual_shape, Shape::Replaced);
    let error =
        compile::<6>(Shape::Multiple, SolutionCompositionPolicy::single_level()).unwrap_err();
    assert_eq!(
        error,
        SolutionCompositionError::DepthLimit {
            solution: t::<Root<6>>(),
            actual: 3,
            maximum: 2
        }
    );
    assert!(
        error
            .to_string()
            .contains("inherits its planning solution registration from multiple components")
    );
    assert!(error.to_string().contains("Remove solution components from the composition chain to create a single-level inheritance structure."));
}
