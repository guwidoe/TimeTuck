//! Original shared-checkpoint and sparse-refresh regressions; no source credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Entity;
fn schema<T: 'static>() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<T>("input", false),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap()
}
struct Counted(i64, Rc<Cell<usize>>);
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.1.set(self.1.get() + 1);
        self.0 == other.0
    }
}
impl Eq for Counted {}
#[test]
fn sparse_input_comparisons_and_snapshot_creation_do_not_walk_all_values() {
    let schema = schema::<Counted>();
    let entity = schema.entity::<Entity>().unwrap();
    let comparisons = Rc::new(Cell::new(0));
    let reads = Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<i64>>::new(&schema);
    let inputs: Vec<_> = (0..256)
        .map(|i| {
            let comparisons = comparisons.clone();
            let reads = &reads;
            builder
                .input(entity.variable("input").unwrap(), i, move |world| {
                    reads.set(reads.get() + 1);
                    Some(Counted(world[i], comparisons.clone()))
                })
                .unwrap()
        })
        .collect();
    let mut session = builder.build();
    let mut world = vec![0; 256];
    session.update_notified(&world).unwrap();
    assert_eq!(reads.get(), 256);
    comparisons.set(0);
    reads.set(0);
    let saved = session.checkpoint().unwrap();
    for _ in 0..100 {
        std::hint::black_box(saved.clone());
    }
    assert_eq!(comparisons.get(), 0);
    assert_eq!(reads.get(), 0);
    world[128] = 9;
    session.mark_dirty(&inputs[128]).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(reads.get(), 1);
    assert_eq!(comparisons.get(), 2);
    assert_eq!(session.read(&inputs[128]).unwrap().value.unwrap().0, 9);
    comparisons.set(0);
    reads.set(0);
    session.update(&world).unwrap();
    assert_eq!(reads.get(), 256);
    assert_eq!(comparisons.get(), 256);
    world[128] = 0;
    session.restore(&saved, &world).unwrap();
    assert_eq!(session.read(&inputs[128]).unwrap().value.unwrap().0, 0);
}
#[derive(Clone)]
struct World {
    values: [i64; 2],
    selected: usize,
}
#[test]
fn bindings_manual_rewires_cycles_and_restore_use_the_correct_cached_snapshot() {
    let schema = schema::<i64>();
    let fields = schema.entity::<Entity>().unwrap();
    let input_field = fields.variable("input").unwrap();
    let output_field = fields.variable("output").unwrap();
    let resolves = Cell::new(0);
    let reads = Cell::new(0);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let inputs: Vec<_> = (0..2)
        .map(|i| {
            let reads = &reads;
            b.input(input_field, i, move |w| {
                reads.set(reads.get() + 1);
                Some(w.values[i])
            })
            .unwrap()
        })
        .collect();
    let out = b
        .derive(output_field, 0, move |_, deps| {
            Some(*deps.for_field::<i64>(input_field).unwrap()[0].unwrap())
        })
        .unwrap();
    let refs: Vec<_> = inputs.iter().map(ShadowNode::erase).collect();
    let resolver_calls = &resolves;
    b.bind_dependencies(&out, move |w| {
        resolver_calls.set(resolver_calls.get() + 1);
        Ok(vec![refs[w.selected].clone()])
    })
    .unwrap();
    let tail = b
        .derive(output_field, 1, move |_, deps| {
            Some(
                deps.for_field::<i64>(output_field)
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .sum::<i64>()
                    + 1,
            )
        })
        .unwrap();
    b.set_dependencies(&tail, &[out.erase()]).unwrap();
    let mut session = b.build();
    let original = World {
        values: [2, 8],
        selected: 0,
    };
    session.update_notified(&original).unwrap();
    assert_eq!(resolves.get(), 1);
    assert_eq!(reads.get(), 2);
    let saved = session.checkpoint().unwrap();
    let mut world = World {
        values: [3, 99],
        selected: 1,
    };
    session.mark_dirty(&inputs[0]).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(resolves.get(), 1);
    assert_eq!(reads.get(), 3);
    assert_eq!(session.read(&out).unwrap().value, Some(&3));
    session.mark_dirty(&out).unwrap();
    session.update_notified(&world).unwrap();
    assert_eq!(resolves.get(), 2);
    assert_eq!(reads.get(), 3);
    assert_eq!(
        session.read(&out).unwrap().value,
        Some(&8),
        "unnotified input retains its sampled value"
    );
    session.set_dependencies(&tail, &[tail.erase()]).unwrap();
    assert!(session.read(&tail).is_err());
    session.update_notified(&world).unwrap();
    assert!(session.read(&tail).unwrap().inconsistent);
    session.mark_dirty(&inputs[1]).unwrap();
    world.values[1] = 12;
    session.update_notified(&world).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&12));
    assert!(session.read(&tail).unwrap().inconsistent);
    let cyclic = session.checkpoint().unwrap();
    session.set_dependencies(&tail, &[out.erase()]).unwrap();
    world.values = [500, 600];
    session.update_notified(&world).unwrap();
    assert_eq!(session.read(&out).unwrap().value, Some(&12));
    assert_eq!(session.read(&tail).unwrap().value, Some(&13));
    assert!(session.restore(&saved, &world).is_err());
    session
        .restore(
            &cyclic,
            &World {
                values: [3, 12],
                selected: 1,
            },
        )
        .unwrap();
    assert!(session.read(&tail).unwrap().inconsistent);
    session.restore(&saved, &original).unwrap();
    assert_eq!(session.read(&tail).unwrap().value, Some(&3));
    session.update(&world).unwrap();
    assert_eq!(session.read(&tail).unwrap().value, Some(&601));
}
#[test]
fn sparse_supplier_panic_retains_notifications_and_shared_checkpoint_values() {
    let schema = schema::<i64>();
    let fields = schema.entity::<Entity>().unwrap();
    let fail = Cell::new(false);
    let mut builder = ShadowSessionBuilder::<usize, i64>::new(&schema);
    let input = builder
        .input(fields.variable("input").unwrap(), 0, |world| Some(*world))
        .unwrap();
    let source = input.clone();
    let fail_supplier = &fail;
    let output = builder
        .derive(fields.variable("output").unwrap(), 0, move |_, deps| {
            assert!(!fail_supplier.get(), "supplier panic");
            Some(deps.get(&source).unwrap().unwrap() * 2)
        })
        .unwrap();
    builder.set_dependencies(&output, &[input.erase()]).unwrap();
    let mut session = builder.build();
    session.update(&1).unwrap();
    let saved = session.checkpoint().unwrap();
    fail.set(true);
    session.mark_dirty(&input).unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| session.update_notified(&7))).is_err());
    assert!(session.read(&output).is_err());
    fail.set(false);
    session.update_notified(&7).unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&14));
    session.restore(&saved, &1).unwrap();
    assert_eq!(session.read(&output).unwrap().value, Some(&2));
}
#[test]
fn notifier_checkpoint_failure_after_successful_sparse_move_cannot_publish_state() {
    for panic in [false, true] {
        let schema = schema::<i64>();
        let fields = schema.entity::<Entity>().unwrap();
        let mut b = ShadowSessionBuilder::<usize, i64>::new(&schema);
        let input = b
            .input(fields.variable("input").unwrap(), 0, |v| Some(*v))
            .unwrap();
        let source = input.clone();
        let output = b
            .derive(fields.variable("output").unwrap(), 0, move |_, values| {
                values.get(&source).unwrap().copied()
            })
            .unwrap();
        b.set_dependencies(&output, &[input.erase()]).unwrap();
        let projected = output.clone();
        let fail = Cell::new(false);
        let manager = ShadowSolutionManager::new(
            b.build(),
            move |_, s| Ok::<_, &'static str>(*s.read(&projected).unwrap().value.unwrap()),
            |_, _| Ok(()),
            |_, v| Ok(SimpleScore::new(*v)),
        );
        let calc = ShadowCalculator::new(manager, || 0).with_change_notifier(
            |before, after, _: &(), s| {
                assert_ne!(before, after);
                s.mark_dirty(&input).unwrap();
                if fail.get() {
                    s.set_dependencies(&output, &[output.erase()]).unwrap();
                    if panic {
                        panic!("notifier panic");
                    }
                    return Err("notifier error");
                }
                Ok(())
            },
        );
        let mut session = CalculatorSession::new(1, calc, false).unwrap();
        session.change(&(), |w| *w = 2).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(2));
        fail.set(true);
        let result = catch_unwind(AssertUnwindSafe(|| session.change(&(), |w| *w = 3)));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(session.calculator().solution().is_err());
        fail.set(false);
        session.reset(4).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(4));
        session.change(&(), |w| *w = 5).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(5));
    }
}
