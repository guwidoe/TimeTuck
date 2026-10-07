// Adapted from VariableChangeRecordingScoreDirectorTest and list fixtures.
// Apache-2.0; see compat/provenance.json.
use std::collections::BTreeMap;
use timetuck::*;
struct Solution;
struct Owner;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Owner>(vec![
        PlanningVariableDefinition::list::<&'static str>("valueList", false),
    ])])
    .unwrap()
}
struct Harness<'s> {
    r: ListChangeRecorder<'s, &'static str, &'static str>,
    lists: BTreeMap<&'static str, Vec<&'static str>>,
}
impl<'s> Harness<'s> {
    fn new(
        s: &'s PlanningSchema,
        rows: impl IntoIterator<Item = (&'static str, Vec<&'static str>)>,
    ) -> Self {
        Self {
            r: ListChangeRecorder::new(s.entity::<Owner>().unwrap().variable("valueList").unwrap())
                .unwrap(),
            lists: rows.into_iter().collect(),
        }
    }
    fn before(&self, e: &'static str, from: usize, to: usize) {
        assert_eq!(
            self.r.before(e, from, to, &self.lists[e]).unwrap(),
            Some(ListChangeNotification::Before {
                variable: self.r.variable(),
                entity: e,
                from,
                to
            })
        );
    }
    fn after(&self, e: &'static str, from: usize, to: usize) {
        assert_eq!(
            self.r.after(e, from, to, &self.lists[e]).unwrap(),
            Some(ListChangeNotification::After {
                variable: self.r.variable(),
                entity: e,
                from,
                to
            })
        );
    }
    fn replace(&mut self, e: &'static str, from: usize, to: usize, values: &[&'static str]) {
        self.before(e, from, to);
        self.lists
            .get_mut(e)
            .unwrap()
            .splice(from..to, values.iter().copied());
        self.after(e, from, from + values.len());
    }
    // Expected order is reverse recording order, stronger than unordered mock counts.
    fn undo(&mut self, expected: &[(&'static str, usize, usize, usize)]) {
        let mut events = vec![];
        self.r
            .undo_changes(&mut self.lists, |e, _| events.push(e))
            .unwrap();
        let mut want = vec![];
        for &(entity, from, before_to, after_to) in expected {
            want.push(ListChangeNotification::Before {
                variable: self.r.variable(),
                entity,
                from,
                to: before_to,
            });
            want.push(ListChangeNotification::After {
                variable: self.r.variable(),
                entity,
                from,
                to: after_to,
            });
        }
        want.push(ListChangeNotification::UpdateShadows);
        assert_eq!(events, want);
    }
}
#[test]
fn same_list_single_element_change_merges_into_one_undo_notification() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0", "1", "2"])]);
    h.replace("e", 1, 2, &["new"]);
    h.undo(&[("e", 1, 2, 2)]);
    assert_eq!(h.lists["e"], ["0", "1", "2"]);
}

#[test]
fn cross_list_single_element_move_both_entities_merge_independently() {
    let s = schema();
    let mut h = Harness::new(&s, [("a", vec!["v0"]), ("b", vec![])]);
    h.replace("a", 0, 1, &[]);
    h.replace("b", 0, 0, &["v0"]);
    h.undo(&[("b", 0, 1, 0), ("a", 0, 0, 1)]);
    assert_eq!(h.lists["a"], ["v0"]);
    assert!(h.lists["b"].is_empty());
}

#[test]
fn cross_entity_swap_non_adjacent_brackets_still_merge() {
    let s = schema();
    let mut h = Harness::new(&s, [("l", vec!["left"]), ("r", vec!["right"])]);
    h.before("l", 0, 1);
    h.before("r", 0, 1);
    h.lists.get_mut("l").unwrap()[0] = "right";
    h.lists.get_mut("r").unwrap()[0] = "left";
    h.after("l", 0, 1);
    h.after("r", 0, 1);
    h.undo(&[("r", 0, 1, 1), ("l", 0, 1, 1)]);
    assert_eq!(h.lists["l"], ["left"]);
    assert_eq!(h.lists["r"], ["right"]);
}

#[test]
fn lifo_nested_brackets_still_merge_independently() {
    let s = schema();
    let mut h = Harness::new(&s, [("dest", vec!["dest"]), ("src", vec!["src"])]);
    h.before("dest", 0, 1);
    h.before("src", 0, 1);
    h.lists.get_mut("src").unwrap()[0] = "dest";
    h.after("src", 0, 1);
    h.lists.get_mut("dest").unwrap()[0] = "src";
    h.after("dest", 0, 1);
    h.undo(&[("src", 0, 1, 1), ("dest", 0, 1, 1)]);
    assert_eq!(h.lists["dest"], ["dest"]);
    assert_eq!(h.lists["src"], ["src"]);
}

#[test]
fn k_opt_style_batched_brackets_three_entities_each_merge_independently() {
    let s = schema();
    let mut h = Harness::new(
        &s,
        [("e1", vec!["1"]), ("e2", vec!["2"]), ("e3", vec!["3"])],
    );
    for e in ["e1", "e2", "e3"] {
        h.before(e, 0, 1);
    }
    for (e, v) in [("e1", "2"), ("e2", "3"), ("e3", "1")] {
        h.lists.get_mut(e).unwrap()[0] = v;
    }
    for e in ["e1", "e2", "e3"] {
        h.after(e, 0, 1);
    }
    h.undo(&[("e3", 0, 1, 1), ("e2", 0, 1, 1), ("e1", 0, 1, 1)]);
    for (e, v) in [("e1", "1"), ("e2", "2"), ("e3", "3")] {
        assert_eq!(h.lists[e], [v]);
    }
}

#[test]
fn same_entity_two_independent_brackets_do_not_cross_merge() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["1", "2"])]);
    h.replace("e", 0, 1, &[]);
    h.replace("e", 0, 0, &["3"]);
    h.undo(&[("e", 0, 1, 0), ("e", 0, 0, 1)]);
    assert_eq!(h.lists["e"], ["1", "2"]);
}

#[test]
fn same_entity_sequential_brackets_with_identical_indexes_undo_restores_original() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["v0", "v1"])]);
    h.replace("e", 0, 1, &["x"]);
    h.replace("e", 0, 1, &["y"]);
    h.undo(&[("e", 0, 1, 1), ("e", 0, 1, 1)]);
    assert_eq!(h.lists["e"], ["v0", "v1"]);
}

#[test]
fn same_entity_sequential_overlapping_brackets_undo_restores_original() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["v0", "v1", "v2", "v3"])]);
    h.replace("e", 1, 2, &["x"]);
    h.replace("e", 0, 3, &["p", "q"]);
    h.undo(&[("e", 0, 2, 3), ("e", 1, 2, 2)]);
    assert_eq!(h.lists["e"], ["v0", "v1", "v2", "v3"]);
}

#[test]
fn same_entity_sequential_brackets_touching_at_aboundary_undo_restores_original() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["v0", "v1", "v2", "v3"])]);
    h.replace("e", 0, 2, &["x"]);
    h.replace("e", 1, 3, &["y"]);
    h.undo(&[("e", 1, 2, 3), ("e", 0, 1, 2)]);
    assert_eq!(h.lists["e"], ["v0", "v1", "v2", "v3"]);
}

#[test]
fn get_non_delegating_shares_the_pending_tracker_across_instances() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0"])]);
    let n = h.r.non_delegating();
    h.before("e", 0, 1);
    h.lists.get_mut("e").unwrap()[0] = "new";
    assert_eq!(n.after("e", 0, 1, &h.lists["e"]).unwrap(), None);
    h.undo(&[("e", 0, 1, 1)]);
    assert_eq!(h.lists["e"], ["0"]);
}

#[test]
fn get_non_delegating_two_entities_pending_across_instances_both_still_merge() {
    let s = schema();
    let mut h = Harness::new(&s, [("l", vec!["left"]), ("r", vec!["right"])]);
    let n = h.r.non_delegating();
    h.before("l", 0, 1);
    h.before("r", 0, 1);
    h.lists.get_mut("l").unwrap()[0] = "right";
    h.lists.get_mut("r").unwrap()[0] = "left";
    for e in ["l", "r"] {
        assert_eq!(n.after(e, 0, 1, &h.lists[e]).unwrap(), None);
    }
    h.undo(&[("r", 0, 1, 1), ("l", 0, 1, 1)]);
    assert_eq!(h.lists["l"], ["left"]);
    assert_eq!(h.lists["r"], ["right"]);
}

#[test]
fn create_undo_move_while_abracket_is_still_open_is_retroactively_merged() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0"])]);
    h.before("e", 0, 1);
    let undo = h.r.undo_handle();
    h.lists.get_mut("e").unwrap()[0] = "x";
    h.after("e", 0, 1);
    let mut backing = vec![];
    undo.execute(&mut h.lists, |e, _| backing.push(e)).unwrap();
    assert_eq!(h.lists["e"], ["0"]);
    assert_eq!(backing.len(), 3);
}

#[test]
fn create_undo_move_before_any_change_is_recorded_still_sees_later_changes() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0"])]);
    let undo = h.r.undo_handle();
    h.before("e", 0, 1);
    h.lists.get_mut("e").unwrap()[0] = "x";
    h.after("e", 0, 1);
    let mut backing = vec![];
    undo.execute(&mut h.lists, |e, _| backing.push(e)).unwrap();
    assert_eq!(h.lists["e"], ["0"]);
    assert_eq!(backing.len(), 3);
}

#[test]
fn undo_changes_releases_the_pending_tracker_so_the_same_entity_can_open_afresh_bracket() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0"])]);
    h.replace("e", 0, 1, &["x"]);
    h.undo(&[("e", 0, 1, 1)]);
    h.before("e", 0, 1);
}

#[test]
fn second_bracket_for_an_open_entity_fails_fast() {
    let s = schema();
    let h = Harness::new(&s, [("e", vec!["v0", "v1", "v2", "v3"])]);
    h.before("e", 0, 4);
    let error = h.r.before("e", 1, 3, &h.lists["e"]).unwrap_err();
    assert_eq!(error, ListRecordingError::AlreadyOpen);
    assert!(error.to_string().contains("already has an open before"));
}

#[test]
fn orphaned_after_with_no_before_fails_fast() {
    let s = schema();
    let h = Harness::new(&s, [("e", vec!["0", "1"])]);
    let error = h.r.after("e", 0, 1, &h.lists["e"]).unwrap_err();
    assert_eq!(error, ListRecordingError::NoOpenBracket);
    assert!(error.to_string().contains("has no matching before"));
}

#[test]
fn duplicate_after_for_aclosed_bracket_fails_fast() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["v0", "v1"])]);
    h.replace("e", 0, 1, &["x"]);
    let error = h.r.after("e", 0, 1, &h.lists["e"]).unwrap_err();
    assert_eq!(error, ListRecordingError::NoOpenBracket);
    assert!(error.to_string().contains("has no matching before"));
}

#[test]
fn mismatched_after_from_index_throws() {
    let s = schema();
    let h = Harness::new(&s, [("e", vec!["0"])]);
    h.before("e", 0, 1);
    let error = h.r.after("e", 5, 1, &h.lists["e"]).unwrap_err();
    assert_eq!(
        error,
        ListRecordingError::WrongStart {
            expected: 0,
            actual: 5
        }
    );
    assert!(error.to_string().contains("must match its before"));
}

#[test]
fn get_non_delegating_drifted_from_index_across_instances_fails_fast() {
    let s = schema();
    let h = Harness::new(&s, [("e", vec!["v0", "v1", "v2"])]);
    let n = h.r.non_delegating();
    h.before("e", 1, 3);
    let error = n.after("e", 2, 3, &h.lists["e"]).unwrap_err();
    assert_eq!(
        error,
        ListRecordingError::WrongStart {
            expected: 1,
            actual: 2
        }
    );
    assert!(error.to_string().contains("must match its before"));
}

#[test]
fn after_under_reporting_the_length_change_fails_fast() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["v0", "v1", "v2"])]);
    h.before("e", 0, 2);
    h.lists.get_mut("e").unwrap().splice(0..2, ["x", "y", "z"]);
    let error = h.r.after("e", 0, 2, &h.lists["e"]).unwrap_err();
    assert_eq!(error, ListRecordingError::WrongLengthChange);
    assert!(error.to_string().contains("actually changed length by"));
}

#[test]
fn unclosed_bracket_at_undo_time_fails_fast() {
    let s = schema();
    let mut h = Harness::new(&s, [("e", vec!["0"])]);
    h.before("e", 0, 1);
    let error =
        h.r.undo_changes(&mut h.lists, |_, _| panic!("invalid undo notified backing"))
            .unwrap_err();
    assert_eq!(error, ListRecordingError::UnclosedBracket);
    assert!(error.to_string().contains("was never closed by its after"));
}
