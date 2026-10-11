//! Original membership-driven shadow registration and stream regressions.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Model;
struct Task;
#[derive(Eq, PartialEq)]
struct Amount(i64); // Session values need not implement Clone.
#[derive(Eq, PartialEq)]
struct Membership(Vec<usize>); // Signatures need not implement Clone.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Row {
    value: i64,
    parent: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct World {
    rows: BTreeMap<usize, Row>,
    factor: i64,
}
type Output = Vec<(usize, Option<i64>, bool)>;
#[derive(Default)]
struct Stats {
    factories: Cell<usize>,
    reads: Cell<usize>,
    suppliers: Cell<usize>,
    projects: Cell<usize>,
    live: Cell<usize>,
    failure: Cell<u8>,
}
struct Registration<'a>(&'a Cell<usize>);
impl Registration<'_> {
    fn alive(&self) -> bool {
        self.0.get() > 0
    }
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
        PlanningVariableDefinition::basic::<i64>("factor", false),
        PlanningVariableDefinition::shadow::<Amount>("amount"),
    ])])
    .unwrap()
}
fn world(values: &[(usize, i64, Option<usize>)]) -> World {
    World {
        rows: values
            .iter()
            .map(|&(id, value, parent)| (id, Row { value, parent }))
            .collect(),
        factor: 1,
    }
}
fn program<'a, 's: 'a>(
    schema: &'s PlanningSchema,
    signature: &Membership,
    stats: &'a Stats,
) -> Result<ShadowProgram<'a, 's, usize, World, Output>, ShadowSessionError> {
    stats.factories.set(stats.factories.get() + 1);
    stats.live.set(stats.live.get() + 1);
    let registration = Registration(&stats.live);
    if stats.failure.get() == 1 {
        return Err(ShadowSessionError::Metadata("factory failure"));
    }
    assert!(stats.failure.get() != 2, "factory panic");
    let fields = schema.entity::<Task>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, World>::new(schema);
    let mut nodes = BTreeMap::new();
    let mut sources = BTreeMap::new();
    for &key in &signature.0 {
        let value = b.input(fields.variable("value").unwrap(), key, move |w| {
            stats.reads.set(stats.reads.get() + 1);
            assert!(stats.failure.get() != 3, "reader panic");
            Some(w.rows[&key].value)
        })?;
        let factor = b.input(fields.variable("factor").unwrap(), key, move |w| {
            Some(w.factor)
        })?;
        let v = value.clone();
        let f = factor.clone();
        let amount = b.derive(fields.variable("amount").unwrap(), key, move |_, i| {
            stats.suppliers.set(stats.suppliers.get() + 1);
            assert!(stats.failure.get() != 5, "supplier panic");
            let own =
                i.get(&v).unwrap().copied().unwrap_or(0) * i.get(&f).unwrap().copied().unwrap_or(0);
            Some(Amount(
                own + i
                    .for_field::<Amount>(fields.variable("amount").unwrap())
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .map(|a| a.0)
                    .sum::<i64>(),
            ))
        })?;
        nodes.insert(key, amount);
        sources.insert(key, vec![value.erase(), factor.erase()]);
    }
    for (&key, node) in &nodes {
        let nodes = nodes.clone();
        let basic = sources[&key].clone();
        b.bind_dependencies(node, move |w| {
            if stats.failure.get() == 4 {
                return Err(ShadowSessionError::UnregisteredSource);
            }
            let mut inputs = basic.clone();
            if let Some(parent) = w.rows[&key].parent {
                inputs.push(
                    nodes
                        .get(&parent)
                        .ok_or(ShadowSessionError::UnregisteredSource)?
                        .erase(),
                );
            }
            Ok(inputs)
        })?;
    }
    ShadowProgram::new(b, move |_, session| {
        assert!(registration.alive());
        stats.projects.set(stats.projects.get() + 1);
        if stats.failure.get() == 6 {
            return Err(ShadowSessionError::Metadata("projection failure"));
        }
        assert!(stats.failure.get() != 7, "projection panic");
        nodes
            .iter()
            .map(|(&key, node)| {
                let read = session.read(node)?;
                Ok((key, read.value.map(|v| v.0), read.inconsistent))
            })
            .collect()
    })
}
fn graph<'a, 's: 'a>(
    schema: &'s PlanningSchema,
    stats: &'a Stats,
) -> (
    StreamGraph<'a, SimpleScore>,
    StreamSource<usize, World>,
    StreamDataset<usize, Output>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, World>();
    let out = b
        .map_dynamic_shadows(
            source.port(),
            |w| Membership(w.rows.keys().copied().collect()),
            move |m| program(schema, m, stats),
        )
        .unwrap();
    let dataset = b.dataset(&out).unwrap();
    b.score("sum", &out, |rows| {
        SimpleScore::new(rows.iter().map(|(_, v, _)| v.unwrap_or(0)).sum())
    })
    .unwrap();
    (b.build(), source, dataset)
}
fn apply(
    graph: &mut StreamGraph<'_, SimpleScore>,
    source: &StreamSource<usize, World>,
    id: usize,
    world: World,
) {
    let mut tx = graph.transaction();
    tx.apply(source, TupleChange::put(id, world)).unwrap();
    tx.commit().unwrap();
}
#[test]
fn membership_changes_rebuild_once_while_values_and_facts_reuse_each_world_cache() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, &stats);
    let a = world(&[(1, 2, None), (3, 4, Some(1))]);
    apply(&mut g, &s, 7, a.clone());
    apply(&mut g, &s, 8, a.clone());
    assert_eq!(stats.factories.get(), 2);
    assert_eq!(stats.suppliers.get(), 4);
    assert_eq!(g.score(), SimpleScore::new(16));
    apply(&mut g, &s, 7, a.clone());
    assert_eq!(stats.factories.get(), 2);
    assert_eq!(stats.suppliers.get(), 4);
    let mut a = a;
    a.factor = 2;
    apply(&mut g, &s, 7, a.clone());
    assert_eq!(stats.factories.get(), 2);
    assert_eq!(stats.suppliers.get(), 6);
    assert_eq!(g.score(), SimpleScore::new(24));
    // A removal plus insertion in one complete world builds once, retaining output ID.
    a.rows.remove(&1);
    a.rows.insert(
        0,
        Row {
            value: 5,
            parent: None,
        },
    );
    a.rows.get_mut(&3).unwrap().parent = Some(0);
    apply(&mut g, &s, 7, a.clone());
    assert_eq!(stats.factories.get(), 3);
    assert_eq!(stats.suppliers.get(), 8);
    assert_eq!(
        g.dataset_rows(&d)
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [7, 8]
    );
    assert_eq!(
        g.dataset_rows(&d).unwrap()[&7],
        [(0, Some(10), false), (3, Some(18), false)]
    );
    a.rows.get_mut(&3).unwrap().value = 1;
    apply(&mut g, &s, 7, a);
    assert_eq!(stats.factories.get(), 3);
    assert_eq!(stats.suppliers.get(), 9);
    // Retraction itself runs no program callbacks; reintroduction initializes fresh.
    let counts = (
        stats.factories.get(),
        stats.reads.get(),
        stats.suppliers.get(),
        stats.projects.get(),
    );
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(7)).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        (
            stats.factories.get(),
            stats.reads.get(),
            stats.suppliers.get(),
            stats.projects.get()
        ),
        counts
    );
    apply(&mut g, &s, 7, world(&[(1, 2, None)]));
    assert_eq!(stats.factories.get(), 4);
    g.assert_consistent().unwrap();
}
#[test]
fn switching_registrations_retracting_and_undoing_preserve_prior_programs() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, &stats);
    let initial = world(&[(1, 2, None), (3, 4, Some(1))]);
    apply(&mut g, &s, 7, initial.clone());
    let before = g.dataset_rows(&d).unwrap().clone();
    let sources = g.source_rows(&s).unwrap().clone();
    let count;
    {
        let mut tx = g.transaction();
        for key in 10..30 {
            tx.apply(&s, TupleChange::put(7, world(&[(key, key as i64, None)])))
                .unwrap();
            assert!(stats.live.get() <= 3, "unbounded registration retention");
        }
        tx.apply(&s, TupleChange::retract(7)).unwrap();
        count = (
            stats.factories.get(),
            stats.reads.get(),
            stats.suppliers.get(),
            stats.projects.get(),
        );
    }
    assert_eq!(
        (
            stats.factories.get(),
            stats.reads.get(),
            stats.suppliers.get(),
            stats.projects.get()
        ),
        count
    );
    assert_eq!(g.dataset_rows(&d).unwrap(), &before);
    assert_eq!(g.source_rows(&s).unwrap(), &sources);
    apply(&mut g, &s, 7, initial);
    assert_eq!(stats.factories.get(), count.0);
    assert_eq!(stats.suppliers.get(), count.2);
    apply(&mut g, &s, 7, world(&[(99, 1, None)]));
    assert!(stats.live.get() <= 2);
    g.assert_consistent().unwrap();
    drop(g);
    assert_eq!(stats.live.get(), 0);
}
#[test]
fn factory_binding_reader_supplier_and_projection_failures_never_publish_partial_scores() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, &stats);
    let initial = world(&[(1, 2, None)]);
    apply(&mut g, &s, 7, initial.clone());
    let before = g.dataset_rows(&d).unwrap().clone();
    let sources = g.source_rows(&s).unwrap().clone();
    for changed_membership in [false, true] {
        for failure in 1..=7 {
            if !changed_membership && failure <= 2 {
                continue;
            }
            stats.failure.set(failure);
            let target = if changed_membership {
                world(&[(9, 7, None)])
            } else {
                world(&[(1, 8, None)])
            };
            let mut tx = g.transaction();
            if [1, 4, 6].contains(&failure) {
                assert!(tx.apply(&s, TupleChange::put(7, target)).is_err());
            } else {
                assert!(
                    catch_unwind(AssertUnwindSafe(
                        || tx.apply(&s, TupleChange::put(7, target))
                    ))
                    .is_err()
                );
            }
            assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
            drop(tx);
            assert_eq!(g.dataset_rows(&d).unwrap(), &before);
            assert_eq!(g.source_rows(&s).unwrap(), &sources);
            assert_eq!(g.score(), SimpleScore::new(2));
            assert_eq!(stats.live.get(), 1, "unpublished registration leaked");
            stats.failure.set(0);
            apply(&mut g, &s, 7, initial.clone());
            assert_eq!(g.dataset_rows(&d).unwrap(), &before);
        }
    }
    g.assert_consistent().unwrap();
}
#[test]
fn runtime_cycles_and_new_members_match_fresh_registration_results() {
    let schema = schema();
    let stats = Stats::default();
    let (mut g, s, d) = graph(&schema, &stats);
    let worlds = [
        world(&[(1, 1, None), (2, 2, Some(1))]),
        world(&[(1, 1, Some(2)), (2, 2, Some(1))]),
        world(&[(1, 1, Some(2)), (2, 2, Some(1)), (0, 3, Some(1))]),
        world(&[(1, 1, None), (0, 3, Some(1))]),
        world(&[]),
    ];
    for w in worlds {
        apply(&mut g, &s, 7, w.clone());
        let fresh_stats = Stats::default();
        let (mut fresh, fresh_source, fresh_data) = graph(&schema, &fresh_stats);
        apply(&mut fresh, &fresh_source, 7, w);
        assert_eq!(
            g.dataset_rows(&d).unwrap(),
            fresh.dataset_rows(&fresh_data).unwrap()
        );
        assert_eq!(g.score(), fresh.score());
    }
    assert_eq!(stats.factories.get(), 4); // The second world changed dependencies only.
    assert_eq!(g.dataset_rows(&d).unwrap().len(), 1);
    assert!(g.dataset_rows(&d).unwrap()[&7].is_empty());
    g.assert_consistent().unwrap();
}
