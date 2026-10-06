// Adapted from Timefold Community MoveDirectorTest and Testdata fixtures,
// Apache-2.0. See compat/provenance.json for exact methods and helper attribution.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::*;
fn bad() -> Move<&'static str> {
    Move::Change {
        variable: 0,
        value: Some("bad"),
    }
}
macro_rules! both {
    (|$d:ident| $body:block) => {{
        let p = FiniteProblem::new(vec![
            Variable::new(vec!["good", "bad"]).with_initial("good"),
        ])
        .unwrap();
        {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("Bad value", vec![0], |v| {
                SimpleScore::new(-i64::from(v[0].copied() == Some("bad")))
            })
            .unwrap();
            let mut $d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            $body
        }
        {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<&'static str>)>();
            b.score("Bad value", source.port(), |(_, v)| {
                SimpleScore::new(-i64::from(*v == Some("bad")))
            })
            .unwrap();
            let mut $d =
                StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
            $body
        }
    }};
}
#[test]
fn restore_working_score_with_identity_postprocessor() {
    both!(|d| {
        let previous = d.score();
        let result = TemporaryMoves::new(&mut d)
            .process(&bad(), TemporaryMoveOptions::default(), |_, score| {
                Ok::<_, ()>(score)
            })
            .unwrap();
        assert_eq!(result, SimpleScore::new(-1));
        assert_eq!(d.assignment().values(), &[Some("good")]);
        assert_eq!(d.score(), previous);
    });
}
#[test]
fn restore_working_score_with_postprocessor() {
    both!(|d| {
        let previous = d.score();
        let result = TemporaryMoves::new(&mut d)
            .process(&bad(), TemporaryMoveOptions::default(), |a, score| {
                assert_eq!(a.values(), &[Some("bad")]);
                assert_eq!(score, SimpleScore::new(-1));
                Ok::<_, ()>("postprocessed")
            })
            .unwrap();
        assert_eq!(result, "postprocessed");
        assert_eq!(d.assignment().values(), &[Some("good")]);
        assert_eq!(d.score(), previous);
    });
}
#[test]
fn postprocessor_failure_leaves_the_move_applied() {
    both!(|d| {
        let previous = d.score();
        let result = catch_unwind(AssertUnwindSafe(|| {
            TemporaryMoves::new(&mut d).process(
                &bad(),
                TemporaryMoveOptions {
                    failure: TrialFailurePolicy::KeepApplied,
                    ..Default::default()
                },
                |_, _| -> Result<(), ()> { panic!("Postprocessor failure.") },
            )
        }));
        assert!(result.is_err());
        assert_eq!(d.assignment().values(), &[Some("bad")]);
        assert_eq!(d.score(), SimpleScore::new(-1));
        assert_ne!(d.score(), previous);
        d.assert_consistent().unwrap();
    });
}
#[test]
fn execute_temporary_producing_undo_move_returns_an_already_applied_undo_move() {
    both!(|d| {
        let mut observed = None;
        let (_, record) = TemporaryMoves::new(&mut d)
            .recorded(&bad(), TemporaryMoveOptions::default(), |_, score| {
                observed = Some(score);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(observed, Some(SimpleScore::new(-1)));
        assert_eq!(d.assignment().values(), &[Some("good")]);
        let forward = record.forward(d.assignment()).unwrap();
        d.apply(forward).unwrap().commit();
        assert_eq!(d.assignment().values(), &[Some("bad")]);
        let undo = record.undo(d.assignment()).unwrap();
        d.apply(undo).unwrap().commit();
        assert_eq!(d.assignment().values(), &[Some("good")]);
        assert_eq!(d.score(), SimpleScore::new(0));
    });
}
#[test]
fn restore_working_score_with_postprocessor_without_fresh_score() {
    both!(|d| {
        let previous = d.score();
        let result = TemporaryMoves::new(&mut d)
            .process(
                &bad(),
                TemporaryMoveOptions {
                    restored_score: RestoredScorePolicy::Cached,
                    ..Default::default()
                },
                |a, _| Ok::<_, ()>(a.values()[0].unwrap()),
            )
            .unwrap();
        assert_eq!(result, "bad");
        assert_eq!(d.assignment().values(), &[Some("good")]);
        assert_eq!(d.score(), previous);
    });
}
#[test]
fn restore_working_score_with_postprocessor_and_fresh_score() {
    both!(|d| {
        let previous = d.score();
        let result = TemporaryMoves::new(&mut d)
            .process(
                &bad(),
                TemporaryMoveOptions {
                    restored_score: RestoredScorePolicy::CheckFresh,
                    ..Default::default()
                },
                |a, _| Ok::<_, ()>(a.values()[0].unwrap()),
            )
            .unwrap();
        assert_eq!(result, "bad");
        assert_eq!(d.assignment().values(), &[Some("good")]);
        assert_eq!(d.score(), previous);
    });
}
