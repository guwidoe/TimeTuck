//! Original borrowed move-check comparisons against transactional application.
use timetuck::*;
fn compare(p: &FiniteProblem<i32>, before: &Assignment<i32>, movement: &Move<i32>) {
    let mut candidate = before.clone();
    let expected = p
        .apply(&mut candidate, movement)
        .map(|trial| trial.values() != before.values());
    assert_eq!(
        p.is_move_doable(before, movement),
        expected,
        "{before:?}, {movement:?}"
    );
    assert_eq!(
        p.move_checker(before).and_then(|c| c.is_doable(movement)),
        expected
    );
}
#[test]
fn nested_checks_match_transactional_final_states_and_errors() {
    for pinned in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .with_initial(0)
                .with_pinned(pinned)
                .with_range_diagnostics("job0", "worker", i32::to_string),
            Variable::new(vec![1, 2]),
            Variable::new(vec![0, 2]),
        ])
        .unwrap();
        let mut moves = vec![
            Move::Composite(vec![]),
            Move::Swap { left: 0, right: 1 },
            Move::Swap { left: 2, right: 0 },
            Move::Swap { left: 1, right: 1 },
            Move::Swap { left: 3, right: 0 },
        ];
        for variable in 0..4 {
            for value in [None, Some(0), Some(1), Some(2), Some(99)] {
                moves.push(Move::Change { variable, value });
            }
        }
        for before in [
            Assignment::new(vec![None, None, None]),
            Assignment::new(vec![Some(0), Some(1), Some(0)]),
            Assignment::new(vec![Some(0), Some(2), Some(2)]),
        ] {
            for a in &moves {
                compare(&p, &before, a);
                for b in &moves {
                    compare(
                        &p,
                        &before,
                        &Move::Composite(vec![a.clone(), Move::Composite(vec![b.clone()])]),
                    );
                }
            }
        }
    }
}
#[test]
fn checks_require_no_clone_bound_and_restore_composites_are_not_changes() {
    #[derive(Debug, PartialEq, Eq)]
    struct Id(u32);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![Id(1), Id(2)]),
        Variable::new(vec![Id(1), Id(2)]),
    ])
    .unwrap();
    let before = Assignment::new(vec![Some(Id(1)), None]);
    assert!(
        p.is_move_doable(&before, &Move::Swap { left: 0, right: 1 })
            .unwrap()
    );
    let restored = Move::Composite(vec![
        Move::Swap { left: 0, right: 1 },
        Move::Swap { left: 0, right: 1 },
    ]);
    assert!(!p.is_move_doable(&before, &restored).unwrap());
    assert!(
        !p.is_move_doable(
            &before,
            &Move::Change {
                variable: 0,
                value: Some(Id(1))
            }
        )
        .unwrap()
    );
    assert!(
        p.is_move_doable(
            &before,
            &Move::Change {
                variable: 0,
                value: None
            }
        )
        .unwrap()
    );
    assert_eq!(before.values(), &[Some(Id(1)), None]);
}
#[test]
fn restored_domains_and_pins_validate_only_after_all_indices_are_checked() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
        Variable::new(vec![2]).with_initial(2),
    ])
    .unwrap();
    let before = p.initial_assignment();
    let swap = Move::Swap { left: 0, right: 1 };
    assert!(p.is_move_doable(&before, &swap).is_err());
    let restore = Move::Composite(vec![swap.clone(), swap]);
    assert!(!p.is_move_doable(&before, &restore).unwrap());
    let invalid = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(99),
        },
        Move::Change {
            variable: 4,
            value: None,
        },
    ]);
    compare(&p, &before, &invalid);
    compare(&p, &Assignment::new(vec![Some(1)]), &invalid);
    let mut deep = Move::Change {
        variable: 0,
        value: Some(1),
    };
    for _ in 0..512 {
        deep = Move::Composite(vec![deep]);
    }
    assert!(!p.is_move_doable(&before, &deep).unwrap());
}

#[test]
fn repeated_checks_only_compare_written_variables_after_snapshot_validation() {
    use std::{cell::Cell, rc::Rc};
    struct Value(u32, Rc<Cell<usize>>);
    impl PartialEq for Value {
        fn eq(&self, rhs: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == rhs.0
        }
    }
    impl Eq for Value {}
    let comparisons = Rc::new(Cell::new(0));
    let value = |id| Value(id, Rc::clone(&comparisons));
    let p = FiniteProblem::new(
        (0..1024)
            .map(|_| Variable::new(vec![value(0), value(1)]))
            .collect(),
    )
    .unwrap();
    let a = Assignment::new((0..1024).map(|_| Some(value(0))).collect());
    let checker = p.move_checker(&a).unwrap();
    let after_validation = comparisons.get();
    for variable in [0, 511, 1023] {
        assert!(
            checker
                .is_doable(&Move::Change {
                    variable,
                    value: Some(value(1))
                })
                .unwrap()
        );
    }
    // Two domain comparisons and one change comparison per candidate, independent
    // of the 1,024 assigned variables. Values deliberately do not implement Clone.
    assert_eq!(comparisons.get() - after_validation, 9);
}

#[test]
fn lazy_providers_filter_before_scoring_on_both_basic_backends() {
    struct Provider {
        inspected: usize,
        emitted: usize,
    }
    impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider {
        fn moves<'a>(
            &'a mut self,
            p: &'a FiniteProblem<i32>,
            a: &'a Assignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
        where
            Move<i32>: 'a,
        {
            let checker = p.move_checker(a).unwrap();
            let current = a.values()[0].unwrap();
            let change = |v| Move::Change {
                variable: 0,
                value: Some(v),
            };
            let candidates = [
                change(current),
                Move::Swap { left: 0, right: 7 },
                change(current + 1),
                Move::Composite(vec![change(99), change(current)]),
            ];
            Box::new(candidates.into_iter().filter(move |m| {
                self.inspected += 1;
                // This provider deliberately discards invalid proposals as well
                // as unchanged ones. Engine defaults still reject invalid moves.
                let keep = checker.is_doable(m) == Ok(true);
                self.emitted += usize::from(keep);
                keep
            }))
        }
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    for (stream, selection) in [
        (false, MoveSelection::FirstAccepted),
        (true, MoveSelection::FirstAccepted),
        (false, MoveSelection::BestAccepted),
        (true, MoveSelection::BestAccepted),
    ] {
        let mut provider = Provider {
            inspected: 0,
            emitted: 0,
        };
        let config = LocalSearchConfig {
            selection,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                SimpleScore::new(i64::from(*v[0].unwrap()))
            })
            .unwrap();
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let inspected = if selection == MoveSelection::FirstAccepted {
            10
        } else {
            12
        };
        assert_eq!(provider.inspected, inspected);
        assert_eq!(provider.emitted, 2);
        assert_eq!(result.evaluations, 3); // Initialization plus two changing moves.
        assert_eq!(result.completed_steps, 2);
        assert_eq!(result.best.unwrap().score, SimpleScore::new(2));
    }
}
