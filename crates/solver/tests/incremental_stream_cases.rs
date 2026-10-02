// Adapted from pinned Timefold Community 2.7.0 AbstractUniConstraintStreamTest.
// Apache-2.0. Fixtures/helper adaptations and each method are in compat/provenance.json.
// Stable IDs replace Java object identities; native delta delivery replaces notifications.
use timetuck::{
    FilterNode, GroupNode, IntSummary, JoinNode, ScoreNode, SimpleScore, TupleChange as Change,
};

fn assert_terminal<I: Ord + Clone + std::fmt::Debug, T>(
    score: &ScoreNode<'_, I, T, SimpleScore>,
    expected: &[I],
) {
    let mut expected = expected.to_vec();
    expected.sort();
    assert_eq!(
        score
            .contributions()
            .map(|(id, s)| (id.clone(), s))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .cloned()
            .map(|id| (id, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
    assert_eq!(score.score(), SimpleScore::new(-(expected.len() as i64)));
}

#[test]
fn filter_problem_fact() {
    let mut filter = FilterNode::new(|code: &String| code.starts_with("MyValueGroup"));
    let mut score = ScoreNode::new(|_: &String| SimpleScore::new(-1));
    for (id, code) in [
        "Generated ValueGroup 0",
        "Generated ValueGroup 1",
        "MyValueGroup 1",
        "MyValueGroup 2",
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(change) = filter.apply(Change::put(id, code.to_owned())) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[2, 3]);
    assert_eq!(filter.rows().map(|(id, _)| *id).collect::<Vec<_>>(), [2, 3]);
    score
        .apply(
            filter
                .apply(Change::put(2, "Other code".to_owned()))
                .unwrap(),
        )
        .unwrap();
    assert_terminal(&score, &[3]);
    assert_eq!(filter.rows().map(|(id, _)| *id).collect::<Vec<_>>(), [3]);
}

#[test]
fn filter_entity() {
    let mut filter = FilterNode::new(|group: &usize| *group == 3);
    let mut score = ScoreNode::new(|_: &usize| SimpleScore::new(-1));
    for id in 0..10 {
        let group = match id {
            7 | 8 => 3,
            9 => 0,
            _ => id % 3,
        };
        if let Some(change) = filter.apply(Change::put(id, group)) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[7, 8]);
    for (change, expected) in [
        (Change::put(9, 3), vec![7, 8, 9]),
        (Change::retract(9), vec![7, 8]),
        (Change::put(9, 3), vec![7, 8, 9]),
    ] {
        score.apply(filter.apply(change).unwrap()).unwrap();
        assert_terminal(&score, &expected);
        assert_eq!(
            filter.rows().map(|(id, _)| *id).collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn filter_consecutive() {
    let mut filters = [
        FilterNode::new(|id: &usize| *id != 0),
        FilterNode::new(|id: &usize| *id != 1),
        FilterNode::new(|id: &usize| *id != 2),
    ];
    let mut score = ScoreNode::new(|_: &usize| SimpleScore::new(-1));
    for id in 0..4 {
        let mut change = Some(Change::put(id, id));
        for filter in &mut filters {
            change = change.and_then(|change| filter.apply(change));
        }
        if let Some(change) = change {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[3]);
    let mut change = Some(Change::retract(3));
    for filter in &mut filters {
        change = change.and_then(|change| filter.apply(change));
    }
    score.apply(change.unwrap()).unwrap();
    assert_terminal(&score, &[]);
}

#[test]
fn join_0() {
    let mut join = JoinNode::new(|_: &usize| (), |_: &usize| ());
    let mut score = ScoreNode::new(|_: &(usize, usize)| SimpleScore::new(-1));
    for id in 0..2 {
        for change in join.apply_left(Change::put(id, id)) {
            score.apply(change).unwrap();
        }
    }
    for id in 0..2 {
        for change in join.apply_right(Change::put(id, id)) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[(0, 0), (0, 1), (1, 0), (1, 1)]);
    assert_eq!(
        join.matches()
            .map(|(_, (&a, &b))| (a, b))
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1), (1, 0), (1, 1)]
    );
    for change in join.apply_right(Change::retract(1)) {
        score.apply(change).unwrap();
    }
    assert_terminal(&score, &[(0, 0), (1, 0)]);
    assert_eq!(
        join.matches()
            .map(|(_, (&a, &b))| (a, b))
            .collect::<Vec<_>>(),
        [(0, 0), (1, 0)]
    );
}

type SelfJoin = JoinNode<'static, usize, usize, (usize, usize), (usize, usize), usize>;
type SelfScore = ScoreNode<'static, (usize, usize), ((usize, usize), (usize, usize)), SimpleScore>;
fn update(join: &mut SelfJoin, score: &mut SelfScore, id: usize, value: usize) {
    for change in join.apply_left(Change::put(id, (id, value))) {
        score.apply(change).unwrap();
    }
    for change in join.apply_right(Change::put(id, (id, value))) {
        score.apply(change).unwrap();
    }
}
fn assert_join(join: &SelfJoin, score: &SelfScore, mut expected: Vec<(usize, usize)>) {
    assert_terminal(score, &expected);
    expected.sort();
    assert_eq!(
        join.matches()
            .map(|(_, (a, b))| (a.0, b.0))
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn join_1_equal() {
    let mut join = SelfJoin::new(|entity| entity.1, |entity| entity.1);
    let mut score = SelfScore::new(|_| SimpleScore::new(-1));
    // Fixture has five generated values. New value uses identity 5.
    update(&mut join, &mut score, 0, 0);
    update(&mut join, &mut score, 1, 5);
    update(&mut join, &mut score, 2, 0);
    assert_join(&join, &score, vec![(0, 0), (0, 2), (1, 1), (2, 0), (2, 2)]);
    update(&mut join, &mut score, 2, 5);
    assert_join(&join, &score, vec![(0, 0), (1, 1), (1, 2), (2, 1), (2, 2)]);
    update(&mut join, &mut score, 0, 5);
    update(&mut join, &mut score, 2, 0);
    assert_join(&join, &score, vec![(0, 0), (1, 1), (0, 1), (1, 0), (2, 2)]);
}

#[test]
fn group_by_1_mapping_0_collector() {
    let mut group = GroupNode::new();
    let mut score = ScoreNode::new(|_: &IntSummary| SimpleScore::new(-1));
    // Seven generated entities in group 0, two new entities in group 1,
    // and another new entity in group 0.
    for id in 0..10 {
        for change in group.apply(Change::put(id, (usize::from(id == 7 || id == 8), 1))) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[0, 1]);
    assert_eq!(
        group.groups().map(|(key, _)| *key).collect::<Vec<_>>(),
        [0, 1]
    );
    for id in [7, 8] {
        for change in group.apply(Change::retract(id)) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[0]);
    assert_eq!(group.groups().map(|(key, _)| *key).collect::<Vec<_>>(), [0]);
}

#[test]
fn group_by_1_mapping_1_collector() {
    let mut group = GroupNode::new();
    let mut score = ScoreNode::new(|_: &IntSummary| SimpleScore::new(-1));
    for id in 0..3 {
        for change in group.apply(Change::put(id, (id % 2, 1))) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[0, 1]);
    assert_eq!(
        group
            .groups()
            .map(|(key, s)| (*key, s.count))
            .collect::<Vec<_>>(),
        [(0, 2), (1, 1)]
    );
    for id in [0, 1] {
        for change in group.apply(Change::retract(id)) {
            score.apply(change).unwrap();
        }
    }
    assert_terminal(&score, &[0]);
    assert_eq!(
        group
            .groups()
            .map(|(key, s)| (*key, s.count))
            .collect::<Vec<_>>(),
        [(0, 1)]
    );
}
