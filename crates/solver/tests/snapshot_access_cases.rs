// Adapted from pinned Community AllVariablesAssertTest and MutationCounterTest.
// Apache-2.0; source cases and generator fixtures in compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::*;
struct Solution;
struct Entity;
type Basic = BTreeMap<usize, Option<i32>>;
type Lists = BTreeMap<usize, Vec<i32>>;
fn schema(list: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        if list {
            PlanningVariableDefinition::list::<i32>("valueList", false)
        } else {
            PlanningVariableDefinition::basic::<i32>("value", false)
        },
    ])])
    .unwrap()
}
fn basic_tracker(s: &PlanningSchema) -> SolutionTracker<'_, '_, usize, Basic> {
    let mut t = SolutionTracker::new(s);
    t.scalar(
        s.entity::<Entity>().unwrap().variable("value").unwrap(),
        |w: &Basic| w.clone(),
        i32::to_string,
    )
    .unwrap();
    t
}
fn list_tracker(s: &PlanningSchema) -> SolutionTracker<'_, '_, usize, Lists> {
    let mut t = SolutionTracker::new(s);
    t.list(
        s.entity::<Entity>().unwrap().variable("valueList").unwrap(),
        |w: &Lists| w.clone(),
        i32::to_string,
    )
    .unwrap();
    t
}
fn generated_basic() -> Basic {
    (0..3).map(|i| (i, Some(i as i32))).collect()
}
fn generated_lists() -> Lists {
    (0..3)
        .map(|i| (i, (i..9).step_by(3).map(|v| v as i32).collect()))
        .collect()
}
fn edit_basic(w: &mut Basic) {
    w.insert(0, None);
    w.insert(1, Some(0));
}
#[test]
fn snapshot_basic_variables() {
    let s = schema(false);
    let t = basic_tracker(&s);
    let field = s.entity::<Entity>().unwrap().variable("value").unwrap();
    let mut w = generated_basic();
    let snapshot = t.snapshot(&w);
    edit_basic(&mut w);
    for i in 0..3 {
        let entry = snapshot.get::<Option<i32>>(field, &i).unwrap().unwrap();
        assert_eq!(entry.variable, field);
        assert_eq!(entry.entity, &i);
        assert_eq!(entry.value, &Some(i as i32));
    }
}
#[test]
fn snapshot_list_variables() {
    let s = schema(true);
    let t = list_tracker(&s);
    let field = s.entity::<Entity>().unwrap().variable("valueList").unwrap();
    let mut w = generated_lists();
    let snapshot = t.snapshot(&w);
    w.insert(0, vec![0, 1]);
    w.insert(1, vec![2, 3, 4]);
    w.insert(2, vec![5, 6, 7, 8]);
    for (i, expected) in [vec![0, 3, 6], vec![1, 4, 7], vec![2, 5, 8]]
        .iter()
        .enumerate()
    {
        let entry = snapshot.get::<Vec<i32>>(field, &i).unwrap().unwrap();
        assert_eq!(entry.variable, field);
        assert_eq!(entry.entity, &i);
        assert_eq!(entry.value, expected);
    }
}
#[test]
fn snapshot_diff_basic_variables() {
    let s = schema(false);
    let t = basic_tracker(&s);
    let field = s.entity::<Entity>().unwrap().variable("value").unwrap();
    let mut w = generated_basic();
    let before = t.snapshot(&w);
    edit_basic(&mut w);
    let after = t.snapshot(&w);
    assert_eq!(
        before
            .changed_variables(&after)
            .unwrap()
            .collect::<Vec<_>>(),
        [(field, &0), (field, &1)]
    );
}
#[test]
fn snapshot_diff_list_variables() {
    let s = schema(true);
    let t = list_tracker(&s);
    let field = s.entity::<Entity>().unwrap().variable("valueList").unwrap();
    let mut w = generated_lists();
    let before = t.snapshot(&w);
    w.insert(0, vec![0, 3]);
    w.insert(1, vec![1, 4, 6, 7]);
    let after = t.snapshot(&w);
    assert_eq!(
        before
            .changed_variables(&after)
            .unwrap()
            .collect::<Vec<_>>(),
        [(field, &0), (field, &1)]
    );
}
fn mutations(values: [i32; 4]) -> usize {
    let s = schema(false);
    let t = basic_tracker(&s);
    let a = [1, 1, 3, 3]
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i, Some(v)))
        .collect();
    let b = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| (i, Some(v)))
        .collect();
    t.snapshot(&a).mutation_count(&t.snapshot(&b)).unwrap()
}
#[test]
fn count_mutations_none() {
    assert_eq!(mutations([1, 1, 3, 3]), 0);
}
#[test]
fn count_mutations_some() {
    assert_eq!(mutations([3, 1, 3, 2]), 2);
}
#[test]
fn count_mutations_all() {
    assert_eq!(mutations([2, 2, 2, 2]), 4);
}
