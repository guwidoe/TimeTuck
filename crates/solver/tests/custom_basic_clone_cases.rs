// Adapted CustomSolutionClonerTest, Apache-2.0; see provenance and notices.
#[path = "fixtures/custom_basic_clone_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn cloned_using_custom_cloner() {
    let solved = run(Policy::Correct).unwrap();
    assert!(solved.solution.cloned);
    assert!(solved.native.complete);
    assert_eq!(solved.solution.score, solved.native.score);
}
#[test]
fn score_not_cloned() {
    let error = run(Policy::MissingScore).err().unwrap();
    assert!(
        matches!(error, BasicSolutionCloneError::ScoreCorruption(_)),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("Cloning corruption: the original's score ")
    );
}
#[test]
fn score_not_equal() {
    let error = run(Policy::WrongScore).err().unwrap();
    assert!(
        matches!(error, BasicSolutionCloneError::ScoreCorruption(_)),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("Cloning corruption: the original's score ")
    );
}
#[test]
fn entities_not_cloned() {
    let error = run(Policy::SharedEntity).err().unwrap();
    assert!(
        matches!(error, BasicSolutionCloneError::SharedEntity { .. }),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .contains("Cloning corruption: the same entity ")
    );
}
