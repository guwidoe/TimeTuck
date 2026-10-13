//! Original real execution/undo, retained-edge reconstruction and invalid draws.
#[path = "fixtures/sequential_kopt_fixture.rs"]
mod fixture;
use timetuck::*;
#[test]
fn varied_odd_k_executes_real_local_cycles_and_undo() {
    for k in [3, 5, 7, 9] {
        let f = fixture::fixture(&vec![0; k]);
        let mut values = f.values.into_iter();
        let mut origin = Some(f.origin);
        let p = &f.problem;
        let a = p.initial_assignment();
        let selector =
            ListKOptSequentialSelector::new(ListKOptConfig::new(k, k).unwrap(), vec![7]).unwrap();
        let proposal = selector
            .select(p, &a, 6, || origin.take(), || values.next(), || true)
            .unwrap();
        assert_eq!(proposal.removed_values(), f.expected);
        assert_eq!(proposal.endpoint_partners(), fixture::expected_partners(k));
        assert_eq!(proposal.physical_plan().physical_cycle_count(), 1);
        let anchor = ListPosition {
            entity: 0,
            index: 0,
        };
        let order = proposal
            .physical_plan()
            .cycle_positions(anchor, false)
            .unwrap();
        // Independent undirected edge multiset from actual cuts plus additions.
        let row = &a.lists()[0];
        let edge = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
        let mut expected: Vec<_> = (0..row.len())
            .map(|i| edge(i, (i + 1) % row.len()))
            .collect();
        for &(a, b) in proposal.removed_edges() {
            let i = expected
                .iter()
                .position(|&e| e == edge(a.index, b.index))
                .unwrap();
            expected.remove(i);
        }
        let slots: Vec<_> = proposal
            .removed_edges()
            .iter()
            .flat_map(|&(a, b)| [a, b])
            .collect();
        for (i, &j) in proposal.endpoint_partners().iter().enumerate() {
            if i < j {
                expected.push(edge(slots[i].index, slots[j].index));
            }
        }
        let mut actual: Vec<_> = (0..order.len())
            .map(|i| edge(order[i].index, order[(i + 1) % order.len()].index))
            .collect();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected);
        let m = proposal
            .physical_plan()
            .movement(p, &a, anchor, false, &[row.len()])
            .unwrap();
        let mut working = a.clone();
        {
            let _undo = p.apply(&mut working, &m).unwrap();
        }
        assert_eq!(working, a);
        p.apply(&mut working, &m).unwrap().commit();
        p.validate(&working).unwrap();
        assert_ne!(working, a);
    }
}
#[test]
fn pins_wrap_direction_and_current_snapshot_draws() {
    let p = ListProblem::new(
        (0..12).collect(),
        vec![ListEntity::new((0..12).collect()).with_pinned_prefix(2)],
    )
    .unwrap();
    let a = p.initial_assignment();
    let s = ListKOptSequentialSelector::new(ListKOptConfig::new(3, 3).unwrap(), vec![1]).unwrap();
    let mut values = [6, 9].into_iter();
    let q = s
        .select(&p, &a, 0, || Some(2), || values.next(), || false)
        .unwrap();
    assert_eq!(q.removed_values(), [2, 11, 6, 5, 9, 8]);
    assert_eq!(q.physical_plan().physical_cycle_count(), 1);
    {
        let m = q
            .physical_plan()
            .movement(
                &p,
                &a,
                ListPosition {
                    entity: 0,
                    index: 2,
                },
                false,
                &[10],
            )
            .unwrap();
        let mut b = a.clone();
        p.apply(&mut b, &m).unwrap().commit();
        assert_eq!(&b.lists()[0][..2], &[0, 1]);
        p.validate(&b).unwrap();
        let mut values = [6, 9].into_iter();
        let next = s
            .select(&p, &b, 0, || Some(2), || values.next(), || true)
            .unwrap();
        {
            let row = &b.lists()[0];
            let i = row.iter().position(|v| *v == 2).unwrap();
            let j = if i + 1 == row.len() { 2 } else { i + 1 };
            assert_eq!(&next.removed_values()[..2], &[2, row[j]]);
        }
    }
}
#[test]
fn invalid_weights_draws_missing_pinned_and_duplicate_cuts_reject() {
    let tiny = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let huge = ListKOptSequentialSelector::new(
        ListKOptConfig::new(usize::MAX, usize::MAX).unwrap(),
        vec![1],
    )
    .unwrap();
    assert!(matches!(
        huge.select(
            &tiny,
            &tiny.initial_assignment(),
            0,
            || panic!("oversized k drew origin"),
            || None,
            || true
        ),
        Err(SequentialKOptError::Topology(ListKOptError::TooFewValues))
    ));
    let config = ListKOptConfig::new(2, 5).unwrap();
    assert!(ListKOptSequentialSelector::new(config, vec![1]).is_err());
    assert!(ListKOptSequentialSelector::new(config, vec![0; 4]).is_err());
    assert!(ListKOptSequentialSelector::new(config, vec![u64::MAX; 4]).is_err());
    let s = ListKOptSequentialSelector::new(config, vec![1; 4]).unwrap();
    let f = fixture::fixture(&[0, 0, 0]);
    let a = f.problem.initial_assignment();
    assert!(matches!(
        s.select(
            &f.problem,
            &a,
            4,
            || panic!("bad draw read origin"),
            || None,
            || true
        ),
        Err(SequentialKOptError::InvalidDraw)
    ));
    assert!(matches!(
        s.select(
            &f.problem,
            &a,
            0,
            || panic!("even k read origin"),
            || None,
            || true
        ),
        Err(SequentialKOptError::UnsupportedK(2))
    ));
    assert!(matches!(
        s.select(&f.problem, &a, 1, || None, || None, || true),
        Err(SequentialKOptError::MissingValue)
    ));
    assert!(
        s.select(
            &f.problem,
            &a,
            1,
            || Some("v0".into()),
            || Some("v0".into()),
            || true
        )
        .is_err()
    );
    let p = ListProblem::new(
        vec![0, 1, 2, 3, 4, 5],
        vec![ListEntity::new(vec![0, 1, 2, 3, 4, 5]).with_pinned_prefix(1)],
    )
    .unwrap();
    assert!(
        s.select(
            &p,
            &p.initial_assignment(),
            1,
            || Some(0),
            || Some(3),
            || true
        )
        .is_err()
    );
    assert_eq!(a, f.problem.initial_assignment());
}
#[test]
fn independent_routes_merge_into_a_real_tour_with_explicit_lengths() {
    for k in [3, 5] {
        let f = fixture::fixture(&(0..k).collect::<Vec<_>>());
        let p = &f.problem;
        let a = p.initial_assignment();
        let mut origin = Some(f.origin);
        let mut values = f.values.into_iter();
        let s =
            ListKOptSequentialSelector::new(ListKOptConfig::new(k, k).unwrap(), vec![1]).unwrap();
        let q = s
            .select(p, &a, 0, || origin.take(), || values.next(), || true)
            .unwrap();
        let plan = q.physical_plan();
        assert_eq!(plan.selected_entities().len(), k);
        assert_eq!(plan.physical_cycle_count(), 1);
        let lengths: Vec<_> = plan
            .selected_entities()
            .iter()
            .map(|&e| a.lists()[e].len())
            .collect();
        let m = plan
            .movement(
                p,
                &a,
                ListPosition {
                    entity: 0,
                    index: 0,
                },
                false,
                &lengths,
            )
            .unwrap();
        let mut b = a.clone();
        p.apply(&mut b, &m).unwrap().commit();
        p.validate(&b).unwrap();
        assert_ne!(a, b);
        let mut old: Vec<_> = a.lists().iter().flatten().cloned().collect();
        let mut new: Vec<_> = b.lists().iter().flatten().cloned().collect();
        old.sort();
        new.sort();
        assert_eq!(old, new);
    }
}
