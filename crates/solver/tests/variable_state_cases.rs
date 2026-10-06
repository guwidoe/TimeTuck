// Adapted from VariableSupportTest. Apache-2.0; see compat/provenance.json.
// Native stable keys and explicit field snapshots replace Java object descriptors.
use timetuck::*;
struct Solution;
struct Entity;
struct InverseEntity;
struct ListOwner;
struct MixedOwner;
struct MixedValue;
struct Multi;
struct GroupEntity;
struct Group;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct OtherValue(&'static str);
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Entity>(vec![PlanningVariableDefinition::basic::<
            &'static str,
        >("value", false)]),
        PlanningEntityDefinition::of::<InverseEntity>(vec![PlanningVariableDefinition::basic::<
            &'static str,
        >("value", true)]),
        PlanningEntityDefinition::of::<ListOwner>(vec![PlanningVariableDefinition::list::<
            &'static str,
        >("valueList", true)]),
        PlanningEntityDefinition::of::<MixedOwner>(vec![PlanningVariableDefinition::list::<
            &'static str,
        >("valueList", false)]),
        PlanningEntityDefinition::of::<MixedValue>(vec![
            PlanningVariableDefinition::basic::<i32>("delay", false),
            PlanningVariableDefinition::shadow::<&'static str>("entity"),
            PlanningVariableDefinition::shadow::<&'static str>("previous"),
            PlanningVariableDefinition::shadow::<i32>("previousDelay"),
        ]),
        PlanningEntityDefinition::of::<Multi>(vec![
            PlanningVariableDefinition::basic::<&'static str>("primaryValue", false),
            PlanningVariableDefinition::basic::<&'static str>("secondaryValue", false),
            PlanningVariableDefinition::basic::<OtherValue>("tertiaryValueAllowedUnassigned", true),
        ]),
        PlanningEntityDefinition::of::<GroupEntity>(vec![
            PlanningVariableDefinition::basic::<&'static str>("group", false),
            PlanningVariableDefinition::shadow::<String>("ownerCode"),
        ]),
        PlanningEntityDefinition::of::<Group>(vec![PlanningVariableDefinition::basic::<
            &'static str,
        >("owner", false)]),
    ])
    .unwrap()
}
fn field<'s, E: 'static>(s: &'s PlanningSchema, name: &str) -> PlanningVariableRef<'s> {
    s.entity::<E>().unwrap().variable(name).unwrap()
}
#[test]
fn basic_eager() {
    let s = schema();
    let v = field::<Entity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<&str, &str>(v).unwrap();
    r.reset_basic(v, [("a", Some("1"))]).unwrap();
    assert_eq!(state.inverse(&"1"), ["a"]);
    assert!(state.inverse(&"2").is_empty());
    r.change_basic(v, "a", Some("2")).unwrap();
    assert!(state.inverse(&"1").is_empty());
    assert_eq!(state.inverse(&"2"), ["a"]);
}
#[test]
fn repeated_basic_eager() {
    let s = schema();
    let v = field::<Entity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<&str, &str>(v).unwrap();
    r.reset_basic(v, [("a", Some("1"))]).unwrap();
    assert_eq!(state.inverse(&"1"), ["a"]);
    assert!(state.inverse(&"2").is_empty());
    assert!(state.inverse(&"3").is_empty());
    r.change_basic(v, "a", Some("2")).unwrap();
    r.change_basic(v, "a", Some("3")).unwrap();
    assert!(state.inverse(&"1").is_empty());
    assert!(state.inverse(&"2").is_empty());
    assert_eq!(state.inverse(&"3"), ["a"]);
}
#[test]
fn list_eager() {
    let s = schema();
    let v = field::<ListOwner>(&s, "valueList");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(v, [("e1", vec!["1"])]).unwrap();
    let state = r.list::<&str, &str>(v).unwrap();
    assert!(!state.is_assigned(&"2"));
    r.update_lists(v, [("e1", vec!["1", "2"])]).unwrap();
    assert!(state.is_assigned(&"2"));
}
#[test]
fn basic_singleton() {
    let s = schema();
    let v = field::<Entity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<&str, &str>(v).unwrap();
    for _ in 0..2 {
        assert!(state.same_instance(&r.basic(v).unwrap()));
    }
}
#[test]
fn list_singleton() {
    let s = schema();
    let v = field::<ListOwner>(&s, "valueList");
    let mut r = VariableStateRegistry::new(&s);
    let state = r.list::<&str, &str>(v).unwrap();
    for _ in 0..2 {
        assert!(state.same_instance(&r.list(v).unwrap()));
    }
}
#[test]
fn basic_survives_resets() {
    let s = schema();
    let v = field::<GroupEntity>(&s, "group");
    let o = field::<Group>(&s, "owner");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(o, [("g1", Some("o1"))]).unwrap();
    r.reset_basic(v, [("e1", Some("g1"))]).unwrap();
    let state = r.basic::<&str, &str>(v).unwrap();
    for _ in 0..3 {
        r.reset_basic(v, [("e1", Some("g1"))]).unwrap();
        assert!(state.same_instance(&r.basic(v).unwrap()));
        assert_eq!(state.inverse(&"g1"), ["e1"]);
    }
}
#[test]
fn mixed_singletons() {
    let s = schema();
    let b = field::<MixedValue>(&s, "delay");
    let l = field::<MixedOwner>(&s, "valueList");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(l, [("e1", vec!["v1", "v2"])]).unwrap();
    r.reset_basic(b, [("v1", None::<i32>), ("v2", None)])
        .unwrap();
    let basic = r.basic::<&str, i32>(b).unwrap();
    let list = r.list::<&str, &str>(l).unwrap();
    assert_eq!(basic.variable(), b);
    assert_eq!(list.variable(), l);
    assert!(basic.same_instance(&r.basic(b).unwrap()));
    assert!(list.same_instance(&r.list(l).unwrap()));
    r.reset_lists(l, [("e1", vec!["v1", "v2"])]).unwrap();
    r.reset_basic(b, [("v1", None::<i32>), ("v2", None)])
        .unwrap();
    assert!(basic.same_instance(&r.basic(b).unwrap()));
    assert!(list.same_instance(&r.list(l).unwrap()));
}
#[test]
fn multiple_fields_remain_separate() {
    let s = schema();
    let a = field::<Multi>(&s, "primaryValue");
    let b = field::<Multi>(&s, "secondaryValue");
    let c = field::<Multi>(&s, "tertiaryValueAllowedUnassigned");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(a, [("e1", Some("v1"))]).unwrap();
    r.reset_basic(b, [("e1", Some("v2"))]).unwrap();
    r.reset_basic(c, [("e1", Some(OtherValue("o1")))]).unwrap();
    let aa = r.basic::<&str, &str>(a).unwrap();
    let bb = r.basic::<&str, &str>(b).unwrap();
    let cc = r.basic::<&str, OtherValue>(c).unwrap();
    assert!(!aa.same_instance(&bb));
    assert_eq!(aa.variable(), a);
    assert_eq!(bb.variable(), b);
    assert_eq!(cc.variable(), c);
    assert!(aa.same_instance(&r.basic(a).unwrap()));
    assert!(bb.same_instance(&r.basic(b).unwrap()));
    assert!(cc.same_instance(&r.basic(c).unwrap()));
    assert_eq!(aa.inverse(&"v1"), ["e1"]);
    assert!(aa.inverse(&"v2").is_empty());
    assert!(bb.inverse(&"v1").is_empty());
    assert_eq!(bb.inverse(&"v2"), ["e1"]);
    assert_eq!(cc.inverse(&OtherValue("o1")), ["e1"]);
}
#[test]
fn list_tracker_first() {
    let s = schema();
    let v = field::<ListOwner>(&s, "valueList");
    let mut r = VariableStateRegistry::new(&s);
    let tracker = r.tracker::<&str, &str>(v).unwrap();
    let state = r.list::<&str, &str>(v).unwrap();
    assert_eq!(tracker.variable(), v);
    assert_eq!(state.variable(), v);
    assert!(state.same_instance(&r.list(v).unwrap()));
    assert!(tracker.same_instance(&r.tracker::<&str, &str>(v).unwrap()));
    r.reset_lists(v, [("e1", vec!["v1"])]).unwrap();
    assert!(state.is_assigned(&"v1"));
    tracker.before("e1");
    assert!(state.is_assigned(&"v1"));
}
fn basic_order(tracker_first: bool) {
    let s = schema();
    let v = field::<Entity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    let (state, tracker) = if tracker_first {
        let t = r.tracker::<&str, &str>(v).unwrap();
        (r.basic::<&str, &str>(v).unwrap(), t)
    } else {
        let b = r.basic::<&str, &str>(v).unwrap();
        (b, r.tracker::<&str, &str>(v).unwrap())
    };
    assert_eq!(state.variable(), v);
    assert_eq!(tracker.variable(), v);
    assert!(state.same_instance(&r.basic(v).unwrap()));
    assert!(tracker.same_instance(&r.tracker::<&str, &str>(v).unwrap()));
    r.reset_basic(v, [("a", Some("1"))]).unwrap();
    tracker.before("a");
    assert_eq!(state.inverse(&"1"), ["a"]);
    assert_eq!(
        tracker.audit([(v, "a")])[0].missing,
        MissingVariableEvent::After
    );
}
#[test]
fn basic_state_first() {
    basic_order(false);
}
#[test]
fn basic_tracker_first() {
    basic_order(true);
}
#[test]
fn multiple_trackers() {
    let s = schema();
    let mut r = VariableStateRegistry::new(&s);
    let fields = s
        .entity::<Multi>()
        .unwrap()
        .genuine_variables()
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 3);
    let a = r.basic::<&str, &str>(fields[0]).unwrap();
    let at = r.tracker::<&str, &str>(fields[0]).unwrap();
    let bt = r.tracker::<&str, &str>(fields[1]).unwrap();
    let b = r.basic::<&str, &str>(fields[1]).unwrap();
    let c = r.basic::<&str, OtherValue>(fields[2]).unwrap();
    let ct = r.tracker::<&str, OtherValue>(fields[2]).unwrap();
    assert!(!a.same_instance(&b));
    assert!(!at.same_instance(&bt));
    assert!(!at.same_instance(&ct));
    assert!(!bt.same_instance(&ct));
    assert_eq!(a.variable(), fields[0]);
    assert_eq!(at.variable(), fields[0]);
    assert_eq!(b.variable(), fields[1]);
    assert_eq!(bt.variable(), fields[1]);
    assert_eq!(c.variable(), fields[2]);
    assert_eq!(ct.variable(), fields[2]);
    assert!(a.same_instance(&r.basic(fields[0]).unwrap()));
    assert!(b.same_instance(&r.basic(fields[1]).unwrap()));
    assert!(c.same_instance(&r.basic(fields[2]).unwrap()));
    assert!(at.same_instance(&r.tracker::<&str, &str>(fields[0]).unwrap()));
    assert!(bt.same_instance(&r.tracker::<&str, &str>(fields[1]).unwrap()));
    assert!(ct.same_instance(&r.tracker::<&str, OtherValue>(fields[2]).unwrap()));
}
#[test]
fn tracking_preserves_inverse_state() {
    let s = schema();
    let v = field::<InverseEntity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<&str, &str>(v).unwrap();
    let tracker = r.tracker::<&str, &str>(v).unwrap();
    let problem = FiniteProblem::new(vec![
        Variable::new(vec!["v1", "v2"])
            .allowing_unassigned()
            .with_initial("v1"),
    ])
    .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let basics = b.source();
    let universe = b.source::<usize, &str>();
    let inverse = basic_inverse_stream(&mut b, &basics, universe.port(), |_| Some("e1")).unwrap();
    let matches = b
        .score("inverse", &inverse, |_| SimpleScore::new(0))
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&universe, TupleChange::put(0, "v1")).unwrap();
    tx.apply(&universe, TupleChange::put(1, "v2")).unwrap();
    tx.commit().unwrap();
    let mut director =
        StreamScoreDirector::new(&problem, graph, basics, problem.initial_assignment()).unwrap();
    r.synchronize_basic(v, director.assignment(), &[("e1", 0)])
        .unwrap();
    assert_eq!(state.variable(), v);
    assert_eq!(state.inverse(&"v1"), ["e1"]);
    assert!(state.inverse(&"v2").is_empty());
    assert!(state.same_instance(&r.basic(v).unwrap()));
    assert_eq!(
        director.graph().constraint_matches(&matches).unwrap()[&0]
            .0
            .1
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        ["e1"]
    );
    assert!(
        director.graph().constraint_matches(&matches).unwrap()[&1]
            .0
            .1
            .is_empty()
    );
    tracker.audit([]);
    let movement = Move::Change {
        variable: 0,
        value: Some("v2"),
    };
    let edit = director.apply(&movement).unwrap();
    r.synchronize_basic(v, edit.assignment(), &[("e1", 0)])
        .unwrap();
    edit.commit();
    assert!(tracker.audit([(v, "e1")]).is_empty());
    assert_eq!(state.inverse(&"v2"), ["e1"]);
    director.assert_consistent().unwrap();
}
#[test]
fn basic_late_lookup() {
    let s = schema();
    let v = field::<Entity>(&s, "value");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_basic(v, [("e1", Some("v1"))]).unwrap();
    let state = r.basic::<&str, &str>(v).unwrap();
    assert_eq!(state.inverse(&"v1"), ["e1"]);
    assert!(state.inverse(&"v2").is_empty());
}
#[test]
fn list_late_lookup() {
    let s = schema();
    let v = field::<ListOwner>(&s, "valueList");
    let mut r = VariableStateRegistry::new(&s);
    r.reset_lists(v, [("e1", vec!["v1"])]).unwrap();
    let state = r.list::<&str, &str>(v).unwrap();
    assert!(state.is_assigned(&"v1"));
    assert!(!state.is_assigned(&"v2"));
}
#[test]
fn linking_does_not_read_solution() {
    let s = schema();
    let v = field::<InverseEntity>(&s, "value");
    let reads = std::cell::Cell::new(0);
    let solution = || {
        reads.set(reads.get() + 1);
        [("e1", Some("v1"))]
    };
    let mut r = VariableStateRegistry::new(&s);
    let state = r.basic::<&str, &str>(v).unwrap();
    assert_eq!(state.variable(), v);
    assert_eq!(reads.get(), 0);
    assert!(state.inverse(&"v1").is_empty());
    r.reset_basic(v, solution()).unwrap();
    assert_eq!(reads.get(), 1);
    assert_eq!(state.inverse(&"v1"), ["e1"]);
}
