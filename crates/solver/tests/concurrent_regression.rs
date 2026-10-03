//! Original shared-schedule transaction and partial-assignment regressions.
#[path = "../examples/concurrent_support/mod.rs"]
mod support;
use support::Schedule;
use timetuck::*;
#[test]
fn cyclic_candidate_restores_cached_times_neighbors_and_score() {
    let s = Schedule::new(&[vec![0, 2]], 4);
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![ListEntity::new(vec![0, 1]), ListEntity::new(vec![2, 3])],
    )
    .unwrap();
    let (g, sources, c) = s.graph(2);
    let mut d = ListStreamScoreDirector::new(&p, g, sources, p.initial_assignment()).unwrap();
    let before = d.graph().constraint_matches(&c).unwrap().clone();
    assert_eq!(d.score(), HardSoftScore::new(0, -240));
    let m = ListMove::Change {
        source: ListPosition {
            entity: 1,
            index: 0,
        },
        destination: ListPosition {
            entity: 0,
            index: 2,
        },
    };
    {
        let t = d.apply(&m).unwrap();
        assert_eq!(t.score(), HardSoftScore::new(-3, -30));
        let rows = &t.graph().constraint_matches(&c).unwrap()[&()].0;
        assert_eq!(
            rows.iter().map(|v| v.inconsistent).collect::<Vec<_>>(),
            vec![true, true, true, false]
        );
        assert_eq!(rows[0].ready, Some(0));
        assert_eq!(rows[0].start, None);
        assert_eq!(rows[3].start, Some(0));
    }
    assert_eq!(d.graph().constraint_matches(&c).unwrap(), &before);
    assert_eq!(d.score(), HardSoftScore::new(0, -240));
    d.apply(&m).unwrap().commit();
    d.assert_consistent().unwrap();
    d.apply(&ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 2,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_eq!(d.graph().constraint_matches(&c).unwrap(), &before);
    d.assert_consistent().unwrap();
}
#[test]
fn all_small_partial_orders_match_independent_closure_scoring() {
    fn permute(values: &mut [usize], at: usize, out: &mut Vec<Vec<usize>>) {
        if at == values.len() {
            out.push(values.to_vec());
            return;
        }
        for i in at..values.len() {
            values.swap(i, at);
            permute(values, at + 1, out);
            values.swap(i, at);
        }
    }
    let s = Schedule::new(&[vec![0, 2]], 4);
    let mut orders = vec![];
    permute(&mut [0, 1, 2, 3], 0, &mut orders);
    for order in orders {
        for code in 0..81 {
            let mut owners = code;
            let mut lists = vec![vec![], vec![]];
            for &v in &order {
                let e = owners % 3;
                owners /= 3;
                if e < 2 {
                    lists[e].push(v);
                }
            }
            assert_eq!(s.score(&lists), s.reference_score(&lists), "{lists:?}");
            let (rows, _) = s.project(&lists);
            assert_eq!(rows[0].start, rows[2].start);
            assert_eq!(rows[0].finish, rows[2].finish);
        }
    }
    let (rows, _) = s.project(&[vec![1, 2], vec![]]);
    assert_eq!(rows[0].owner, None);
    assert_eq!(rows[0].ready, None);
    assert_eq!(rows[0].start, Some(60));
    assert_eq!(rows[0].finish, Some(90));
}
