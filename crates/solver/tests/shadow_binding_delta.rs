//! Original sparse binding delta regressions; no upstream method credit.
use std::{
    cell::RefCell,
    collections::BTreeSet,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Model;
struct Entity;
#[derive(Clone)]
struct World {
    inputs: Vec<i64>,
    sources: Vec<Vec<usize>>,
    fail: Option<(char, usize)>,
}
type Log = Rc<RefCell<Vec<(char, usize)>>>;
type Fixture<'s> = (ShadowSession<'s, 's, usize, World>, Vec<ShadowNode<i64>>);
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap()
}
fn session<'s>(s: &'s PlanningSchema, n: usize, log: &Log) -> Fixture<'s> {
    let fields = s.entity::<Entity>().unwrap();
    let input = fields.variable("input").unwrap();
    let output = fields.variable("output").unwrap();
    let mut b = ShadowSessionBuilder::<usize, World>::new(s);
    let mut handles = vec![];
    for i in 0..n {
        let log = log.clone();
        handles.push(
            b.input(input, i, move |w| {
                log.borrow_mut().push(('r', i));
                assert_ne!(w.fail, Some(('r', i)), "reader panic");
                Some(w.inputs[i])
            })
            .unwrap(),
        );
    }
    for i in 0..n {
        let log = log.clone();
        handles.push(
            b.derive(output, i, move |w, deps| {
                log.borrow_mut().push(('s', i));
                assert_ne!(w.fail, Some(('s', i)), "supplier panic");
                Some(
                    1 + deps
                        .for_field::<i64>(input)
                        .unwrap()
                        .into_iter()
                        .chain(deps.for_field::<i64>(output).unwrap())
                        .flatten()
                        .copied()
                        .sum::<i64>(),
                )
            })
            .unwrap(),
        );
    }
    for i in 0..n {
        let refs = handles.iter().map(ShadowNode::erase).collect::<Vec<_>>();
        let log = log.clone();
        b.bind_dependencies(&handles[n + i], move |w| {
            log.borrow_mut().push(('b', i));
            assert_ne!(w.fail, Some(('b', i)), "resolver panic");
            if w.fail == Some(('e', i)) {
                return Err(ShadowSessionError::UnregisteredSource);
            }
            Ok(w.sources[i]
                .iter()
                .map(|&source| refs[source].clone())
                .collect())
        })
        .unwrap();
    }
    (b.build(), handles)
}
// Independent recursive evaluator: cycles propagate absence to all consumers.
fn reference(w: &World) -> Vec<Option<i64>> {
    fn visit(i: usize, w: &World, state: &mut [u8], values: &mut [Option<i64>]) -> Option<i64> {
        if state[i] == 2 {
            return values[i];
        }
        if state[i] == 1 {
            return None;
        }
        state[i] = 1;
        let n = w.inputs.len();
        let value = if i < n {
            Some(w.inputs[i])
        } else {
            let mut total = Some(1);
            for source in w.sources[i - n].iter().copied().collect::<BTreeSet<_>>() {
                total = total
                    .zip(visit(source, w, state, values))
                    .map(|(a, b)| a + b);
            }
            total
        };
        state[i] = 2;
        values[i] = value;
        value
    }
    let mut state = vec![0; 2 * w.inputs.len()];
    let mut values = vec![None; state.len()];
    for i in 0..state.len() {
        visit(i, w, &mut state, &mut values);
    }
    values
}
fn verify(s: &ShadowSession<'_, '_, usize, World>, handles: &[ShadowNode<i64>], w: &World) {
    for (h, expected) in handles.iter().zip(reference(w)) {
        let actual = s.read(h).unwrap();
        assert_eq!(actual.value.copied(), expected);
        assert_eq!(actual.inconsistent, expected.is_none());
    }
}
#[test]
fn sparse_rewires_match_independent_cycles_and_fresh_full_sessions() {
    let schema = schema();
    let n = 20;
    let log = Log::default();
    let (mut s, h) = session(&schema, n, &log);
    let mut w = World {
        inputs: (1..=n as i64).collect(),
        sources: (0..n).map(|i| vec![i]).collect(),
        fail: None,
    };
    s.update_notified(&w).unwrap();
    let mut checkpoints = vec![(s.checkpoint().unwrap(), w.clone())];
    for step in 0..160 {
        let i = step % n;
        let j = (i + 1) % n;
        w.sources[i] = match (step / n) % 4 {
            0 => vec![n + j],
            1 => vec![i, n + j, i],
            2 => vec![j, i, j],
            _ => vec![i],
        };
        w.inputs[j] += 1;
        s.mark_dirty(&h[n + i]).unwrap();
        s.mark_dirty(&h[j]).unwrap();
        log.borrow_mut().clear();
        s.update_notified(&w).unwrap();
        let events = log.borrow().clone();
        assert_eq!(
            events
                .iter()
                .filter(|(kind, _)| *kind == 'b')
                .copied()
                .collect::<Vec<_>>(),
            [('b', i)]
        );
        assert_eq!(
            events
                .iter()
                .filter(|(kind, _)| *kind == 'r')
                .copied()
                .collect::<Vec<_>>(),
            [('r', j)]
        );
        assert_eq!(&events[..2], [('b', i), ('r', j)]);
        verify(&s, &h, &w);
        let (mut fresh, fh) = session(&schema, n, &Log::default());
        fresh.update(&w).unwrap();
        verify(&fresh, &fh, &w);
        if step % 17 == 0 {
            checkpoints.push((s.checkpoint().unwrap(), w.clone()));
        }
    }
    for (saved, old) in checkpoints.iter().rev() {
        s.restore(saved, old).unwrap();
        verify(&s, &h, old);
        let mut edited = old.clone();
        edited.sources[0] = vec![0];
        s.mark_dirty(&h[n]).unwrap();
        s.update_notified(&edited).unwrap();
        verify(&s, &h, &edited);
    }
}
#[test]
fn failed_sparse_preparation_and_evaluation_preserve_retry_and_checkpoints() {
    let schema = schema();
    let log = Log::default();
    let n = 20;
    let (mut s, h) = session(&schema, n, &log);
    let original = World {
        inputs: (1..=n as i64).map(|i| i * 2).collect(),
        sources: (0..n).map(|i| vec![i]).collect(),
        fail: None,
    };
    s.update_notified(&original).unwrap();
    let saved = s.checkpoint().unwrap();
    for fail in [('b', 1), ('e', 1), ('r', 1), ('s', 1)] {
        s.restore(&saved, &original).unwrap();
        let mut w = original.clone();
        w.sources[0] = vec![1];
        w.sources[1] = vec![0];
        w.inputs[1] = 10;
        w.fail = Some(fail);
        for i in [n + 1, 1, n] {
            s.mark_dirty(&h[i]).unwrap();
        }
        log.borrow_mut().clear();
        let result = catch_unwind(AssertUnwindSafe(|| s.update_notified(&w)));
        if fail.0 == 'e' {
            assert!(matches!(
                result.unwrap(),
                Err(ShadowSessionError::UnregisteredSource)
            ));
        } else {
            assert!(result.is_err());
        }
        let events = log.borrow().clone();
        assert_eq!(&events[..2], [('b', 0), ('b', 1)]);
        assert!(matches!(s.read(&h[n]), Err(ShadowSessionError::NotCurrent)));
        w.fail = None;
        log.borrow_mut().clear();
        s.update_notified(&w).unwrap();
        verify(&s, &h, &w);
        assert_eq!(&log.borrow()[..3], [('b', 0), ('b', 1), ('r', 1)]);
        s.restore(&saved, &original).unwrap();
        verify(&s, &h, &original);
    }
}
#[test]
fn ordered_duplicate_signatures_survive_delta_checkpoints_and_full_refresh() {
    for n in [3, 20] {
        let schema = schema();
        let log = Log::default();
        let (mut s, h) = session(&schema, n, &log);
        let mut w = World {
            inputs: (1..=n as i64).map(|i| i * 2).collect(),
            sources: (0..n)
                .map(|i| if i == 0 { vec![0, 1, 0] } else { vec![i] })
                .collect(),
            fail: None,
        };
        s.update_notified(&w).unwrap();
        let saved = s.checkpoint().unwrap();
        let old = w.clone();
        w.sources[0] = vec![1, 0, 1];
        s.mark_dirty(&h[n]).unwrap();
        log.borrow_mut().clear();
        assert!(s.update_notified(&w).unwrap().is_empty());
        assert_eq!(*log.borrow(), [('b', 0), ('s', 0)]);
        assert!(matches!(
            s.restore(&saved, &w),
            Err(ShadowSessionError::SnapshotMismatch)
        ));
        s.restore(&saved, &old).unwrap();
        log.borrow_mut().clear();
        s.update(&w).unwrap();
        assert_eq!(
            log.borrow()
                .iter()
                .filter(|(kind, _)| *kind == 's')
                .copied()
                .collect::<Vec<_>>(),
            [('s', 0)]
        );
        verify(&s, &h, &w);
    }
}

#[test]
fn large_notified_manager_keeps_sparse_missing_source_diagnostics_atomic() {
    struct Linked {
        id: usize,
        source: Option<Box<Linked>>,
    }
    let schema = PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Linked>(vec![
        PlanningVariableDefinition::shadow::<i64>("output"),
    ])])
    .unwrap();
    let field = schema
        .entity::<Linked>()
        .unwrap()
        .variable("output")
        .unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one("source", SourceMemberRole::Fact, |root: &Linked| {
            root.source.as_deref()
        })
        .unwrap();
    paths
        .one(
            "output",
            SourceMemberRole::Declarative,
            |_: &Linked| -> Option<&i64> { panic!("terminal getter") },
        )
        .unwrap();
    let path = paths.compile::<Linked>("output", "source.output").unwrap();
    let suppliers = std::cell::Cell::new(0);
    let mut builder = ShadowSessionBuilder::<usize, Vec<Linked>>::new(&schema);
    let mut handles = Vec::new();
    for i in 0..40 {
        let handle = builder
            .derive(field, i, |_, _| {
                suppliers.set(suppliers.get() + 1);
                Some(1i64)
            })
            .unwrap();
        builder
            .bind_source_paths(
                &handle,
                vec![&path],
                move |world| &world[i],
                |value| value.downcast_ref::<Linked>().map(|node| node.id),
            )
            .unwrap();
        handles.push(handle);
    }
    let projected = handles.clone();
    let mut manager = ShadowSolutionManager::new(
        builder.build(),
        move |_, session| {
            Ok::<_, ()>(
                projected
                    .iter()
                    .map(|node| session.read(node).unwrap().value.copied())
                    .collect::<Vec<_>>(),
            )
        },
        |_, _| Ok(()),
        |_, values| Ok(values.iter().flatten().sum::<i64>()),
    );
    let mut solution = ManagedSolution::new(
        (0..40).map(|id| Linked { id, source: None }).collect(),
        vec![],
    );
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(suppliers.get(), 40);
    solution.assignment[0].source = Some(Box::new(Linked {
        id: 70,
        source: None,
    }));
    solution.assignment[1].source = Some(Box::new(Linked {
        id: 80,
        source: None,
    }));
    // Deliberately incomplete notifications demonstrate diagnostic scope; callers
    // must notify both targets before any successful refresh of this world.
    manager.session_mut().mark_dirty(&handles[0]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing source expected");
    };
    assert_eq!(
        missing
            .iter()
            .map(|r| (r.target.entity, r.source.entity))
            .collect::<Vec<_>>(),
        [(0, 70)]
    );
    let all = missing_shadow_sources(manager.session(), &solution.assignment).unwrap();
    assert_eq!(
        all.iter()
            .map(|r| (r.target.entity, r.source.entity))
            .collect::<Vec<_>>(),
        [(0, 70), (1, 80)]
    );
    manager.session_mut().mark_dirty(&handles[1]).unwrap();
    let Err(ShadowSolutionError::Shadows(ConsistentShadowUpdateError::MissingSources(missing))) =
        manager.update_notified(&mut solution)
    else {
        panic!("missing sources expected");
    };
    assert_eq!(missing, all);
    assert_eq!(suppliers.get(), 40);
    assert_eq!(solution.shadows, vec![Some(1); 40]);
    assert_eq!(solution.score, Some(40));
    for root in &mut solution.assignment {
        root.source = None;
    }
    manager.update_notified(&mut solution).unwrap();
    assert_eq!(suppliers.get(), 42);
    assert!(manager.session().changed_entities().unwrap().is_empty());
}
