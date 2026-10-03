//! Original development adapter for the generated list-move protocol.
use std::io::{self, BufRead};
use timetuck::*;
fn position(entity: i32, index: i32) -> ListPosition {
    ListPosition {
        entity: usize::try_from(entity).expect("nonnegative entity"),
        index: usize::try_from(index).expect("nonnegative index"),
    }
}
fn state(
    a: &ListAssignment<i32>,
    graph: &StreamGraph<'_, SimpleScore>,
    sources: &ListStreamSources<i32>,
    count: i32,
) -> String {
    assert_eq!(
        graph
            .source_rows(&sources.entities)
            .unwrap()
            .values()
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>(),
        a.lists()
    );
    let views = graph.source_rows(&sources.values).unwrap();
    assert_eq!(views.len(), count as usize);
    let expected_score = a
        .lists()
        .iter()
        .enumerate()
        .flat_map(|(e, list)| {
            list.iter()
                .enumerate()
                .map(move |(i, v)| (*v as i64 + 1) * (e as i64 + 1) + i as i64)
        })
        .sum::<i64>();
    assert_eq!(graph.score(), SimpleScore::new(expected_score));
    let lists = a
        .lists()
        .iter()
        .map(|l| {
            if l.is_empty() {
                "-".to_owned()
            } else {
                l.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            }
        })
        .collect::<Vec<_>>()
        .join(";");
    let shadows = (0..count)
        .map(|v| {
            let row = &views[&(v as usize)];
            assert_eq!(row.value, v);
            assert_eq!(row.range_index, v as usize);
            row.position.map_or_else(
                || "-".to_owned(),
                |p| {
                    format!(
                        "{}:{}:{}:{}",
                        p.entity,
                        p.index,
                        row.previous
                            .map_or_else(|| "-".to_owned(), |v| v.to_string()),
                        row.next.map_or_else(|| "-".to_owned(), |v| v.to_string())
                    )
                },
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{lists}/{shadows}")
}
fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.expect("input");
        let f: Vec<_> = line.split('\t').collect();
        assert_eq!(f.len(), 4);
        let count: i32 = f[1].parse().expect("count");
        assert!(count >= 0);
        let lists = f[2]
            .split(';')
            .map(|l| {
                if l == "-" {
                    vec![]
                } else {
                    l.split(',').map(|v| v.parse().expect("value")).collect()
                }
            })
            .collect();
        let a = ListAssignment::new(lists);
        let p = ListProblem::new(
            (0..count).collect(),
            a.lists().iter().cloned().map(ListEntity::new).collect(),
        )
        .expect("valid model")
        .allowing_unassigned();
        let mut moves = vec![];
        if !f[3].is_empty() {
            for op in f[3].split(';') {
                let parts: Vec<_> = op.split(',').collect();
                let v: Vec<i32> = parts[1..]
                    .iter()
                    .map(|s| s.parse().expect("move integer"))
                    .collect();
                moves.push(match (parts[0], v.as_slice()) {
                    ("a", [value, e, i]) => ListMove::Assign {
                        value: *value,
                        destination: position(*e, *i),
                    },
                    ("c", [e, i, d, j]) => ListMove::Change {
                        source: position(*e, *i),
                        destination: position(*d, *j),
                    },
                    ("r", [e, start, end, d, j, reverse]) if matches!(reverse, 0 | 1) => {
                        ListMove::SubListChange {
                            entity: usize::try_from(*e).unwrap(),
                            start: usize::try_from(*start).unwrap(),
                            end: usize::try_from(*end).unwrap(),
                            destination: position(*d, *j),
                            reverse: *reverse == 1,
                        }
                    }
                    ("x", [e, start, end, d, other_start, other_end, reverse])
                        if matches!(reverse, 0 | 1) =>
                    {
                        ListMove::SubListSwap {
                            left: ListRange {
                                entity: usize::try_from(*e).unwrap(),
                                start: usize::try_from(*start).unwrap(),
                                end: usize::try_from(*end).unwrap(),
                            },
                            right: ListRange {
                                entity: usize::try_from(*d).unwrap(),
                                start: usize::try_from(*other_start).unwrap(),
                                end: usize::try_from(*other_end).unwrap(),
                            },
                            reverse: *reverse == 1,
                        }
                    }
                    ("s", [e, i, d, j]) => ListMove::Swap {
                        left: position(*e, *i),
                        right: position(*d, *j),
                    },
                    ("u", [e, start, end]) => ListMove::Unassign {
                        entity: usize::try_from(*e).unwrap(),
                        start: usize::try_from(*start).unwrap(),
                        end: usize::try_from(*end).unwrap(),
                    },
                    _ => panic!("invalid move"),
                });
            }
        }
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut builder);
        let inspect = sources.clone();
        builder
            .score("positions", sources.values.port(), |r| {
                SimpleScore::new(r.position.map_or(0, |p| {
                    (r.value as i64 + 1) * (p.entity as i64 + 1) + p.index as i64
                }))
            })
            .unwrap();
        let mut director = ListStreamScoreDirector::new(&p, builder.build(), sources, a).unwrap();
        let movement = ListMove::Composite(moves);
        let temporary = {
            let candidate = director.apply(&movement).expect("valid temporary move");
            state(candidate.assignment(), candidate.graph(), &inspect, count)
        };
        let restored = state(director.assignment(), director.graph(), &inspect, count);
        director
            .apply(&movement)
            .expect("valid permanent move")
            .commit();
        director.assert_consistent().unwrap();
        println!(
            "{}\t{}\t{}\t{}",
            f[0],
            temporary,
            restored,
            state(director.assignment(), director.graph(), &inspect, count)
        );
    }
}
