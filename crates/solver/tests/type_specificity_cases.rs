// Adapted complete GizmoSolutionClonerTest.instanceOfComparatorTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Default)]
struct Animal;
#[derive(Default)]
struct Robot;
#[derive(Default)]
struct Zebra {
    animal: Animal,
}
#[derive(Default)]
struct RobotZebra {
    zebra: Zebra,
    robot: Robot,
}
#[test]
fn instance_of_comparator_test() {
    let mut hierarchy = SourceTypeHierarchy::default();
    hierarchy.register(|z: &Zebra| &z.animal).unwrap();
    hierarchy.register(|z: &RobotZebra| &z.zebra).unwrap();
    hierarchy.register(|z: &RobotZebra| &z.robot).unwrap();
    let animal = PlanningType::of::<Animal>();
    let robot = PlanningType::of::<Robot>();
    let zebra = PlanningType::of::<Zebra>();
    let robot_zebra = PlanningType::of::<RobotZebra>();
    let order = TypeSpecificityOrder::new(&hierarchy, [animal, robot, zebra, robot_zebra]).unwrap();
    for ty in [animal, robot, zebra, robot_zebra] {
        assert!(order.compare(ty, ty).unwrap().is_eq());
    }
    for parent in [animal, robot] {
        assert!(order.compare(zebra, parent).unwrap().is_lt());
        assert!(order.compare(parent, zebra).unwrap().is_gt());
    }
    for parent in [animal, robot, zebra] {
        assert!(order.compare(robot_zebra, parent).unwrap().is_lt());
        assert!(order.compare(parent, robot_zebra).unwrap().is_gt());
    }
}
