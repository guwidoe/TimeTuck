//! Native sampling distribution, ownership, errors and bounded solver execution.
use SelectionLifecycleEvent as E;
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};
use timetuck::*;
struct Groups<T> {
    values: Vec<SelectionGroup<T>>,
    opens: usize,
}
impl<C, T> SelectionSource<C, SelectionGroup<T>> for Groups<T> {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = SelectionGroup<T>> + 'a>
    where
        SelectionGroup<T>: 'a,
    {
        self.opens += 1;
        Box::new(self.values.iter().cloned())
    }
}
fn groups(values: &[&[usize]]) -> Groups<usize> {
    Groups {
        values: values
            .iter()
            .map(|v| v.iter().map(|&x| Arc::new(x)).collect())
            .collect(),
        opens: 0,
    }
}
#[test]
fn all_ordered_without_replacement_draws_have_uniform_subset_multiplicity() {
    let mut s = SubgroupSelectionSource::new(
        groups(&[&[0, 1, 2]]),
        1,
        3,
        SubPillarSelection::combinations(2, 2).unwrap(),
        0,
    );
    let mut counts = BTreeMap::new();
    for a in 0..3 {
        for b in 0..2 {
            let mut draws = [0, 0, a, b].into_iter();
            let result = s
                .try_open_with(&(), |_| draws.next().unwrap())
                .unwrap()
                .next()
                .unwrap()
                .unwrap();
            *counts
                .entry(result.iter().map(|v| **v).collect::<Vec<_>>())
                .or_insert(0) += 1;
        }
    }
    assert_eq!(
        counts,
        BTreeMap::from([(vec![0, 1], 2), (vec![0, 2], 2), (vec![1, 2], 2)])
    );
    assert_eq!(s.source().opens, 1);
}
#[test]
fn invalid_group_size_member_and_window_draws_report_once_then_fuse() {
    for (policy, draws, bound, actual) in [
        (
            SubPillarSelection::combinations(1, 2).unwrap(),
            vec![1],
            1,
            1,
        ),
        (
            SubPillarSelection::combinations(1, 2).unwrap(),
            vec![0, 2],
            2,
            2,
        ),
        (
            SubPillarSelection::combinations(1, 2).unwrap(),
            vec![0, 0, 3],
            3,
            3,
        ),
        (
            SubPillarSelection::consecutive(2, 2).unwrap(),
            vec![0, 0, 2],
            2,
            2,
        ),
    ] {
        let mut s = SubgroupSelectionSource::new(groups(&[&[0, 1, 2]]), 1, 3, policy, 0);
        let mut draws = draws.into_iter();
        let mut iter = s.try_open_with(&(), |_| draws.next().unwrap()).unwrap();
        assert!(
            matches!(iter.next(),Some(Err(SubgroupSelectionError::InvalidDraw{bound:b,actual:a})) if b==bound && a==actual)
        );
        assert!(iter.next().is_none());
        assert!(iter.next().is_none());
    }
}
#[test]
fn caps_empty_limits_ordering_panic_and_source_edits_preserve_honest_sizes() {
    let mut capped = SubgroupSelectionSource::new(
        groups(&[&[1, 2], &[3, 4]]),
        2,
        3,
        SubPillarSelection::combinations(1, 1).unwrap(),
        0,
    );
    assert!(matches!(
        capped.try_open_with(&(), |_| 0),
        Err(SubgroupSelectionError::MemberLimit { maximum: 3 })
    ));
    capped.source_mut().values.pop();
    assert!(
        capped
            .try_open_with(&(), |_| 0)
            .unwrap()
            .next()
            .unwrap()
            .is_ok()
    );
    let mut group_cap = SubgroupSelectionSource::new(
        groups(&[&[1], &[2]]),
        1,
        3,
        SubPillarSelection::combinations(1, 1).unwrap(),
        0,
    );
    assert!(matches!(
        group_cap.try_open_with(&(), |_| 0),
        Err(SubgroupSelectionError::Collection(
            SelectionProductError::CandidateLimit { .. }
        ))
    ));
    let panic_now = Cell::new(true);
    let mut sorted = SubgroupSelectionSource::new(
        groups(&[&[3, 2, 1]]),
        1,
        3,
        SubPillarSelection::consecutive(2, 2).unwrap(),
        0,
    )
    .ordered_by(|_: &(), a: &usize, b: &usize| {
        assert!(!panic_now.get());
        a.cmp(b)
    });
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sorted.size(&()))).is_err());
    panic_now.set(false);
    let result = sorted
        .try_open_with(&(), |_| 0)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(result.iter().map(|v| **v).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(sorted.source().opens, 2);
    let empty = SubgroupSelectionSource::new(
        groups(&[&[], &[0]]),
        2,
        1,
        SubPillarSelection::combinations(2, 3).unwrap(),
        0,
    );
    let mut limited = LimitedSelectionSource::new(empty, 10);
    assert_eq!(limited.size(&()), Some(0));
    assert!(limited.open(&()).next().is_none());
}
struct Value {
    id: usize,
    drops: Arc<std::sync::atomic::AtomicUsize>,
}
impl Drop for Value {
    fn drop(&mut self) {
        self.drops
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
#[test]
fn large_nonclone_groups_sample_without_enumeration_and_keep_occurrence_identity() {
    let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let values: SelectionGroup<_> = (0..20000)
        .map(|id| {
            Arc::new(Value {
                id,
                drops: drops.clone(),
            })
        })
        .collect();
    let first = values[0].clone();
    let source = Groups {
        values: vec![values],
        opens: 0,
    };
    let mut s = SubgroupSelectionSource::new(
        source,
        1,
        20000,
        SubPillarSelection::combinations(40, 40).unwrap(),
        0,
    );
    let mut snapshot = s.try_open_with(&(), |_| 0).unwrap();
    s.event(E::PhaseEnded, &());
    drop(s);
    let group = snapshot.next().unwrap().unwrap();
    assert_eq!(
        group.iter().map(|v| v.id).collect::<Vec<_>>(),
        (0..40).collect::<Vec<_>>()
    );
    assert!(Arc::ptr_eq(&first, &group[0]));
    assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 0);
    drop(snapshot);
    assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 19960);
    drop(group);
    assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 19999);
    drop(first);
    assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 20000);
}
#[test]
fn seeded_streams_continue_across_openings_and_can_be_restarted() {
    let mut s = SubgroupSelectionSource::new(
        groups(&[&[1, 2, 3, 4, 5]]),
        1,
        5,
        SubPillarSelection::combinations(1, 4).unwrap(),
        913,
    );
    let first = s.open(&()).take(15).collect::<Vec<_>>();
    let second = s.open(&()).take(15).collect::<Vec<_>>();
    assert_ne!(first, second);
    s.reseed(913);
    let again = s.open(&()).take(30).collect::<Vec<_>>();
    assert_eq!(again, [first, second].concat());
    assert_eq!(s.source().opens, 1);
}
struct Entities(usize, Rc<Cell<usize>>);
impl SelectionSource<Assignment<i32>, usize> for Entities {
    fn size(&mut self, _: &Assignment<i32>) -> Option<usize> {
        Some(self.0)
    }
    fn open<'a>(&'a mut self, _: &'a Assignment<i32>) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.1.set(self.1.get() + 1);
        Box::new(0..self.0)
    }
}
#[test]
fn bounded_sampler_runs_scored_pillar_moves_and_empty_sources_through_the_solver() {
    for count in [0, 3] {
        let p = FiniteProblem::new(
            (0..3)
                .map(|_| Variable::new(vec![0, 1, 2]).with_initial(0))
                .collect(),
        )
        .unwrap();
        let mut n = ConstraintNetwork::new(3);
        n.add_constraint("sum", vec![0, 1, 2], |v| {
            SimpleScore::new(v.iter().map(|v| i64::from(v.copied().unwrap())).sum())
        })
        .unwrap();
        let opens = Rc::new(Cell::new(0));
        let grouped = GroupedSelectionSource::new(
            Entities(count, opens.clone()),
            3,
            |a: &Assignment<i32>, &l: &usize, &r: &usize| a.values()[l] == a.values()[r],
        );
        let sampled = SubgroupSelectionSource::new(
            grouped,
            3,
            3,
            SubPillarSelection::combinations(3, 3).unwrap(),
            42,
        );
        let mut provider = CachedMoveProvider::uniform(
            LimitedSelectionSource::new(sampled, 4),
            CachedProviderConfig {
                maximum_candidates: 4,
                maximum_draws: 4,
                ..Default::default()
            },
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| a.clone(),
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64, g: &SelectionGroup<usize>| {
                let value = a.values()[*g[0]].unwrap() + 1;
                (value <= 2).then(|| {
                    Move::Composite(
                        g.iter()
                            .map(|i| Move::Change {
                                variable: **i,
                                value: Some(value),
                            })
                            .collect(),
                    )
                })
            },
        )
        .unwrap();
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut provider);
        let result = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 5,
                    selection: MoveSelection::BestAccepted,
                    acceptance: Acceptance::HillClimbing,
                    assert_scores: true,
                    ..Default::default()
                })],
                ..Default::default()
            },
            ext,
        )
        .unwrap();
        let value = if count == 0 { 0 } else { 2 };
        assert_eq!(result.assignment.values(), &[Some(value); 3]);
        assert_eq!(result.score, Some(SimpleScore::new(i64::from(value) * 3)));
        assert_eq!(opens.get(), if count == 0 { 1 } else { 3 });
    }
}
