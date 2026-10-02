//! Original development adapter for the generated list-move protocol.
use std::io::{self, BufRead};
use timetuck::{ListAssignment, ListEntity, ListMove, ListPosition, ListProblem};
fn position(entity: i32, index: i32) -> ListPosition {
    ListPosition {
        entity: usize::try_from(entity).expect("nonnegative entity"),
        index: usize::try_from(index).expect("nonnegative index"),
    }
}
fn state(a: &ListAssignment<i32>, count: i32) -> String {
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
            a.element(&v).map_or_else(
                || "-".to_owned(),
                |s| {
                    format!(
                        "{}:{}:{}:{}",
                        s.position.entity,
                        s.position.index,
                        s.previous
                            .map_or_else(|| "-".to_owned(), ToString::to_string),
                        s.next.map_or_else(|| "-".to_owned(), ToString::to_string)
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
        let mut a = ListAssignment::new(lists);
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
        let movement = ListMove::Composite(moves);
        let temporary = {
            let candidate = p.apply(&mut a, &movement).expect("valid temporary move");
            state(&candidate, count)
        };
        let restored = state(&a, count);
        p.apply(&mut a, &movement)
            .expect("valid permanent move")
            .commit();
        println!(
            "{}\t{}\t{}\t{}",
            f[0],
            temporary,
            restored,
            state(&a, count)
        );
    }
}
