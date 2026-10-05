use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn intervals_disable_zero_weights_and_preserve_duplicate_occurrences() {
    let s = WeightedSelection::new(vec![
        ("disabled", 0.0),
        ("a", 1.0),
        ("disabled", -0.0),
        ("a", 3.0),
        ("b", 4.0),
        ("disabled", 0.0),
    ])
    .unwrap();
    assert_eq!(s.len(), 6);
    assert_eq!(s.active_len(), 3);
    assert!(!s.is_empty());
    for (unit, index) in [
        (0.0, 1),
        (0.124, 1),
        (0.125, 3),
        (0.49, 3),
        (0.5, 4),
        (f64::from_bits(1.0_f64.to_bits() - 1), 4),
    ] {
        assert!(std::ptr::eq(
            s.choose(unit).unwrap().unwrap(),
            &s.items()[index]
        ));
    }
    let mut counts = [0; 6];
    for i in 0..8 {
        let selected = s.choose((i as f64 + 0.5) / 8.0).unwrap().unwrap();
        let index = s
            .items()
            .iter()
            .position(|v| std::ptr::eq(v, selected))
            .unwrap();
        counts[index] += 1;
    }
    assert_eq!(counts, [0, 1, 0, 3, 4, 0]);
}
#[test]
fn empty_disabled_and_invalid_inputs_are_explicit() {
    for entries in [vec![], vec![(1, 0.0), (2, -0.0)]] {
        let s = WeightedSelection::new(entries).unwrap();
        assert_eq!(s.active_len(), 0);
        assert_eq!(s.choose(0.0).unwrap(), None);
        let mut it = s.samples_with(|| panic!("no selectable occurrence"));
        assert_eq!(it.next(), None);
        assert_eq!(it.next(), None);
    }
    for weight in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            WeightedSelection::new(vec![(0, 1.0), (1, weight)]),
            Err(WeightedSelectionError::InvalidWeight { index: 1 })
        ));
    }
    let s = WeightedSelection::new(vec![(0, 1.0)]).unwrap();
    for draw in [-0.1, 1.0, 2.0, f64::NAN, f64::INFINITY] {
        assert_eq!(s.choose(draw), Err(WeightedSelectionError::InvalidDraw));
    }
    assert!(catch_unwind(AssertUnwindSafe(|| s.samples_with(|| 1.0).next())).is_err());
}
#[test]
fn power_of_two_scaling_handles_maximum_and_subnormal_weights() {
    for unit in [f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)] {
        let s = WeightedSelection::new(vec![(0, unit), (1, unit)]).unwrap();
        assert_eq!(s.choose(0.0).unwrap(), Some(&0));
        assert_eq!(s.choose(0.49).unwrap(), Some(&0));
        assert_eq!(s.choose(0.5).unwrap(), Some(&1));
        assert_eq!(s.choose(0.99).unwrap(), Some(&1));
    }
    let tiny = f64::from_bits(1);
    let s = WeightedSelection::new(vec![(0, tiny), (1, tiny * 3.0)]).unwrap();
    assert_eq!(s.choose(0.249).unwrap(), Some(&0));
    assert_eq!(s.choose(0.25).unwrap(), Some(&1));
    assert!(matches!(
        WeightedSelection::new(vec![(0, 1.0), (1, f64::EPSILON / 4.0)]),
        Err(WeightedSelectionError::PrecisionLoss { index: 1 })
    ));
    assert!(matches!(
        WeightedSelection::new(vec![(0, tiny), (1, f64::MAX)]),
        Err(WeightedSelectionError::PrecisionLoss { index: 0 })
    ));
}
#[test]
fn snapshots_reweight_nonclone_items_without_mutating_existing_iterators() {
    struct Item(usize);
    let s = WeightedSelection::new(vec![(Item(0), 1.0), (Item(1), 3.0)]).unwrap();
    let old = s.clone();
    let mut before = s.samples_with(|| 0.5);
    let mut calls = vec![];
    let updated = s
        .reweighted(|i, item| {
            calls.push((i, item.0));
            if i == 0 { 1.0 } else { 0.0 }
        })
        .unwrap();
    assert_eq!(calls, [(0, 0), (1, 1)]);
    assert_eq!(before.next().unwrap().0, 1);
    assert_eq!(updated.choose(0.5).unwrap().unwrap().0, 0);
    assert_eq!(old.choose(0.5).unwrap().unwrap().0, 1);
    assert!(std::ptr::eq(updated.items().as_ptr(), s.items().as_ptr()));
    assert!(s.reweighted(|_, _| -1.0).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(
            || s.reweighted(|_, _| panic!("weight failure"))
        ))
        .is_err()
    );
    assert_eq!(before.next().unwrap().0, 1);
}
#[test]
fn bounded_sources_stop_and_owned_sampling_clones_only_selected_items() {
    let reads = Cell::new(0);
    let source = std::iter::repeat_with(|| {
        reads.set(reads.get() + 1);
        (0, 1.0)
    });
    assert!(matches!(
        WeightedSelection::from_bounded_iter(source, 3),
        Err(WeightedSelectionError::CandidateLimit { maximum: 3 })
    ));
    assert_eq!(reads.get(), 4);
    assert_eq!(
        WeightedSelection::from_bounded_iter([(1, 1.0), (2, 1.0)], 2)
            .unwrap()
            .len(),
        2
    );
    assert!(
        WeightedSelection::<i32>::from_bounded_iter([], 0)
            .unwrap()
            .is_empty()
    );
    struct Item(Rc<Cell<usize>>);
    impl Clone for Item {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    let clones = Rc::new(Cell::new(0));
    let s = WeightedSelection::new(vec![(Item(clones.clone()), 1.0)]).unwrap();
    let mut it = s.into_samples(0);
    assert_eq!(clones.get(), 0);
    for _ in 0..5 {
        assert!(it.next().is_some());
    }
    assert_eq!(clones.get(), 5);
}
#[test]
fn seeded_distribution_is_reproducible_and_tracks_fractional_weights() {
    let s = WeightedSelection::new(vec![(0, 0.125), (1, 0.375), (2, 0.75)]).unwrap();
    assert_eq!(
        s.samples(42).take(1000).collect::<Vec<_>>(),
        s.samples(42).take(1000).collect::<Vec<_>>()
    );
    assert_ne!(
        s.samples(42).take(100).collect::<Vec<_>>(),
        s.samples(43).take(100).collect::<Vec<_>>()
    );
    let mut counts = [0usize; 3];
    for v in s.samples(0).take(100_000) {
        counts[*v] += 1;
    }
    for (count, p) in counts.into_iter().zip([0.1_f64, 0.3, 0.6]) {
        let expected = 100_000.0 * p;
        assert!((count as f64 - expected).abs() < 6.0 * (expected * (1.0 - p)).sqrt());
    }
}
#[test]
fn weighted_candidates_solve_through_both_basic_scoring_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 3,
        max_move_evaluations: Some(20),
        assert_scores: true,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut steps = vec![];
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64| {
            steps.push(step);
            let entries = change_moves(p, a).unwrap().map(|m| {
                let weight = match &m {
                    Move::Change { value: Some(3), .. } => 1000.0,
                    _ => 0.5,
                };
                (m, weight)
            });
            WeightedSelection::from_bounded_iter(entries, 3)
                .unwrap()
                .into_samples(step + 7)
                .take(4)
                .collect::<Vec<_>>()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |row| {
                SimpleScore::new(i64::from(row.1.unwrap()))
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
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.assignment.values(), &[Some(3)]);
        assert_eq!(best.score, SimpleScore::new(3));
        assert!(steps.starts_with(&[0, 1]));
    }
}
