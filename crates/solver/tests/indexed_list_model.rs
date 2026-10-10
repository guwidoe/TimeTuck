//! Original native index regressions; these do not claim upstream method credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    rc::Rc,
};
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Collision(u32);
impl Hash for Collision {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state);
    }
}

#[test]
fn collisions_preserve_identity_ranges_and_cloned_models() {
    let values: Vec<_> = (0..24).map(Collision).collect();
    let p = ListProblem::new_indexed(
        values.clone(),
        vec![
            ListEntity::new(values[..12].to_vec()).with_value_range(values[..18].to_vec()),
            ListEntity::new(values[12..].to_vec()),
        ],
    )
    .unwrap();
    for (id, value) in values.iter().enumerate() {
        assert_eq!(p.value_index(value), Some(id));
    }
    assert_eq!(p.value_index(&Collision(24)), None);
    let mut a = p.initial_assignment();
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 11,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    p.apply(&mut a, &movement).unwrap().commit();
    let resumed = p.with_initial_assignment(&a).unwrap();
    assert_eq!(resumed.clone().initial_assignment(), a);
    assert_eq!(resumed.value_index(&Collision(11)), Some(11));
    assert_eq!(
        p.validate(&ListAssignment::new(vec![vec![Collision(23)], vec![]])),
        Err(ListError::ValueOutsideEntityRange { entity: 0 })
    );
    let mut duplicate = values;
    duplicate.push(Collision(0));
    assert!(matches!(
        ListProblem::new_indexed(duplicate, vec![]),
        Err(ListError::DuplicateDomainValue)
    ));
}

#[test]
fn constructor_and_validation_errors_match_including_priority_and_reports() {
    let cases = vec![
        (
            vec![1, 1],
            vec![ListEntity::new(vec![]).with_pinned_prefix(1)],
        ),
        (
            vec![1, 2],
            vec![ListEntity::new(vec![]).with_value_range(vec![1, 1, 3])],
        ),
        (
            vec![1, 2],
            vec![ListEntity::new(vec![]).with_value_range(vec![3, 1, 1])],
        ),
        (
            vec![1, 2],
            vec![
                ListEntity::new(vec![1])
                    .with_pinned(true)
                    .with_pinned_prefix(1),
            ],
        ),
        (vec![1, 2], vec![ListEntity::new(vec![1, 1])]),
        (
            vec![1, 2],
            vec![
                ListEntity::new(vec![3])
                    .with_range_diagnostics("entity", "tasks", |v| format!("task{v}")),
            ],
        ),
        (
            vec![1, 2],
            vec![
                ListEntity::new(vec![2])
                    .with_value_range(vec![1])
                    .with_range_diagnostics("entity", "tasks", |v| format!("task{v}")),
            ],
        ),
    ];
    for (values, entities) in cases {
        assert_eq!(
            ListProblem::new(values.clone(), entities.clone()).unwrap_err(),
            ListProblem::new_indexed(values, entities).unwrap_err()
        );
    }
    let entities = vec![
        ListEntity::new(vec![0]).with_pinned_prefix(1),
        ListEntity::new(vec![]).with_value_range(vec![1, 2]),
    ];
    let linear = ListProblem::new(vec![0, 1, 2], entities.clone()).unwrap();
    let indexed = ListProblem::new_indexed(vec![0, 1, 2], entities).unwrap();
    for a in [
        vec![],
        vec![vec![1], vec![9]],
        vec![vec![0, 1], vec![1]],
        vec![vec![0], vec![0]],
        vec![vec![0], vec![9]],
        vec![vec![0, 2], vec![1]],
    ] {
        let a = ListAssignment::new(a);
        assert_eq!(linear.validate(&a), indexed.validate(&a));
    }
}

fn graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<i32>::new(&mut b);
    b.score("neighbors", s.values.port(), |r| {
        SimpleScore::new(
            r.position.map_or(0, |p| (p.entity * 17 + p.index) as i64)
                + i64::from(r.previous.unwrap_or(0)) * 3
                + i64::from(r.next.unwrap_or(0)) * 7,
        )
    })
    .unwrap();
    (b.build(), s)
}

#[test]
fn every_small_candidate_preserves_stream_rows_order_scores_and_rollback() {
    for code in 0..27 {
        let mut rows = vec![vec![], vec![]];
        let mut digits = code;
        for value in [1, 2, 3] {
            let owner = digits % 3;
            digits /= 3;
            if owner < 2 {
                rows[owner].push(value);
            }
        }
        let entities: Vec<_> = rows.into_iter().map(ListEntity::new).collect();
        let linear = ListProblem::new(vec![1, 2, 3], entities.clone())
            .unwrap()
            .allowing_unassigned();
        let indexed = ListProblem::new_indexed(vec![1, 2, 3], entities)
            .unwrap()
            .allowing_unassigned();
        let a = linear.initial_assignment();
        let candidates = |p: &ListProblem<i32>| {
            list_anchored_value_changes(p, &a, p.values(), p.values(), true, false)
                .unwrap()
                .map(ListValueChange::into_move)
                .collect::<Vec<_>>()
        };
        let moves = candidates(&linear);
        assert_eq!(moves, candidates(&indexed));
        let (g, s) = graph();
        let ls = s.clone();
        let mut left = ListStreamScoreDirector::new(&linear, g, s, a.clone()).unwrap();
        let (g, s) = graph();
        let rs = s.clone();
        let mut right = ListStreamScoreDirector::new(&indexed, g, s, a.clone()).unwrap();
        for m in moves {
            let l = left.apply(&m).unwrap();
            let r = right.apply(&m).unwrap();
            assert_eq!(l.score(), r.score());
            assert_eq!(
                l.graph().source_rows(&ls.values).unwrap(),
                r.graph().source_rows(&rs.values).unwrap()
            );
            assert_eq!(l.assignment(), r.assignment());
            drop((l, r));
            assert_eq!(right.assignment(), &a);
            right.assert_consistent().unwrap();
        }
        let next = ListAssignment::new(vec![vec![3, 1], vec![2]]);
        left.replace_assignment(next.clone()).unwrap().commit();
        right.replace_assignment(next).unwrap().commit();
        assert_eq!(left.score(), right.score());
        assert_eq!(
            left.graph().source_rows(&ls.values).unwrap(),
            right.graph().source_rows(&rs.values).unwrap()
        );
    }
}

#[test]
fn dependencies_and_retained_solving_match_indexed_and_linear_models() {
    let entities = vec![ListEntity::new(vec![2, 1]), ListEntity::new(vec![])];
    let linear = ListProblem::new(vec![1, 2, 3], entities.clone())
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![]])
        .unwrap();
    let indexed = ListProblem::new_indexed(vec![1, 2, 3], entities)
        .unwrap()
        .with_dependencies(vec![vec![], vec![0], vec![]])
        .unwrap();
    assert_eq!(
        linear.inconsistent_values(&linear.initial_assignment()),
        indexed.inconsistent_values(&indexed.initial_assignment())
    );
    let config = SolverConfig {
        local_search: vec![LocalSearchConfig {
            max_steps: 5,
            ..Default::default()
        }],
        ..Default::default()
    };
    let run = |p: &ListProblem<i32>| {
        let (g, s) = graph();
        let a = p.prepare_assignment(&p.initial_assignment()).unwrap();
        let mut d = ListStreamScoreDirector::new(p, g, s, a).unwrap();
        d.run_solver(config.clone()).unwrap()
    };
    assert_eq!(run(&linear), run(&indexed));
}

#[derive(Clone)]
struct Counted(usize, Rc<Cell<usize>>);
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.1.set(self.1.get() + 1);
        self.0 == other.0
    }
}
impl Eq for Counted {}
impl Hash for Counted {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
#[test]
fn large_indexed_validation_and_construction_avoid_quadratic_equality_work() {
    let comparisons = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..2048).map(|i| Counted(i, comparisons.clone())).collect();
    let p = ListProblem::new_indexed(
        values.clone(),
        vec![ListEntity::new(values.clone()).with_value_range(values)],
    )
    .unwrap();
    assert!(comparisons.get() < 2048 * 10);
    comparisons.set(0);
    p.validate(&p.initial_assignment()).unwrap();
    assert!(comparisons.get() < 2048 * 3);
}

#[test]
fn mixed_search_and_repair_keep_identical_results_with_rebuilt_ranges() {
    let entities = vec![
        ListEntity::new(vec![3, 1]).with_value_range(vec![1, 2, 3]),
        ListEntity::new(vec![2]),
    ];
    let linear = ListProblem::new(vec![1, 2, 3], entities.clone()).unwrap();
    let indexed = ListProblem::new_indexed(vec![1, 2, 3], entities).unwrap();
    let run = |lists| {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap(),
            lists,
        );
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<i32, i32>::new(&mut b);
        b.score("basic", s.basic.port(), |(_, v)| {
            SimpleScore::new(i64::from(v.unwrap_or(0)))
        })
        .unwrap();
        b.score("lists", s.lists.values.port(), |r| {
            SimpleScore::new(
                r.position
                    .map_or(0, |p| i64::from(r.value) * (p.index + 1) as i64),
            )
        })
        .unwrap();
        let mut d =
            MixedStreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
        let result = d
            .run_solver(MixedSolverConfig {
                local_search: vec![MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 4,
                        ..Default::default()
                    },
                    joint_moves: true,
                }],
                ..Default::default()
            })
            .unwrap();
        d.assert_consistent().unwrap();
        result
    };
    assert_eq!(run(linear.clone()), run(indexed.clone()));
    let repair = |p: &ListProblem<i32>| {
        ruin_recreate_list_stream_search_from(
            p,
            || Ok(graph()),
            p.initial_assignment(),
            RuinRecreateConfig {
                max_attempts: 4,
                ..Default::default()
            },
        )
        .unwrap()
    };
    assert_eq!(repair(&linear), repair(&indexed));
}
