//! Original publication-identity regressions: sparse refresh and strict fallback.
use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Route;
#[derive(Debug)]
struct Owner {
    id: usize,
    payload: i64,
    clones: Rc<Cell<usize>>,
}
impl Clone for Owner {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            payload: self.payload,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Owner {
    fn eq(&self, rhs: &Self) -> bool {
        self.id == rhs.id
    }
}
impl Eq for Owner {}
impl PartialOrd for Owner {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Owner {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.id.cmp(&rhs.id)
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Shadow(i64);
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Route>(vec![
            PlanningVariableDefinition::list::<u32>("visits", true),
            PlanningVariableDefinition::basic::<i32>("other", false),
        ]),
        PlanningEntityDefinition::of::<u32>(vec![PlanningVariableDefinition::shadow::<Shadow>(
            "shadow",
        )]),
    ])
    .unwrap()
}
fn values<E: Ord + Clone + 'static>(
    session: &ListShadowSession<'_, '_, E, u32, Shadow>,
) -> BTreeMap<u32, Option<i64>> {
    session
        .cached_values()
        .iter()
        .map(|(&k, v)| (k, v.value.as_ref().map(|h| h.0)))
        .collect()
}
#[test]
fn unchanged_publication_reads_and_leaf_notifications_clone_no_registry_owners() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("shadow").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    let clones = Rc::new(Cell::new(0));
    let owner = Owner {
        id: 0,
        payload: 0,
        clones: clones.clone(),
    };
    registry
        .reset_lists(source, [(owner, (1u32..=128).collect())])
        .unwrap();
    let fact = Cell::new(0);
    let calls = Cell::new(0);
    let mut session = ListShadowSession::new(
        registry.list::<Owner, u32>(source).unwrap(),
        target,
        1u32..=128,
        |c| {
            calls.set(calls.get() + 1);
            c.location.map(|_| {
                Shadow(
                    c.previous_shadow.map_or(0, |h: &Shadow| h.0)
                        + 1
                        + if *c.value == 128 { fact.get() } else { 0 },
                )
            })
        },
    )
    .unwrap();
    clones.set(0);
    calls.set(0);
    for _ in 0..100 {
        assert!(session.is_current());
        assert_eq!(
            session.values().unwrap()[&128].value.as_ref().unwrap().0,
            128
        );
        assert!(session.update_changed().unwrap().is_empty());
    }
    assert_eq!(clones.get(), 0);
    assert_eq!(calls.get(), 0);
    fact.set(7);
    session.mark_dirty(&128).unwrap();
    assert!(!session.is_current());
    assert_eq!(session.update_changed().unwrap(), [128]);
    assert_eq!(
        session.values().unwrap()[&128].value.as_ref().unwrap().0,
        135
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(clones.get(), 0);
}
#[test]
fn eq_equal_replacements_resets_and_restored_assignments_prepare_new_payloads() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("shadow").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    let clones = Rc::new(Cell::new(0));
    let owner = |payload| Owner {
        id: 0,
        payload,
        clones: clones.clone(),
    };
    registry
        .reset_lists(source, [(owner(10), vec![1u32, 2])])
        .unwrap();
    let mut session = ListShadowSession::new(
        registry.list::<Owner, u32>(source).unwrap(),
        target,
        [1u32, 2],
        |c| {
            c.location
                .map(|p| Shadow(p.entity.payload + c.previous_shadow.map_or(0, |h: &Shadow| h.0)))
        },
    )
    .unwrap();
    let saved = session.checkpoint();
    for (mode, payload) in [(0, 20), (1, 30), (2, 40)] {
        if mode == 1 {
            registry
                .reset_lists(source, [(owner(payload), vec![1u32, 2])])
                .unwrap();
        } else {
            registry
                .update_lists(source, [(owner(payload), vec![1u32, 2])])
                .unwrap();
        }
        if mode == 2 {
            session.restore(&saved).unwrap();
        } // Eq-by-ID permits restoration; publication still differs.
        assert!(session.is_current());
        session.mark_dirty(&1).unwrap();
        clones.set(0);
        session.update_changed().unwrap();
        assert!(
            clones.get() > 0,
            "new publication must prepare current owner payloads"
        );
        assert_eq!(
            values(&session),
            BTreeMap::from([(1, Some(payload)), (2, Some(2 * payload))])
        );
        clones.set(0);
        session.mark_dirty(&1).unwrap();
        session.update_changed().unwrap();
        assert_eq!(clones.get(), 0); // Now the exact publication is known.
    }
}
#[test]
fn failed_and_unrelated_field_publications_preserve_fast_path() {
    let schema = schema();
    let fields = schema.entity::<Route>().unwrap();
    let source = fields.variable("visits").unwrap();
    let target = schema.entity::<u32>().unwrap().variable("shadow").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    let clones = Rc::new(Cell::new(0));
    let owner = || Owner {
        id: 0,
        payload: 0,
        clones: clones.clone(),
    };
    registry
        .reset_lists(source, [(owner(), vec![1u32])])
        .unwrap();
    let mut session = ListShadowSession::new(
        registry.list::<Owner, u32>(source).unwrap(),
        target,
        [1u32],
        |_| Some(Shadow(1)),
    )
    .unwrap();
    assert!(
        registry
            .update_lists(source, [(owner(), vec![1u32, 1])])
            .is_err()
    );
    registry
        .reset_basic(fields.variable("other").unwrap(), [(0usize, Some(4i32))])
        .unwrap();
    clones.set(0);
    assert!(session.is_current());
    session.mark_dirty(&1).unwrap();
    session.update_changed().unwrap();
    assert_eq!(clones.get(), 0);
    // Successful equal publication still invalidates the proof, independently of tracker events.
    registry
        .update_lists(source, [(owner(), vec![1u32])])
        .unwrap();
    clones.set(0);
    session.update_changed().unwrap();
    assert!(clones.get() > 0);
    clones.set(0);
    session.update_changed().unwrap();
    assert_eq!(clones.get(), 0);
}
#[test]
fn failed_pending_refresh_keeps_values_notifications_and_publication_proof() {
    let schema = schema();
    let source = schema
        .entity::<Route>()
        .unwrap()
        .variable("visits")
        .unwrap();
    let target = schema.entity::<u32>().unwrap().variable("shadow").unwrap();
    let mut registry = VariableStateRegistry::new(&schema);
    let clones = Rc::new(Cell::new(0));
    registry
        .reset_lists(
            source,
            [(
                Owner {
                    id: 0,
                    payload: 0,
                    clones: clones.clone(),
                },
                vec![1u32, 2],
            )],
        )
        .unwrap();
    let fact = Cell::new(0);
    let fail = Cell::new(false);
    let mut session = ListShadowSession::new(
        registry.list::<Owner, u32>(source).unwrap(),
        target,
        [1u32, 2],
        |c| {
            assert!(!(fail.get() && *c.value == 2), "supplier");
            c.location
                .map(|_| Shadow(c.previous_shadow.map_or(0, |v: &Shadow| v.0) + fact.get() + 1))
        },
    )
    .unwrap();
    let before = values(&session);
    let saved = session.checkpoint();
    clones.set(0);
    fact.set(4);
    session.mark_dirty(&1).unwrap();
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.update_changed())).is_err());
    assert_eq!(values(&session), before);
    assert_eq!(session.pending_values().copied().collect::<Vec<_>>(), [1]);
    assert_eq!(clones.get(), 0);
    fail.set(false);
    session.update_changed().unwrap();
    assert_eq!(
        values(&session),
        BTreeMap::from([(1, Some(5)), (2, Some(10))])
    );
    assert_eq!(clones.get(), 0);
    session.restore(&saved).unwrap();
    fact.set(0);
    clones.set(0);
    assert!(session.values().is_ok());
    assert!(session.update_changed().unwrap().is_empty());
    assert_eq!(values(&session), before);
    assert_eq!(clones.get(), 0);
}
#[test]
fn sparse_notifications_and_structural_fallback_match_fresh_directional_references() {
    for direction in [ListCascadeDirection::Previous, ListCascadeDirection::Next] {
        let schema = schema();
        let source = schema
            .entity::<Route>()
            .unwrap()
            .variable("visits")
            .unwrap();
        let target = schema.entity::<u32>().unwrap().variable("shadow").unwrap();
        let mut registry = VariableStateRegistry::new(&schema);
        let mut row = (0u32..8).collect::<Vec<_>>();
        registry
            .reset_lists(source, [(0usize, row.clone())])
            .unwrap();
        let state = registry.list::<usize, u32>(source).unwrap();
        let facts = RefCell::new(vec![0i64; 8]);
        let supply = |c: ListShadowContext<'_, usize, u32, Shadow>| {
            c.location.map(|p| {
                Shadow(
                    p.index as i64
                        + facts.borrow()[*c.value as usize]
                        + c.previous_shadow
                            .or(c.next_shadow)
                            .map_or(0, |h: &Shadow| h.0),
                )
            })
        };
        let mut session = ListShadowSession::new_with_direction(
            state.clone(),
            target,
            0u32..8,
            direction,
            supply,
        )
        .unwrap();
        for step in 0..40 {
            let key = (step * 3 % 8) as u32;
            facts.borrow_mut()[key as usize] += 1;
            session.mark_dirty(&key).unwrap();
            if step % 5 == 0 {
                row.rotate_left(1);
                registry
                    .update_lists(source, [(0usize, row.clone())])
                    .unwrap();
            }
            session.update_changed().unwrap();
            let reference = ListShadowSession::new_with_direction(
                state.clone(),
                target,
                0u32..8,
                direction,
                supply,
            )
            .unwrap();
            assert_eq!(values(&session), values(&reference));
        }
    }
}
