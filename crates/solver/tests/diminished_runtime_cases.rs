// Adapted pinned Community TimefoldRecorderDiminishedReturnsTest; Apache-2.0.
#[path = "fixtures/diminished_runtime_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn nothing_set() {
    let mut p = empty();
    let before = p.clone();
    let b = properties(Default::default()).bind(&mut p).unwrap();
    assert_no(b, &p, &before);
}
#[test]
fn only_enabled_set() {
    let mut p = empty();
    let b = properties(RuntimeDiminishedReturns {
        enabled: Some(true),
        ..Default::default()
    })
    .bind(&mut p)
    .unwrap();
    assert_present(b, None, None);
}
#[test]
fn only_sliding_window_set() {
    let mut p = empty();
    let b = properties(RuntimeDiminishedReturns {
        sliding_window_duration: Some(WINDOW),
        ..Default::default()
    })
    .bind(&mut p)
    .unwrap();
    assert_present(b, Some(WINDOW), None);
}
#[test]
fn only_minimum_improvement_ratio_set() {
    let mut p = empty();
    let b = properties(RuntimeDiminishedReturns {
        minimum_improvement_ratio: Some(RATIO),
        ..Default::default()
    })
    .bind(&mut p)
    .unwrap();
    assert_present(b, None, Some(RATIO));
}
#[test]
fn minimum_improvement_ratio_and_sliding_window_set() {
    let mut p = empty();
    let b = properties(RuntimeDiminishedReturns {
        sliding_window_duration: Some(WINDOW),
        minimum_improvement_ratio: Some(RATIO),
        ..Default::default()
    })
    .bind(&mut p)
    .unwrap();
    assert_present(b, Some(WINDOW), Some(RATIO));
}
#[test]
fn disabled_and_minimum_improvement_ratio_and_sliding_window_set() {
    let mut p = empty();
    let before = p.clone();
    let b = properties(RuntimeDiminishedReturns {
        enabled: Some(false),
        sliding_window_duration: Some(WINDOW),
        minimum_improvement_ratio: Some(RATIO),
    })
    .bind(&mut p)
    .unwrap();
    assert_no(b, &p, &before);
}
#[test]
fn disabled_and_phases_configured() {
    let mut p = SolverSequence {
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(Default::default()),
        ],
        ..Default::default()
    };
    let before = p.clone();
    let b = properties(RuntimeDiminishedReturns {
        enabled: Some(false),
        ..Default::default()
    })
    .bind(&mut p)
    .unwrap();
    assert_no(b, &p, &before);
}
