//! Original multiworld registration churn against a direct final-world reference.
use std::{cell::Cell, collections::BTreeMap};
use timetuck::*;
struct Model;
struct Task;
#[test]
fn multiworld_membership_churn_and_rollback_match_direct_projection() {
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
    ])])
    .unwrap();
    let calls = Cell::new(0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let output = builder
        .map_dynamic_shadows(
            source.port(),
            |world| world.0,
            |membership| {
                calls.set(calls.get() + 1);
                let mut session = ShadowSessionBuilder::<usize, (usize, i64)>::new(&schema);
                let field = session.input(
                    schema.entity::<Task>().unwrap().variable("value").unwrap(),
                    *membership,
                    |world| Some(world.1),
                )?;
                let membership = *membership as i64;
                ShadowProgram::new(session, move |_, session| {
                    Ok(*session.read(&field)?.value.unwrap() + membership)
                })
            },
        )
        .unwrap();
    let rows = builder.dataset(&output).unwrap();
    builder
        .score("values", &output, |v| SimpleScore::new(*v))
        .unwrap();
    let mut graph = builder.build();
    let mut expected = BTreeMap::new();
    for id in 0..128 {
        let world = (id % 7, id as i64);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(id, world)).unwrap();
        tx.commit().unwrap();
        expected.insert(id, world);
    }
    assert_eq!(calls.get(), 128);
    for step in 0..512 {
        let id = (step * 37) % 128;
        let before = calls.get();
        let mut next = expected.clone();
        let change = if step % 5 == 0 {
            next.remove(&id);
            TupleChange::retract(id)
        } else {
            let world = if step % 3 == 0 {
                let membership = expected.get(&id).map_or(1, |w| w.0);
                (membership, -(step as i64))
            } else {
                (step + 100, step as i64)
            };
            next.insert(id, world);
            TupleChange::put(id, world)
        };
        let rebuild = next
            .get(&id)
            .is_some_and(|w| expected.get(&id).is_none_or(|old| old.0 != w.0));
        let score = |worlds: &BTreeMap<usize, (usize, i64)>| {
            SimpleScore::new(worlds.values().map(|w| w.0 as i64 + w.1).sum())
        };
        let mut tx = graph.transaction();
        tx.apply(&source, change).unwrap();
        assert_eq!(tx.score().unwrap(), score(&next));
        assert_eq!(calls.get(), before + usize::from(rebuild));
        if step % 4 == 0 {
            drop(tx);
        } else {
            tx.commit().unwrap();
            expected = next;
        }
        assert_eq!(graph.score(), score(&expected));
        assert_eq!(graph.source_rows(&source).unwrap(), &expected);
        assert_eq!(
            graph.dataset_rows(&rows).unwrap(),
            &expected
                .iter()
                .map(|(id, w)| (*id, w.0 as i64 + w.1))
                .collect::<BTreeMap<_, _>>()
        );
    }
    graph.assert_consistent().unwrap();
}

#[test]
fn failed_state_publication_retires_the_unpublished_program() {
    use std::{
        cmp::Ordering,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    #[derive(Clone)]
    struct Key(Rc<Cell<bool>>);
    impl PartialEq for Key {
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
            Some(self.cmp(rhs))
        }
    }
    impl Ord for Key {
        fn cmp(&self, _: &Self) -> Ordering {
            assert!(!self.0.get(), "state publication comparison panic");
            Ordering::Equal
        }
    }
    struct Live<'a>(&'a Cell<usize>);
    impl Live<'_> {
        fn alive(&self) -> bool {
            self.0.get() > 0
        }
    }
    impl Drop for Live<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() - 1);
        }
    }
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
    ])])
    .unwrap();
    let fail = Rc::new(Cell::new(false));
    let live = Cell::new(0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<Key, usize>();
    let output = builder
        .map_dynamic_shadows(
            source.port(),
            |world| *world,
            |membership| {
                live.set(live.get() + 1);
                let guard = Live(&live);
                let mut session = ShadowSessionBuilder::<usize, usize>::new(&schema);
                let field = session.input(
                    schema.entity::<Task>().unwrap().variable("value").unwrap(),
                    *membership,
                    |w| Some(*w as i64),
                )?;
                let program = ShadowProgram::new(session, move |_, session| {
                    assert!(guard.alive());
                    Ok(*session.read(&field)?.value.unwrap())
                })?;
                fail.set(*membership == 99);
                Ok(program)
            },
        )
        .unwrap();
    builder
        .score("value", &output, |v| SimpleScore::new(*v))
        .unwrap();
    let mut graph = builder.build();
    let key = Key(fail.clone());
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(key.clone(), 0)).unwrap();
    tx.commit().unwrap();
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(key.clone(), 99))
            ))
            .is_err()
        );
        fail.set(false);
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    }
    assert_eq!(graph.score(), SimpleScore::ZERO);
    assert_eq!(live.get(), 2); // Old live registration and unpublished retired slot.
    for membership in 1..32 {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(key.clone(), membership))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(graph.score(), SimpleScore::new(membership as i64));
        assert_eq!(live.get(), 2); // Current plus prior checkpoint's deferred retirement.
    }
    drop(graph);
    assert_eq!(live.get(), 0);
}
