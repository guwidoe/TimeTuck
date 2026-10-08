//! Original transactional integration checks; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct World(i64);
#[derive(Debug, PartialEq)]
struct Shadows(i64);
#[derive(Debug, PartialEq)]
struct Score(i64);
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<i64>("input", false),
        PlanningVariableDefinition::shadow::<i64>("double"),
    ])])
    .unwrap()
}
fn step(control: &Cell<(u8, bool)>, stage: u8) -> Result<(), &'static str> {
    let (at, panic) = control.get();
    if at == stage {
        assert!(!panic, "injected callback panic");
        Err("injected callback error")
    } else {
        Ok(())
    }
}
type Manager<'a> = ShadowSolutionManager<'a, 'a, usize, World, Shadows, Score, &'static str>;
fn manager<'a>(
    schema: &'a PlanningSchema,
    control: &'a Cell<(u8, bool)>,
    calls: &'a Cell<usize>,
) -> (Manager<'a>, ShadowNode<i64>, ShadowNode<i64>) {
    let e = schema.entity::<Entity>().unwrap();
    let mut b = ShadowSessionBuilder::<usize, World>::new(schema);
    let input = b
        .input(e.variable("input").unwrap(), 0, |w| Some(w.0))
        .unwrap();
    let source = input.clone();
    let double = b
        .derive(e.variable("double").unwrap(), 0, move |_, i| {
            calls.set(calls.get() + 1);
            step(control, 1).unwrap();
            i.get(&source).unwrap().map(|v| v * 2)
        })
        .unwrap();
    b.set_dependencies(&double, &[input.erase()]).unwrap();
    let out = double.clone();
    let manager = ShadowSolutionManager::new(
        b.build(),
        move |_, s| {
            step(control, 2)?;
            Ok(Shadows(*s.read(&out).unwrap().value.unwrap()))
        },
        move |_, _| step(control, 3),
        move |_, h| {
            step(control, 4)?;
            Ok(Score(-h.0))
        },
    )
    .with_preflight(move |_| step(control, 5));
    (manager, input, double)
}
#[test]
fn every_adapter_failure_and_panic_restores_initialized_and_uninitialized_sessions() {
    for initialized in [false, true] {
        for stage in 1..=5 {
            for panic in [false, true] {
                if stage == 1 && !panic {
                    continue;
                }
                let schema = schema();
                let control = Cell::new((0, false));
                let calls = Cell::new(0);
                let (mut m, input, double) = manager(&schema, &control, &calls);
                let mut s = ManagedSolution::new(World(3), Shadows(99));
                s.score = Some(Score(100));
                if initialized {
                    m.update(&mut s).unwrap();
                }
                s.assignment.0 = 7;
                control.set((stage, panic));
                let result = catch_unwind(AssertUnwindSafe(|| m.update(&mut s).map(|_| ())));
                if panic {
                    assert!(result.is_err());
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(ShadowSolutionError::Adapter("injected callback error"))
                    ));
                }
                assert_eq!(s.assignment.0, 7);
                assert_eq!(s.shadows.0, if initialized { 6 } else { 99 });
                assert_eq!(
                    s.score.as_ref().unwrap().0,
                    if initialized { -6 } else { 100 }
                );
                if initialized {
                    assert_eq!(m.session().read(&input).unwrap().value, Some(&3));
                    assert_eq!(m.session().read(&double).unwrap().value, Some(&6));
                } else {
                    assert!(matches!(
                        m.session().read(&double),
                        Err(ShadowSessionError::NotCurrent)
                    ));
                }
                control.set((0, false));
                m.update(&mut s).unwrap();
                assert_eq!((s.shadows.0, s.score.as_ref().unwrap().0), (14, -14));
                let before = calls.get();
                m.update(&mut s).unwrap();
                assert_eq!(
                    calls.get(),
                    before,
                    "successful retry must leave reusable cache"
                );
            }
        }
    }
}
#[test]
fn policies_skip_graph_or_score_and_keep_nonclone_outputs() {
    let schema = schema();
    let control = Cell::new((1, true));
    let calls = Cell::new(0);
    let (mut m, _, double) = manager(&schema, &control, &calls);
    let mut s = ManagedSolution::new(World(3), Shadows(99));
    assert_eq!(
        m.update_with_policy(&mut s, SolutionUpdatePolicy::ScoreOnly)
            .unwrap(),
        Some(&Score(-99))
    );
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        m.session().read(&double),
        Err(ShadowSessionError::NotCurrent)
    ));
    control.set((4, true));
    m.update_with_policy(&mut s, SolutionUpdatePolicy::ShadowsOnly)
        .unwrap();
    assert_eq!(s.shadows, Shadows(6));
    assert_eq!(s.score, Some(Score(-99)));
    control.set((0, false));
    m.update(&mut s).unwrap();
    assert_eq!(s.score, Some(Score(-6)));
    assert_eq!(calls.get(), 1);
}
#[test]
fn rejected_cycles_keep_pending_edits_and_diagnostics_then_recover() {
    let schema = schema();
    let control = Cell::new((0, false));
    let calls = Cell::new(0);
    let (mut m, input, double) = manager(&schema, &control, &calls);
    let mut s = ManagedSolution::new(World(3), Shadows(0));
    m.update(&mut s).unwrap();
    m.session_mut()
        .set_dependencies(&double, &[double.erase()])
        .unwrap();
    for _ in 0..2 {
        let err = m.update(&mut s).unwrap_err();
        match err {
            ShadowSolutionError::Shadows(ConsistentShadowUpdateError::Inconsistent(report)) => {
                assert_eq!(report.loops.len(), 1);
                assert_eq!(report.loops[0][0].variable, "double");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            m.session().read(&double),
            Err(ShadowSessionError::NotCurrent)
        ));
        assert_eq!((s.shadows.0, s.score.as_ref().unwrap().0), (6, -6));
    }
    m.session_mut()
        .set_dependencies(&double, &[input.erase()])
        .unwrap();
    s.assignment.0 = 9;
    m.update(&mut s).unwrap();
    assert_eq!((s.shadows.0, s.score.as_ref().unwrap().0), (18, -18));
}
#[test]
fn projection_failure_restores_pending_dirty_notification() {
    let schema = schema();
    let control = Cell::new((0, false));
    let calls = Cell::new(0);
    let (mut m, _, double) = manager(&schema, &control, &calls);
    let mut s = ManagedSolution::new(World(3), Shadows(0));
    m.update(&mut s).unwrap();
    m.session_mut().mark_dirty(&double).unwrap();
    control.set((2, false));
    assert!(m.update(&mut s).is_err());
    assert!(matches!(
        m.session().read(&double),
        Err(ShadowSessionError::NotCurrent)
    ));
    assert_eq!(calls.get(), 2);
    control.set((0, false));
    m.update(&mut s).unwrap();
    assert_eq!(calls.get(), 3);
}
