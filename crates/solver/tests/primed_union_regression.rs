use std::{cell::Cell, collections::BTreeMap, rc::Rc};
use timetuck::*;
#[test]
fn finite_counts_seed_replay_disabled_children_and_borrowed_payloads() {
    let run = || {
        PrimedMoveUnion::from_iterators(
            [(0, 3), (1, 1), (2, 5)].map(|(label, n)| (std::iter::repeat_n(label, n), 1)),
            UnionOrder::Random { seed: 7 },
        )
        .collect::<Vec<_>>()
    };
    let first = run();
    assert_eq!(first, run());
    let mut counts = BTreeMap::new();
    for label in first {
        *counts.entry(label).or_insert(0) += 1;
    }
    assert_eq!(counts, BTreeMap::from([(0, 3), (1, 1), (2, 5)]));
    let polls = Rc::new(Cell::new(0));
    let observed = polls.clone();
    let disabled = std::iter::from_fn(move || {
        observed.set(observed.get() + 1);
        Some(9)
    });
    let children: Vec<(Box<dyn Iterator<Item = i32>>, u32)> =
        vec![(Box::new(disabled), 0), (Box::new(std::iter::once(1)), 1)];
    let mut union = PrimedMoveUnion::from_iterators(children, UnionOrder::Random { seed: 0 });
    assert!(union.has_next());
    assert!(union.has_next());
    assert_eq!(union.next(), Some(1));
    assert!(!union.has_next());
    assert_eq!(union.next(), None);
    assert_eq!(union.next(), None);
    assert_eq!(polls.get(), 0);
    struct Item(u8);
    let items = [Item(4), Item(8)];
    let cursor = PrimedMoveUnion::from_iterators(
        [(items[..1].iter(), 1), (items[1..].iter(), 1)],
        UnionOrder::Sequential,
    );
    assert_eq!(cursor.map(|v| v.0).collect::<Vec<_>>(), vec![4, 8]);
}
#[test]
fn availability_observations_are_cached_and_panic_after_draw_fuses() {
    struct Cursor {
        polls: Rc<Cell<usize>>,
        draws: Rc<Cell<usize>>,
    }
    impl AvailableMoveCursor for Cursor {
        type Move = usize;
        fn poll_available(&mut self) -> bool {
            let n = self.polls.get() + 1;
            self.polls.set(n);
            assert!(n < 2, "poll fault after actual draw");
            true
        }
        fn draw_move(&mut self) -> Option<usize> {
            self.draws.set(self.draws.get() + 1);
            Some(1)
        }
    }
    let polls = Rc::new(Cell::new(0));
    let draws = Rc::new(Cell::new(0));
    let mut union = PrimedMoveUnion::new(
        [(
            Cursor {
                polls: polls.clone(),
                draws: draws.clone(),
            },
            1,
        )],
        UnionOrder::Random { seed: 0 },
    );
    for _ in 0..100 {
        assert!(union.has_next());
    }
    assert_eq!(polls.get(), 1);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| union.next())).is_err());
    assert_eq!(draws.get(), 1);
    assert!(!union.has_next());
    assert_eq!(union.next(), None);
    assert_eq!(polls.get(), 2);
}
#[test]
fn primed_provider_union_executes_actual_seeded_solver_scans() {
    struct Leaf {
        variable: usize,
    }
    impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Leaf {
        fn moves<'a>(
            &'a mut self,
            _: &'a FiniteProblem<i32>,
            a: &'a Assignment<i32>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
        where
            Move<i32>: 'a,
        {
            let next = a.values()[self.variable].unwrap() + 1;
            let movement = (next <= 2).then_some(Move::Change {
                variable: self.variable,
                value: Some(next),
            });
            Box::new(movement.into_iter())
        }
    }
    let run = || {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 3]).unwrap();
        let mut n = ConstraintNetwork::<i32, SimpleScore>::new(3);
        n.add_constraint("sum", vec![0, 1, 2], |values| {
            SimpleScore::new(values.iter().filter_map(|v| v.map(|v| i64::from(*v))).sum())
        })
        .unwrap();
        let mut leaves = [
            Leaf { variable: 0 },
            Leaf { variable: 1 },
            Leaf { variable: 2 },
        ];
        let providers = leaves.iter_mut().map(|leaf| {
            leaf as &mut dyn MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>>
        });
        let mut union = ProviderUnion::unweighted(providers, UnionOrder::Random { seed: 42 })
            .unwrap()
            .with_primed_polling();
        let mut extensions = SolverExtensions::default();
        extensions.move_providers.bind(0, &mut union);
        solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 20,
                    max_evaluations: 100,
                    selection: MoveSelection::FirstAccepted,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                })],
                ..Default::default()
            },
            extensions,
        )
        .unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first.assignment.values(), &[Some(2), Some(2), Some(2)]);
    assert_eq!(first.score, Some(SimpleScore::new(6)));
    assert_eq!(first.work.completed_steps, 6);
    assert_eq!(first.assignment, second.assignment);
    assert_eq!(first.work, second.work);
}
