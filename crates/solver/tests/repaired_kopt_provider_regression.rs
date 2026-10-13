use timetuck::*;
fn provider(seed: u64, attempts: usize) -> RepairedKOptProvider {
    RepairedKOptProvider::new(
        ListKOptConfig::new(2, 12).unwrap(),
        vec![1, 0, 3, 0, 1],
        seed,
        attempts,
    )
    .unwrap()
}
fn model(n: usize, m: usize) -> ListProblem<i32> {
    let split = n + 2;
    let end = split + m + 1;
    ListProblem::new_indexed(
        (0..end as i32 + 1).collect(),
        vec![
            ListEntity::new((0..split as i32).collect()).with_pinned_prefix(2),
            ListEntity::new((split as i32..end as i32).collect()).with_pinned_prefix(1),
            ListEntity::new(vec![end as i32]).with_pinned(true),
        ],
    )
    .unwrap()
}
#[test]
fn seeded_live_multiowner_draws_preserve_pins_lengths_membership_and_undo() {
    for (n, m) in [(9, 13), (6, 10), (16, 7)] {
        let p = model(n, m);
        let a = p.initial_assignment();
        let sampler = provider(37, 512);
        let moves: Vec<_> = sampler.open(&p, &a, 0).unwrap().collect();
        assert!(!moves.is_empty());
        assert_eq!(moves, sampler.open(&p, &a, 0).unwrap().collect::<Vec<_>>());
        assert_ne!(moves, sampler.open(&p, &a, 1).unwrap().collect::<Vec<_>>());
        let mut crosses = 0;
        for movement in moves {
            assert!(p.is_move_doable(&a, &movement).unwrap());
            let mut after = a.clone();
            {
                let trial = p.apply(&mut after, &movement).unwrap();
                p.validate(&trial).unwrap();
                assert_ne!(*trial, a);
                for e in 0..3 {
                    assert_eq!(trial.lists()[e].len(), a.lists()[e].len());
                }
                assert_eq!(&trial.lists()[0][..2], &a.lists()[0][..2]);
                assert_eq!(&trial.lists()[1][..1], &a.lists()[1][..1]);
                assert_eq!(trial.lists()[2], a.lists()[2]);
                crosses += usize::from(
                    trial.lists()[0]
                        .iter()
                        .any(|v| a.lists()[1][1..].contains(v)),
                );
                let mut old: Vec<_> = a.lists().iter().flatten().copied().collect();
                let mut new: Vec<_> = trial.lists().iter().flatten().copied().collect();
                old.sort();
                new.sort();
                assert_eq!(old, new);
            }
            assert_eq!(after, a);
        }
        assert!(crosses > 0);
    }
}
#[test]
fn exhausted_rejected_attempts_are_finite_and_disabled_open_still_validates() {
    let p = ListProblem::new_indexed(
        vec![1, 2],
        vec![
            ListEntity::new(vec![1]).with_value_range(vec![1]),
            ListEntity::new(vec![2]).with_value_range(vec![2]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let sampler =
        RepairedKOptProvider::new(ListKOptConfig::new(2, 2).unwrap(), vec![1], 37, 31).unwrap();
    let mut scan = sampler.open(&p, &a, 0).unwrap();
    assert_eq!(scan.attempts(), 0);
    assert!(scan.next().is_none());
    assert_eq!(scan.attempts(), 31);
    assert!(scan.next().is_none());
    assert_eq!(scan.attempts(), 31);
    assert_eq!(scan.size_hint(), (0, Some(0)));
    let disabled = provider(37, 0);
    assert_eq!(disabled.max_attempts(), 0);
    assert_eq!(disabled.open(&p, &a, 0).unwrap().count(), 0);
    let foreign = model(3, 4);
    assert!(disabled.open(&p, &foreign.initial_assignment(), 0).is_err());
    assert!(
        RepairedKOptProvider::new(ListKOptConfig::new(2, 5).unwrap(), vec![1, 1], 37, 10).is_err()
    );
    assert!(
        RepairedKOptProvider::new(
            ListKOptConfig::new(2, 5).unwrap(),
            vec![u64::MAX, 0, 1],
            37,
            10
        )
        .is_err()
    );
    for weights in [vec![], vec![0], vec![0, 0]] {
        assert!(
            RepairedKOptProvider::new(ListKOptConfig::new(2, 5).unwrap(), weights, 37, 10).is_err()
        );
    }
    let pinned =
        ListProblem::new(vec![1], vec![ListEntity::new(vec![1]).with_pinned(true)]).unwrap();
    let pa = pinned.initial_assignment();
    let mut scan = sampler.open(&pinned, &pa, 0).unwrap();
    assert!(scan.next().is_none());
    assert_eq!(scan.attempts(), 0);
}
#[test]
fn receiving_ranges_are_checked_and_reopened_moves_use_changed_endpoints() {
    let p = model(9, 13);
    let mut a = p.initial_assignment();
    let sampler = provider(91, 256);
    let before = a.clone();
    let old: Vec<_> = sampler.open(&p, &a, 0).unwrap().collect();
    p.apply(&mut a, &old[0]).unwrap().commit();
    assert_ne!(a, before);
    // Returned moves are positional; change membership/geometry before reopening.
    for _ in 0..5 {
        let destination = ListPosition {
            entity: 1,
            index: a.lists()[1].len(),
        };
        p.apply(
            &mut a,
            &ListMove::Change {
                source: ListPosition {
                    entity: 0,
                    index: 2,
                },
                destination,
            },
        )
        .unwrap()
        .commit();
    }
    let current_lengths: Vec<_> = a.lists().iter().map(Vec::len).collect();
    let fresh: Vec<_> = sampler.open(&p, &a, 0).unwrap().collect();
    assert!(!fresh.is_empty());
    assert_ne!(fresh, old);
    assert!(old.iter().any(|m| p.is_move_doable(&a, m).is_err()));
    for m in fresh {
        let mut trial = a.clone();
        let undo = p.apply(&mut trial, &m).unwrap();
        p.validate(&undo).unwrap();
        assert_eq!(
            undo.lists().iter().map(Vec::len).collect::<Vec<_>>(),
            current_lengths
        );
    }
    let p = ListProblem::new_indexed(
        (0..16).collect(),
        vec![
            ListEntity::new((0..8).collect()).with_value_range((0..8).collect()),
            ListEntity::new((8..16).collect()).with_value_range((8..16).collect()),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = sampler.open(&p, &a, 0).unwrap().collect();
    assert!(!moves.is_empty());
    for m in moves {
        let mut trial = a.clone();
        let undo = p.apply(&mut trial, &m).unwrap();
        assert!(undo.lists()[0].iter().all(|&v| v < 8));
        assert!(undo.lists()[1].iter().all(|&v| v >= 8));
    }
}
fn rank(row: &[i32]) -> SimpleScore {
    SimpleScore::new(
        -row.iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 + 1) * i64::from(v))
            .sum::<i64>(),
    )
}
fn independent(a: &ListAssignment<i32>) -> SimpleScore {
    SimpleScore::new(
        a.lists()
            .iter()
            .flat_map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(i, &v)| -((i as i64 + 1) * i64::from(v)))
            })
            .sum(),
    )
}
#[derive(Clone)]
struct Observed {
    provider: RepairedKOptProvider,
    openings: Vec<(u64, ListAssignment<i32>)>,
}
impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> for Observed {
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<i32>,
        a: &'a ListAssignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<i32>> + 'a>
    where
        ListMove<i32>: 'a,
    {
        self.openings.push((step, a.clone()));
        self.provider.moves(p, a, step)
    }
}
#[test]
fn dense_and_stream_search_reopen_live_sampler_and_improve_independently_scored_routes() {
    let p = model(12, 16);
    let a = p.initial_assignment();
    let initial = independent(&a);
    let config = LocalSearchConfig {
        max_steps: 5,
        max_evaluations: 256,
        selection: MoveSelection::BestAccepted,
        acceptance: Acceptance::StrictImprovement,
        assert_scores: true,
        ..Default::default()
    };
    let mut n = ListConstraintNetwork::new(3, 32);
    for e in 0..3 {
        n.add_constraint(format!("rank{e}"), vec![ListScope::Entity(e)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            rank(row)
        })
        .unwrap();
    }
    let mut d = Observed {
        provider: provider(37, 64),
        openings: vec![],
    };
    let mut s = d.clone();
    let dense =
        list_local_search_from_with_provider(&p, &n, a.clone(), config.clone(), &mut d).unwrap();
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::new(&mut b);
    b.score("rank", sources.entities.port(), |(_, row)| rank(row))
        .unwrap();
    let stream = list_stream_local_search_from_with_provider(
        &p,
        b.build(),
        sources,
        a.clone(),
        config,
        &mut s,
    )
    .unwrap();
    assert_eq!(dense, stream);
    assert_eq!(d.openings, s.openings);
    assert!(d.openings.len() > 1);
    assert!(d.openings.windows(2).any(|w| w[0].1 != w[1].1));
    let best = dense.best.unwrap();
    p.validate(&best.assignment).unwrap();
    assert!(best.score > initial);
    assert_eq!(best.score, independent(&best.assignment));
    assert!(dense.evaluations > 0 && dense.evaluations <= 256);
}
