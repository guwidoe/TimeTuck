// Original native regressions; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[test]
fn compound_drafts_replay_and_rollback_in_both_scoring_backends() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| {
                Variable::new(vec![0, 1, 2, 3])
                    .with_initial(i)
                    .allowing_unassigned()
            })
            .collect(),
    )
    .unwrap();
    let base = p.initial_assignment();
    let mut draft = BasicMoveBuilder::new(&p, &base).unwrap();
    draft.change(1, None).unwrap();
    draft.cycle(&[3, 1, 0]).unwrap();
    draft.swap(0, 2).unwrap();
    assert_eq!(
        draft.assignment().values(),
        &[Some(2), Some(3), None, Some(0)]
    );
    let mut fork = draft.clone();
    fork.change_many([(0, Some(99)), (0, Some(1)), (3, Some(2))])
        .unwrap();
    assert_eq!(
        fork.assignment().values(),
        &[Some(1), Some(3), None, Some(2)]
    );
    let expected = draft.assignment().clone();
    let movement = draft.finish();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("weighted", vec![0, 1, 2, 3], |v| {
        SimpleScore::new(
            v.iter()
                .enumerate()
                .map(|(i, v)| (i as i64 + 1) * i64::from(v.copied().unwrap_or(0)))
                .sum(),
        )
    })
    .unwrap();
    let mut scoped = ScoreDirector::new(&p, &n, base.clone()).unwrap();
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    b.score("weighted", source.port(), |(i, v)| {
        SimpleScore::new((*i as i64 + 1) * i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    let mut stream = StreamScoreDirector::new(&p, b.build(), source, base.clone()).unwrap();
    macro_rules! exercise {
        ($director:expr) => {{
            let d = &mut $director;
            {
                let mut s = d.edit_session().unwrap();
                s.apply(&movement).unwrap();
                assert_eq!(s.assignment(), &expected);
                assert_eq!(s.score(), SimpleScore::new(8));
            }
            assert_eq!(d.assignment(), &base);
            assert_eq!(d.score(), SimpleScore::new(20));
            let mut s = d.edit_session().unwrap();
            s.apply(&movement).unwrap();
            s.commit();
            assert_eq!(d.assignment(), &expected);
            assert_eq!(d.score(), SimpleScore::new(8));
        }};
    }
    exercise!(scoped);
    exercise!(stream);
    stream.assert_consistent().unwrap();
    let mut replay = base.clone();
    p.apply(&mut replay, &fork.finish()).unwrap().commit();
    assert_eq!(replay.values(), &[Some(1), Some(3), None, Some(2)]);
    assert_eq!(base, p.initial_assignment());
}

#[test]
fn failed_operations_preserve_prior_edits_and_final_validation_allows_restoration() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![1, 2]).with_initial(2),
        Variable::new(vec![2, 3]).with_initial(3),
    ])
    .unwrap();
    assert!(BasicMoveBuilder::new(&p, &Assignment::new(vec![])).is_err());
    let base = p.initial_assignment();
    let mut d = BasicMoveBuilder::new(&p, &base).unwrap();
    d.change(1, Some(1)).unwrap();
    let saved = d.assignment().clone();
    let steps = d.steps().to_vec();
    for movement in [
        Move::Swap { left: 0, right: 2 },
        Move::Change {
            variable: 2,
            value: Some(1),
        },
        compose_basic_moves([
            Move::Change {
                variable: 1,
                value: Some(2),
            },
            Move::Change {
                variable: 9,
                value: None,
            },
        ]),
    ] {
        assert!(d.push(movement).is_err());
        assert_eq!(d.assignment(), &saved);
        assert_eq!(d.steps(), steps);
    }
    assert_eq!(
        d.cycle(&[1, 2, 1]),
        Err(BasicMoveBuildError::DuplicateCycleVariable(1))
    );
    assert!(d.cycle(&[1, 9]).is_err());
    assert!(d.cycle(&[1, 2]).is_err()); // moved 3 is outside slot 1's range
    assert_eq!(d.assignment(), &saved);
    assert_eq!(d.steps(), steps);
    d.change_many([(0, Some(99)), (0, Some(1)), (2, Some(99)), (2, Some(2))])
        .unwrap();
    d.cycle(&[]).unwrap();
    d.cycle(&[0]).unwrap();
    d.change(1, None).unwrap(); // required slots may be temporarily unassigned
    let expected = d.assignment().clone();
    let movement = d.finish();
    let mut replay = base.clone();
    {
        let trial = p.apply(&mut replay, &movement).unwrap();
        assert_eq!(trial.values(), expected.values());
    }
    assert_eq!(replay, base);
}

#[derive(Debug)]
struct Fragile {
    value: i32,
    remaining: Rc<Cell<Option<usize>>>,
}
impl PartialEq for Fragile {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Fragile {}
impl Clone for Fragile {
    fn clone(&self) -> Self {
        if let Some(n) = self.remaining.get() {
            self.remaining.set(n.checked_sub(1));
            assert!(n > 0, "injected clone failure");
        }
        Self {
            value: self.value,
            remaining: self.remaining.clone(),
        }
    }
}
#[test]
fn clone_failures_during_preparation_and_application_leave_draft_reusable() {
    let remaining = Rc::new(Cell::new(None));
    let v = |value| Fragile {
        value,
        remaining: remaining.clone(),
    };
    let p = FiniteProblem::new(vec![
        Variable::new(vec![v(1), v(2), v(3)]).with_initial(v(1)),
        Variable::new(vec![v(1), v(2), v(3)]).with_initial(v(2)),
        Variable::new(vec![v(1), v(2), v(3)]).with_initial(v(3)),
    ])
    .unwrap();
    let base = p.initial_assignment();
    // Walk every clone failure point until one complete cycle succeeds. This
    // covers gathering values and rollback after earlier child writes.
    let mut failures = 0;
    for limit in 0..64 {
        let mut d = BasicMoveBuilder::new(&p, &base).unwrap();
        d.swap(0, 1).unwrap();
        let saved = d.assignment().clone();
        let steps = d.steps().to_vec();
        remaining.set(Some(limit));
        let result = catch_unwind(AssertUnwindSafe(|| d.cycle(&[0, 1, 2])));
        remaining.set(None);
        match result {
            Ok(result) => {
                result.unwrap();
                assert!(failures >= 3);
                return;
            }
            Err(_) => {
                failures += 1;
                assert_eq!(d.assignment(), &saved);
                assert_eq!(d.steps(), steps);
                d.cycle(&[0, 1, 2]).unwrap();
                assert_eq!(
                    d.assignment()
                        .values()
                        .iter()
                        .map(|v| v.as_ref().unwrap().value)
                        .collect::<Vec<_>>(),
                    vec![3, 2, 1]
                );
            }
        }
    }
    panic!("cycle never completed");
}
