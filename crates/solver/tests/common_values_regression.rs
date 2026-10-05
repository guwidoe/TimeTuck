//! Original budget, immutable identity, membership and group-solving checks.
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
fn shared(v: impl IntoIterator<Item = i32>) -> SharedValueRange<'static, i32> {
    Arc::new(EnumeratedValueRange::new(v))
}
#[test]
fn exact_picks_match_all_small_intersections_and_exclusions() {
    for left in 0u32..16 {
        for right in 0u32..16 {
            let a: Vec<_> = (0..4).filter(|i| left & (1 << i) != 0).collect();
            let b: Vec<_> = (0..4).filter(|i| right & (1 << i) != 0).collect();
            let ranges = CommonValueRanges::new([shared(a.clone()), shared(b.clone())]).unwrap();
            for exclude in [None, Some(0), Some(2), Some(7)] {
                let legal: BTreeSet<_> = a
                    .iter()
                    .copied()
                    .filter(|v| b.contains(v) && Some(*v) != exclude)
                    .collect();
                let mut reached = BTreeSet::new();
                for seed in 0..64 {
                    let picked = ranges.pick_exact(exclude.as_ref(), seed, 4).unwrap();
                    match picked {
                        Some(v) => {
                            assert!(legal.contains(&v));
                            reached.insert(v);
                        }
                        None => assert!(legal.is_empty()),
                    }
                }
                assert_eq!(reached, legal);
            }
        }
    }
}
struct CountedRange {
    size: u128,
    gets: Arc<AtomicUsize>,
    contains: Arc<AtomicUsize>,
    constant: Option<u128>,
}
impl ValueRange for CountedRange {
    type Value = u128;
    fn len(&self) -> u128 {
        self.size
    }
    fn get(&self, i: u128) -> Option<u128> {
        self.gets.fetch_add(1, Ordering::Relaxed);
        (i < self.size).then_some(self.constant.unwrap_or(i))
    }
    fn contains(&self, v: &u128) -> bool {
        self.contains.fetch_add(1, Ordering::Relaxed);
        self.size != 0 && self.constant.map_or(*v < self.size, |a| a == *v)
    }
}
#[test]
fn huge_sources_obey_limits_and_never_report_a_budget_miss_as_empty() {
    let gets = Arc::new(AtomicUsize::new(0));
    let contains = Arc::new(AtomicUsize::new(0));
    let r: SharedValueRange<'_, u128> = Arc::new(CountedRange {
        size: u128::MAX,
        gets: gets.clone(),
        contains: contains.clone(),
        constant: Some(0),
    });
    let ranges = CommonValueRanges::new([r]).unwrap();
    assert_eq!(gets.load(Ordering::Relaxed), 0);
    assert_eq!(
        ranges.pick_exact(None, 1, 9),
        Err(CommonValueError::ScanLimit {
            size: u128::MAX,
            limit: 9
        })
    );
    assert_eq!(gets.load(Ordering::Relaxed), 0);
    assert_eq!(
        ranges.find_target(
            Some(&0),
            2,
            CommonValueBudget {
                random_draws: 3,
                max_exact_values: 9
            }
        ),
        Err(CommonValueError::ScanLimit {
            size: u128::MAX,
            limit: 9
        })
    );
    assert_eq!(gets.load(Ordering::Relaxed), 3);
    assert_eq!(intersection_probe_budget(0), 0);
    assert_eq!(intersection_probe_budget(1), 10);
    assert_eq!(intersection_probe_budget(u128::MAX), u64::MAX);
}
#[test]
fn failed_probes_fall_back_and_empty_smallest_range_needs_no_reads() {
    let gets = Arc::new(AtomicUsize::new(0));
    let membership = Arc::new(AtomicUsize::new(0));
    let source: SharedValueRange<'_, u128> = Arc::new(CountedRange {
        size: 3,
        gets: gets.clone(),
        contains: membership.clone(),
        constant: None,
    });
    let other: SharedValueRange<'_, u128> = Arc::new(EnumeratedValueRange::new([2, 4, 5]));
    let ranges = CommonValueRanges::new([source.clone(), other]).unwrap();
    assert_eq!(
        ranges
            .find_target(
                None,
                8,
                CommonValueBudget {
                    random_draws: 0,
                    max_exact_values: 3
                }
            )
            .unwrap(),
        Some(2)
    );
    assert_eq!(gets.load(Ordering::Relaxed), 3);
    assert_eq!(
        ranges
            .find_target(
                Some(&2),
                8,
                CommonValueBudget {
                    random_draws: 2,
                    max_exact_values: 3
                }
            )
            .unwrap(),
        None
    );
    assert_eq!(gets.load(Ordering::Relaxed), 8);
    let empty: SharedValueRange<'_, u128> = Arc::new(EnumeratedValueRange::new([]));
    let ranges = CommonValueRanges::new([source, empty]).unwrap();
    let before = gets.load(Ordering::Relaxed);
    assert_eq!(
        ranges
            .find_target(
                None,
                2,
                CommonValueBudget {
                    random_draws: u64::MAX,
                    max_exact_values: 0
                }
            )
            .unwrap(),
        None
    );
    assert_eq!(gets.load(Ordering::Relaxed), before);
}
#[test]
fn identity_dedup_and_variable_domain_dedup_have_explicit_boundaries() {
    let a = shared([1, 2]);
    let b = shared([1, 2]);
    assert_eq!(
        CommonValueRanges::new([a.clone(), a.clone()])
            .unwrap()
            .range_count(),
        1
    );
    assert_eq!(
        CommonValueRanges::new([a.clone(), b.clone()])
            .unwrap()
            .range_count(),
        2
    );
    assert_ne!(
        CommonValueRanges::new([a]).unwrap(),
        CommonValueRanges::new([b]).unwrap()
    );
    assert!(matches!(
        CommonValueRanges::<i32>::new([]),
        Err(CommonValueError::EmptyGroup)
    ));
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]),
        Variable::new(vec![1, 2]),
        Variable::new(vec![2, 3]),
    ])
    .unwrap();
    assert_eq!(
        CommonValueRanges::for_variables(&p, &[0, 1, 2])
            .unwrap()
            .range_count(),
        2
    );
    assert!(matches!(
        CommonValueRanges::for_variables(&p, &[]),
        Err(CommonValueError::EmptyGroup)
    ));
    assert!(matches!(
        CommonValueRanges::for_variables(&p, &[0, 0]),
        Err(CommonValueError::DuplicateVariable { variable: 0 })
    ));
    assert!(matches!(
        CommonValueRanges::for_variables(&p, &[3]),
        Err(CommonValueError::VariableIndex {
            variable: 3,
            count: 3
        })
    ));
}
#[test]
fn group_moves_validate_members_pins_noops_and_unassigned_sources() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1),
        Variable::new(vec![2, 3]).allowing_unassigned(),
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let budget = CommonValueBudget::default();
    let m = sample_common_change(&p, &a, &[0, 1], 3, budget)
        .unwrap()
        .unwrap();
    let mut copy = a.clone();
    {
        let trial = p.apply(&mut copy, &m).unwrap();
        assert_eq!(trial.values(), &[Some(2), Some(2), Some(1)]);
    }
    assert_eq!(copy, a);
    assert!(matches!(
        sample_common_change(&p, &a, &[0, 2], 3, budget),
        Err(CommonValueError::PinnedVariable { variable: 2 })
    ));
    assert!(sample_common_change(&p, &Assignment::new(vec![]), &[0], 3, budget).is_err());
    let singleton = FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap();
    assert_eq!(
        sample_common_change(&singleton, &singleton.initial_assignment(), &[0], 3, budget).unwrap(),
        None
    );
    assert!(matches!(
        sample_common_change(&p, &a, &[0], 3, budget).unwrap(),
        Some(Move::Change {
            variable: 0,
            value: Some(2)
        })
    ));
}
#[test]
fn common_change_provider_escapes_single_move_trap_in_both_backends() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0),
    ])
    .unwrap();
    let quality = |a: i32, b: i32| SimpleScore::new(if a == b { i64::from(a) * 10 } else { -10 });
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("pair", vec![0, 1], |v| {
        quality(*v[0].unwrap(), *v[1].unwrap())
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 3,
        max_move_evaluations: Some(20),
        assert_scores: true,
        ..Default::default()
    };
    let mut single = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
        change_moves(p, a).unwrap().collect::<Vec<_>>()
    };
    assert_eq!(
        local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut single)
            .unwrap()
            .best
            .unwrap()
            .score,
        SimpleScore::ZERO
    );
    for stream in [false, true] {
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, step: u64| {
            sample_common_change(p, a, &[0, 1], step, CommonValueBudget::default())
                .unwrap()
                .into_iter()
                .collect::<Vec<_>>()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            let l = b.filter(source.port(), |r| r.0 == 0).unwrap();
            let r = b.filter(source.port(), |r| r.0 == 1).unwrap();
            let pairs = b.join_equal(&l, &r, |_| (), |_| ()).unwrap();
            b.score("pair", &pairs, move |(l, r)| {
                quality(l.1.unwrap(), r.1.unwrap())
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
        assert_eq!(best.assignment.values(), &[Some(1), Some(1)]);
        assert_eq!(best.score, SimpleScore::new(10));
        assert_eq!(result.completed_steps, 1);
    }
}
