// Adapted from pinned Community BavetNodeDeactivationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone, Copy)]
enum Case {
    Join(bool),
    Exists(bool),
    Concat,
    Group,
    Flatten,
    Shared,
    Precompute(bool),
}
fn settle<I, T>(
    b: StreamGraphBuilder<'static, SimpleScore>,
    entities: StreamSource<usize, i64>,
    extras: StreamSource<usize, i64>,
    out: StreamPort<I, T>,
    extra: bool,
) -> (StreamActivity, NodeActivity) {
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..7 {
        tx.apply(&entities, TupleChange::put(id, id as i64))
            .unwrap();
    }
    if extra {
        tx.apply(&extras, TupleChange::put(0, 0)).unwrap();
    }
    tx.commit().unwrap();
    let activity = g.activity();
    let output = activity.get(&out).unwrap();
    (activity, output)
}
fn run(case: Case) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let entities = b.source::<usize, i64>();
    let extras = b.source::<usize, i64>();
    let entity_port = entities.port().clone();
    let extra_port = extras.port().clone();
    match case {
        Case::Join(_) | Case::Shared => {
            let populated = matches!(case, Case::Join(true));
            let shared = matches!(case, Case::Shared);
            let join = b
                .join_equal(entities.port(), extras.port(), |_| (), |_| ())
                .unwrap();
            b.score("join", &join, |_| SimpleScore::new(-1)).unwrap();
            if shared {
                b.score("alive", entities.port(), |_| SimpleScore::new(-1))
                    .unwrap();
            }
            let (a, out) = settle(b, entities, extras, join, populated);
            assert_eq!(out.active, populated);
            assert_eq!(a.get(&entity_port).unwrap().active, populated || shared);
            assert_eq!(a.get(&extra_port).unwrap().active, populated);
            // Two declared roots, one join, one or two terminal consumers.
            assert_eq!(a.node_count(), if shared { 5 } else { 4 });
        }
        Case::Exists(exists) => {
            let out = b
                .existence_equal(
                    entities.port(),
                    extras.port(),
                    if exists {
                        ExistenceMode::Exists
                    } else {
                        ExistenceMode::NotExists
                    },
                    |_| (),
                    |_| (),
                )
                .unwrap();
            b.score("exists", &out, |_| SimpleScore::new(-1)).unwrap();
            let (a, out) = settle(b, entities, extras, out, false);
            assert_eq!(out.active, !exists);
            assert_eq!(a.get(&entity_port).unwrap().active, !exists);
            assert!(!a.get(&extra_port).unwrap().active);
        }
        Case::Concat => {
            let l = b.map(extras.port(), |v| *v).unwrap();
            let r = b.map(entities.port(), |v| *v).unwrap();
            let out = b.concat(&l, &r).unwrap();
            b.score("concat", &out, |_| SimpleScore::new(-1)).unwrap();
            let (a, out) = settle(b, entities, extras, out, false);
            assert!(out.active);
            assert!(a.get(&entity_port).unwrap().active);
            assert!(!a.get(&extra_port).unwrap().active);
        }
        Case::Group => {
            let out = b
                .collect::<_, _, _, CountCollector>(extras.port(), |_| ())
                .unwrap();
            b.score("count", &out, |_| SimpleScore::new(-1)).unwrap();
            let (a, out) = settle(b, entities, extras, out, false);
            assert!(!out.active);
            assert!(!a.get(&extra_port).unwrap().active);
        }
        Case::Flatten => {
            let out = b.flat_map(extras.port(), |v| vec![*v]).unwrap();
            b.score("flatten", &out, |_| SimpleScore::new(-1)).unwrap();
            let (a, out) = settle(b, entities, extras, out, false);
            assert!(!out.active);
            assert!(!a.get(&extra_port).unwrap().active);
        }
        Case::Precompute(populated) => {
            let out = b.precompute(extras.port(), |v| *v, |v| *v).unwrap();
            b.score("precompute", &out, |_| SimpleScore::new(-1))
                .unwrap();
            let (_, out) = settle(b, entities, extras, out, populated);
            assert_eq!(out.active, populated);
        }
    }
}
#[test]
fn empty_join_side_deactivates_chain() {
    run(Case::Join(false));
}
#[test]
fn non_empty_join_side_keeps_chain_active() {
    run(Case::Join(true));
}
#[test]
fn if_not_exists_with_empty_right_stays_active() {
    run(Case::Exists(false));
}
#[test]
fn if_exists_with_empty_right_deactivates() {
    run(Case::Exists(true));
}
#[test]
fn concat_with_one_empty_side_stays_active() {
    run(Case::Concat);
}
#[test]
fn group_by_over_empty_class_deactivates() {
    run(Case::Group);
}
#[test]
fn flatten_over_empty_class_deactivates() {
    run(Case::Flatten);
}
#[test]
fn shared_for_each_stays_active_when_one_branch_is_alive() {
    run(Case::Shared);
}
#[test]
fn precompute_over_empty_source_deactivates() {
    run(Case::Precompute(false));
}
#[test]
fn precompute_over_populated_source_stays_active() {
    run(Case::Precompute(true));
}
