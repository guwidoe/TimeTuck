// Adapted from Timefold Community MoveDirectorTest and no_inconsistent_field
// fixtures, Apache-2.0. Exact test/helper attribution: compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
// Registry 0=b1, 1=b2. Both last one hour; b2 depends on b1.
fn finish_score(list: &[usize]) -> HardSoftScore {
    let graph = DependencyGraph::new([(0, vec![]), (1, vec![0])]).unwrap();
    let edges: Vec<_> = list.windows(2).map(|w| (w[1], w[0])).collect();
    let times = graph
        .evaluate(&edges, |&i, deps| {
            if !list.contains(&i) {
                return None;
            }
            let mut start = 0;
            for (_, dependency) in deps {
                start = start.max(dependency.value?);
            }
            Some(start + 60)
        })
        .unwrap();
    assert!(times.values().all(|v| !v.inconsistent));
    HardSoftScore::new(0, -times.values().filter_map(|v| v.value).sum::<i64>())
}
#[test]
fn restore_working_score_when_handling_structurally_flawed_solutions() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    let calls = Cell::new(0);
    let score = |list: &[usize]| {
        calls.set(calls.get() + 1);
        finish_score(list)
    };
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("finish early", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(list) = facts[0] else {
            panic!()
        };
        score(list)
    })
    .unwrap();
    macro_rules! check {
        ($d:ident) => {{
            let previous_score = $d.score();
            assert_eq!(previous_score, HardSoftScore::new(0, -180));
            let before = calls.get();
            let result = TemporaryMoves::new(&mut $d)
                .process_inspected(
                    &ListMove::Swap {
                        left: ListPosition {
                            entity: 0,
                            index: 0,
                        },
                        right: ListPosition {
                            entity: 0,
                            index: 1,
                        },
                    },
                    TemporaryMoveOptions::default(),
                    |_, _| -> Result<&str, ()> {
                        panic!("flawed candidate reached valid callback")
                    },
                    |preview| {
                        assert_eq!(preview.assignment().lists(), &[vec![1, 0]]);
                        assert_eq!(preview.inconsistent_values(), &[0, 1]);
                        Ok("flawed")
                    },
                )
                .unwrap();
            assert_eq!(result, "flawed");
            assert_eq!($d.assignment().lists(), &[vec![0, 1]]);
            assert_eq!($d.score(), previous_score);
            assert_eq!(calls.get(), before, "flawed candidate must not be scored");
            $d.assert_consistent().unwrap();
        }};
    }
    let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    check!(d);
    let mut b = StreamGraphBuilder::new();
    let sources = ListStreamSources::<usize>::new(&mut b);
    b.score("finish early", sources.entities.port(), |(_, list)| {
        score(list)
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    check!(d);
}
