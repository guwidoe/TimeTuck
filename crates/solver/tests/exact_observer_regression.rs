//! Original observer eligibility, termination and ownership contracts.
use timetuck::*;
#[test]
fn exact_observers_skip_ties_and_rejections_preserve_counts_and_can_cancel() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3])]).unwrap();
    let mut snapshots = Vec::new();
    let score = |a: &Assignment<i64>| match a.values()[0].unwrap() {
        0 => SimpleScore::new(99).with_structural(-1),
        1 | 2 => SimpleScore::new(1),
        _ => SimpleScore::new(2),
    };
    let r = exhaustive_search_with_observer(&basic, score, ExactSearchConfig::default(), |b| {
        snapshots.push(b.clone())
    });
    assert_eq!(
        r,
        exhaustive_search_with_config(&basic, score, ExactSearchConfig::default())
    );
    assert_eq!(r.evaluations, 4);
    assert_eq!(
        snapshots
            .iter()
            .map(|b| b.assignment.values()[0])
            .collect::<Vec<_>>(),
        vec![Some(1), Some(3)]
    );
    let token = CancellationToken::new();
    let r = exhaustive_search_with_observer(
        &basic,
        score,
        ExactSearchConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        |_| {
            token.cancel();
        },
    );
    assert_eq!(r.evaluations, 2);
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.best.unwrap(), snapshots[0]);
    let r = exhaustive_search_with_observer(
        &basic,
        score,
        ExactSearchConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        |_| panic!("zero budget"),
    );
    assert!(r.best.is_none());
    assert_eq!(r.evaluations, 0);
}
#[test]
fn list_and_mixed_observers_have_same_eligibility_and_termination_contract() {
    let lists =
        ListProblem::new(vec![7], (0..4).map(|_| ListEntity::new(vec![])).collect()).unwrap();
    let score = |a: &ListAssignment<i32>| match a.location(&7).unwrap().entity {
        0 => SimpleScore::new(99).with_structural(-1),
        1 | 2 => SimpleScore::new(1),
        _ => SimpleScore::new(2),
    };
    let mut list = Vec::new();
    let r =
        exhaustive_list_search_with_observer(&lists, score, ExactSearchConfig::default(), |b| {
            list.push(b.clone())
        });
    assert_eq!(
        r,
        exhaustive_list_search_with_config(&lists, score, ExactSearchConfig::default())
    );
    assert_eq!(
        list.iter()
            .map(|b| b.assignment.location(&7).unwrap().entity)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let token = CancellationToken::new();
    let r = exhaustive_list_search_with_observer(
        &lists,
        score,
        ExactSearchConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        |_| {
            token.cancel();
        },
    );
    assert_eq!(r.evaluations, 2);
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.best.unwrap(), list[0]);
    let mixed = MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), lists);
    let mut bests = Vec::new();
    let r = exhaustive_mixed_search_with_observer(
        &mixed,
        |a| score(a.lists()),
        ExactSearchConfig::default(),
        |b| bests.push(b.clone()),
    );
    assert_eq!(
        r,
        exhaustive_mixed_search_with_config(
            &mixed,
            |a| score(a.lists()),
            ExactSearchConfig::default()
        )
    );
    assert_eq!(
        bests
            .iter()
            .map(|b| b.assignment.lists().location(&7).unwrap().entity)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    let token = CancellationToken::new();
    let r = exhaustive_mixed_search_with_observer(
        &mixed,
        |a| score(a.lists()),
        ExactSearchConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        |_| {
            token.cancel();
        },
    );
    assert_eq!(r.evaluations, 2);
    assert_eq!(r.termination, Termination::Cancelled);
    assert_eq!(r.best.unwrap(), bests[0]);
}
