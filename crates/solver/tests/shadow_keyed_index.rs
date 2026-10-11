//! Original indexed compiled-source lookup, diagnostics, and rollback contracts.
use std::{
    any::Any,
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::Arc,
};
use timetuck::*;
struct Model;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Root {
    id: usize,
    group: Vec<Arc<Root>>,
    other: Option<Arc<Other>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Other {
    id: usize,
}
#[derive(Default, Debug)]
struct Stats {
    equals: Cell<usize>,
    hashes: Cell<usize>,
    clones: Cell<usize>,
    supplies: Cell<usize>,
    hash_panic: Cell<Option<usize>>,
    constant: bool,
}
#[derive(Debug)]
struct Key {
    id: usize,
    stats: Rc<Stats>,
}
impl Clone for Key {
    fn clone(&self) -> Self {
        self.stats.clones.set(self.stats.clones.get() + 1);
        Self {
            id: self.id,
            stats: self.stats.clone(),
        }
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.stats.equals.set(self.stats.equals.get() + 1);
        self.id == other.id
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.stats.hashes.set(self.stats.hashes.get() + 1);
        assert_ne!(self.stats.hash_panic.get(), Some(self.id), "key hash panic");
        if self.stats.constant {
            0usize.hash(h)
        } else {
            self.id.hash(h)
        }
    }
}
fn root(id: usize, group: &[usize], other: usize) -> Root {
    Root {
        id,
        group: group
            .iter()
            .map(|&id| {
                Arc::new(Root {
                    id,
                    group: vec![],
                    other: None,
                })
            })
            .collect(),
        other: Some(Arc::new(Other { id: other })),
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![
        PlanningEntityDefinition::of::<Root>(vec![
            PlanningVariableDefinition::basic::<i64>("input", false),
            PlanningVariableDefinition::shadow::<i64>("output"),
        ]),
        PlanningEntityDefinition::of::<Other>(vec![PlanningVariableDefinition::basic::<i64>(
            "input", false,
        )]),
    ])
    .unwrap()
}
fn paths(s: &PlanningSchema) -> SourcePathRegistry<'_> {
    let mut p = SourcePathRegistry::new(s);
    p.many("group", SourceMemberRole::Fact, |r: &Root| {
        r.group.iter().map(Arc::as_ref).collect()
    })
    .unwrap();
    p.one("other", SourceMemberRole::Fact, |r: &Root| {
        r.other.as_deref()
    })
    .unwrap();
    p.one(
        "input",
        SourceMemberRole::Variable,
        |_: &Root| -> Option<&i64> { panic!("terminal getter") },
    )
    .unwrap();
    p.one(
        "input",
        SourceMemberRole::Variable,
        |_: &Other| -> Option<&i64> { panic!("terminal getter") },
    )
    .unwrap();
    p
}
fn identify(v: &dyn Any, stats: &Rc<Stats>) -> Option<Key> {
    let id = v
        .downcast_ref::<Root>()
        .map(|r| r.id)
        .or_else(|| v.downcast_ref::<Other>().map(|r| r.id))?;
    Some(Key {
        id,
        stats: stats.clone(),
    })
}
type Fixture<'a, 's> = (ShadowSession<'a, 's, Key, Vec<Root>>, Vec<ShadowNode<i64>>);
fn build<'a, 's: 'a>(
    s: &'s PlanningSchema,
    group: &'a CompiledSourcePath<'a, 's>,
    other: &'a CompiledSourcePath<'a, 's>,
    n: usize,
    indexed: bool,
    stats: &Rc<Stats>,
) -> Fixture<'a, 's> {
    let input = s.entity::<Root>().unwrap().variable("input").unwrap();
    let output = s.entity::<Root>().unwrap().variable("output").unwrap();
    let other_input = s.entity::<Other>().unwrap().variable("input").unwrap();
    let mut b = ShadowSessionBuilder::<Key, Vec<Root>>::new(s);
    if indexed {
        b = b.with_keyed_source_index();
    }
    let mut handles = vec![];
    for i in 0..n {
        let counter = stats.clone();
        let out = b
            .derive(
                output,
                Key {
                    id: i,
                    stats: stats.clone(),
                },
                move |_, deps| {
                    counter.supplies.set(counter.supplies.get() + 1);
                    Some(
                        deps.for_field::<i64>(input)
                            .unwrap()
                            .into_iter()
                            .chain(deps.for_field::<i64>(other_input).unwrap())
                            .flatten()
                            .copied()
                            .sum::<i64>(),
                    )
                },
            )
            .unwrap();
        let identify_stats = stats.clone();
        b.bind_source_paths(
            &out,
            vec![group, other],
            move |w| &w[i],
            move |v| identify(v, &identify_stats),
        )
        .unwrap();
        handles.push(out);
    }
    // Index must include registrations after both opt-in and source binding.
    for i in 0..n {
        b.input(
            input,
            Key {
                id: i,
                stats: stats.clone(),
            },
            move |_| Some(i as i64 + 1),
        )
        .unwrap();
        b.input(
            other_input,
            Key {
                id: i,
                stats: stats.clone(),
            },
            move |_| Some(1000 + i as i64),
        )
        .unwrap();
    }
    stats.clones.set(0);
    let s = b.build();
    assert_eq!(
        stats.clones.get(),
        0,
        "index construction must not clone keys"
    );
    (s, handles)
}
fn verify(s: &ShadowSession<'_, '_, Key, Vec<Root>>, h: &[ShadowNode<i64>], w: &[Root]) {
    for (node, root) in h.iter().zip(w) {
        let group: std::collections::BTreeSet<_> = root.group.iter().map(|r| r.id).collect();
        let expected = group.into_iter().map(|i| i as i64 + 1).sum::<i64>()
            + root.other.as_ref().map_or(0, |r| 1000 + r.id as i64);
        let value = s.read(node).unwrap();
        assert!(!value.inconsistent);
        assert_eq!(value.value, Some(&expected));
    }
}
#[test]
fn indexed_and_eq_only_paths_match_with_late_fields_collisions_and_duplicate_order() {
    let schema = schema();
    let paths = paths(&schema);
    let group = paths.compile::<Root>("output", "group[].input").unwrap();
    let other = paths.compile::<Root>("output", "other.input").unwrap();
    for constant in [false, true] {
        let indexed_stats = Rc::new(Stats {
            constant,
            ..Stats::default()
        });
        let plain_stats = Rc::new(Stats::default());
        let (mut indexed, ih) = build(&schema, &group, &other, 40, true, &indexed_stats);
        let (mut plain, ph) = build(&schema, &group, &other, 40, false, &plain_stats);
        let mut w: Vec<_> = (0..40).map(|i| root(i, &[i, (i + 1) % 40, i], i)).collect();
        indexed.update(&w).unwrap();
        plain.update(&w).unwrap();
        let saved = indexed.checkpoint().unwrap();
        let initial = w.clone();
        for step in 0..32 {
            let i = step % 40;
            w[i] = root(i, &[(i + 2) % 40, (i + 1) % 40, (i + 2) % 40], (i + 3) % 40);
            indexed.mark_dirty(&ih[i]).unwrap();
            plain.mark_dirty(&ph[i]).unwrap();
            indexed.update_notified(&w).unwrap();
            plain.update_notified(&w).unwrap();
            verify(&indexed, &ih, &w);
            verify(&plain, &ph, &w);
        }
        indexed.restore(&saved, &initial).unwrap();
        verify(&indexed, &ih, &initial);
        let mut reordered = initial.clone();
        reordered[0] = root(0, &[1, 0, 1], 0);
        indexed.mark_dirty(&ih[0]).unwrap();
        let before = indexed_stats.supplies.get();
        assert!(indexed.update_notified(&reordered).unwrap().is_empty());
        assert_eq!(indexed_stats.supplies.get(), before + 1);
        assert!(matches!(
            indexed.restore(&saved, &reordered),
            Err(ShadowSessionError::SnapshotMismatch)
        ));
        indexed.restore(&saved, &initial).unwrap();
        verify(&indexed, &ih, &initial);
    }
}
#[test]
fn indexed_lookup_bounds_eq_work_and_hash_panics_preserve_ready_and_pending_states() {
    let schema = schema();
    let paths = paths(&schema);
    let group = paths.compile::<Root>("output", "group[].input").unwrap();
    let other = paths.compile::<Root>("output", "other.input").unwrap();
    let stats = Rc::new(Stats::default());
    let (mut s, h) = build(&schema, &group, &other, 256, true, &stats);
    let mut w: Vec<_> = (0..256).map(|i| root(i, &[255], 255)).collect();
    stats.equals.set(0);
    stats.hashes.set(0);
    s.update(&w).unwrap();
    assert_eq!(stats.hashes.get(), 512);
    assert!(
        stats.equals.get() <= 768,
        "root identity plus at most one Eq per found source"
    );
    let saved = s.checkpoint().unwrap();
    let original = w.clone();
    w[0] = root(0, &[254], 254);
    s.mark_dirty(&h[0]).unwrap();
    stats.hash_panic.set(Some(254));
    assert!(catch_unwind(AssertUnwindSafe(|| s.update_notified(&w))).is_err());
    assert!(matches!(s.read(&h[0]), Err(ShadowSessionError::NotCurrent)));
    stats.hash_panic.set(None);
    stats.equals.set(0);
    stats.hashes.set(0);
    s.update_notified(&w).unwrap();
    assert_eq!(stats.hashes.get(), 2);
    assert!(stats.equals.get() <= 3);
    verify(&s, &h, &w);
    s.restore(&saved, &original).unwrap();
    stats.hash_panic.set(Some(254));
    let calls = stats.supplies.get();
    assert!(catch_unwind(AssertUnwindSafe(|| missing_shadow_sources(&s, &w))).is_err());
    assert_eq!(stats.supplies.get(), calls);
    verify(&s, &h, &original);
    stats.hash_panic.set(None);
}
#[test]
fn indexed_missing_sources_keep_order_field_identity_and_manager_publication_atomic() {
    let schema = schema();
    let paths = paths(&schema);
    let group = paths.compile::<Root>("output", "group[].input").unwrap();
    let other = paths.compile::<Root>("output", "other.input").unwrap();
    let stats = Rc::new(Stats {
        constant: true,
        ..Stats::default()
    });
    let (s, h) = build(&schema, &group, &other, 40, true, &stats);
    let projected = h.clone();
    let mut manager = ShadowSolutionManager::new(
        s,
        move |_, s| {
            Ok::<_, ()>(
                projected
                    .iter()
                    .map(|h| *s.read(h).unwrap().value.unwrap())
                    .collect::<Vec<_>>(),
            )
        },
        |_, _| Ok(()),
        |_, values| Ok(values.iter().sum::<i64>()),
    );
    let mut solution = ManagedSolution::new((0..40).map(|i| root(i, &[i], i)).collect(), vec![]);
    manager.update_notified(&mut solution).unwrap();
    let old = solution.shadows.clone();
    let score = solution.score;
    solution.assignment[0] = root(0, &[90, 80, 90], 90);
    solution.assignment[1] = root(1, &[80], 80);
    manager.session_mut().mark_dirty(&h[0]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing sources")
    };
    let description: Vec<_> = missing
        .iter()
        .map(|r| (r.target.entity.id, r.source.entity.id, r.source.entity_type))
        .collect();
    assert_eq!(
        description,
        [
            (0, 90, PlanningType::of::<Root>()),
            (0, 80, PlanningType::of::<Root>()),
            (0, 90, PlanningType::of::<Other>())
        ]
    );
    assert_eq!(solution.shadows, old);
    assert_eq!(solution.score, score);
    let all = missing_shadow_sources(manager.session(), &solution.assignment).unwrap();
    assert_eq!(all.len(), 5);
    manager.session_mut().mark_dirty(&h[1]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing sources")
    };
    assert_eq!(missing, all);
    solution.assignment[0] = root(0, &[0], 0);
    solution.assignment[1] = root(1, &[1], 1);
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(solution.shadows, old);
    assert_eq!(solution.score, score);
}
#[test]
fn default_eq_only_keys_and_opted_in_nonclone_keys_keep_native_handles_direct() {
    #[derive(Eq, PartialEq)]
    struct EqOnly(usize);
    let schema = schema();
    let field = schema.entity::<Root>().unwrap().variable("input").unwrap();
    let mut b = ShadowSessionBuilder::<EqOnly, ()>::new(&schema);
    let h = b.input(field, EqOnly(0), |_| Some(7i64)).unwrap();
    let mut s = b.build();
    s.update(&()).unwrap();
    assert_eq!(s.read(&h).unwrap().value, Some(&7));
    #[derive(Eq, PartialEq, Hash)]
    struct NonClone(usize);
    let mut b = ShadowSessionBuilder::<NonClone, ()>::new(&schema).with_keyed_source_index();
    let h = b.input(field, NonClone(0), |_| Some(9i64)).unwrap();
    let mut s = b.build();
    s.update(&()).unwrap();
    assert_eq!(s.read(&h).unwrap().value, Some(&9));
}
