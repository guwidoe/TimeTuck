// Adapted from Community ConcurrentValuesShadowVariableTest, Apache-2.0.
// See compat/provenance.json for source fixtures and modifications.
#[path = "../examples/concurrent_support/mod.rs"]
mod support;
use support::{Schedule, Visit};
use timetuck::*;
fn starts(rows: &[Visit], ids: &[usize], time: i64) {
    for &i in ids {
        assert_eq!(rows[i].start, Some(time));
        assert_eq!(rows[i].finish, Some(time + 30));
        assert!(!rows[i].inconsistent);
    }
}
fn invalid(rows: &[Visit], ids: &[usize]) {
    for &i in ids {
        assert_eq!(rows[i].start, None);
        assert_eq!(rows[i].finish, None);
        assert!(rows[i].inconsistent);
    }
}
fn fixture() -> Schedule {
    Schedule::new(&[vec![0, 1], vec![2, 3, 4]], 6)
}
fn simple() {
    let s = Schedule::new(&[], 3);
    let (a, _) = s.project(&[vec![0, 1, 2]]);
    for i in 0..3 {
        assert_eq!(a[i].previous, i.checked_sub(1));
        assert_eq!(a[i].next, if i < 2 { Some(i + 1) } else { None });
        assert_eq!(a[i].index, Some(i));
        assert_eq!(a[i].cascading, Some(i as i64 * 1440));
        starts(&a, &[i], i as i64 * 60);
    }
    let (b, _) = s.project(&[vec![0, 2, 1]]);
    for (i, &v) in [0, 2, 1].iter().enumerate() {
        assert_eq!(b[v].previous, i.checked_sub(1).map(|j| [0, 2, 1][j]));
        assert_eq!(b[v].next, [0, 2, 1].get(i + 1).copied());
        starts(&b, &[v], i as i64 * 60);
    }
}
#[test]
fn simple_chain() {
    simple();
}
#[test]
fn solution_simple_chain() {
    simple();
}
#[test]
fn group_chain() {
    let s = fixture();
    let (a, calls) = s.project(&[vec![0, 3], vec![1, 4], vec![2, 5]]);
    assert_eq!(&calls[..5], &[1, 0, 1, 0, 0]);
    starts(&a, &[0, 1], 0);
    starts(&a, &[2, 3, 4], 60);
    starts(&a, &[5], 120);
    let (b, _) = s.project(&[vec![5, 0, 3], vec![1, 4], vec![2]]);
    starts(&b, &[0, 1], 60);
    starts(&b, &[2, 3, 4], 120);
    starts(&b, &[5], 0);
}
#[test]
fn group_chain_valid_to_invalid() {
    let s = fixture();
    let (a, _) = s.project(&[vec![0, 3], vec![1, 4], vec![2, 5]]);
    starts(&a, &[0, 1], 0);
    starts(&a, &[2, 3, 4], 60);
    starts(&a, &[5], 120);
    let (b, _) = s.project(&[vec![3, 0], vec![1, 4], vec![2, 5]]);
    invalid(&b, &[0, 1, 2, 3, 4, 5]);
    let (c, _) = s.project(&[vec![3, 0], vec![5, 1, 4], vec![2]]);
    invalid(&c, &[0, 1, 2, 3, 4]);
    starts(&c, &[5], 0);
}
#[test]
fn group_chain_invalid_to_valid() {
    let s = fixture();
    let (a, _) = s.project(&[vec![2, 0, 4], vec![1, 3], vec![5]]);
    invalid(&a, &[0, 1, 2, 3, 4]);
    starts(&a, &[5], 0);
    let (b, _) = s.project(&[vec![0], vec![1, 3], vec![2, 5]]);
    starts(&b, &[0, 1], 0);
    starts(&b, &[2, 3, 4], 60);
    starts(&b, &[5], 120);
}
fn solve_case(s: Schedule) {
    let p = ListProblem::new((0..6).collect(), vec![ListEntity::new(vec![]); 3]).unwrap();
    let result = solve_list_stream(
        &p,
        || {
            let (g, s, _) = s.graph(3);
            Ok((g, s))
        },
        SolverConfig {
            limits: SolverLimits {
                max_move_evaluations: Some(1000),
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                ..Default::default()
            }],
        },
    )
    .unwrap();
    assert!(result.complete);
    assert!(result.work.move_evaluations <= 1000);
    let score = result.score.unwrap();
    assert!(score.is_feasible());
    assert_eq!(score, s.score(result.assignment.lists()));
}
#[test]
fn solve_no_concurrent_values() {
    solve_case(Schedule::new(&[], 6));
}
#[test]
fn solve_with_concurrent_values() {
    solve_case(fixture());
}
