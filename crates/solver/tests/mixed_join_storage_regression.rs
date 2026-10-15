//! Original direct planning join storage contracts; no upstream method credit.
use std::{
    cell::Cell,
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn problem(count: usize) -> MixedProblem<i32, i32> {
    MixedProblem::new(
        FiniteProblem::new(
            (0..count)
                .map(|i| {
                    let variable = Variable::new(vec![0, 1, 3]).allowing_unassigned();
                    if i < 2 {
                        variable.with_initial(1)
                    } else {
                        variable
                    }
                })
                .collect(),
        )
        .unwrap(),
        ListProblem::new(
            vec![10, 20, 30],
            vec![
                ListEntity::new(vec![10]).with_pinned(true),
                ListEntity::new(vec![20, 30]).with_pinned_prefix(1),
            ],
        )
        .unwrap()
        .allowing_unassigned(),
    )
}
fn expected(assignment: &MixedAssignment<i32, i32>) -> SimpleScore {
    SimpleScore::new(
        assignment
            .lists()
            .lists()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                i64::from(assignment.basic().values()[i].unwrap_or(0))
                    * row.iter().map(|v| i64::from(*v)).sum::<i64>()
            })
            .sum(),
    )
}
fn change(value: i32) -> MixedMove<i32, i32> {
    MixedMove {
        basic: Move::Change {
            variable: 1,
            value: Some(value),
        },
        lists: ListMove::Composite(vec![]),
    }
}

#[test]
fn direct_join_nested_restoration_keeps_pins_partial_rows_and_saved_payloads() {
    let problem = problem(2);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut builder);
    let inspect = sources.clone();
    let joined = builder
        .join_equal(
            sources.basic.port(),
            sources.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    builder
        .score("joint", &joined, |((_, v), (_, row))| {
            SimpleScore::new(
                i64::from(v.unwrap_or(0)) * row.iter().map(|x| i64::from(*x)).sum::<i64>(),
            )
        })
        .unwrap();
    let original = problem.initial_assignment();
    let mut director =
        MixedStreamScoreDirector::new(&problem, builder.build(), sources, original.clone())
            .unwrap();
    let snapshot = director
        .graph()
        .source_rows(&inspect.lists.entities)
        .unwrap()
        .clone();
    {
        let mut outer = director.edit_session().unwrap();
        outer.apply(&change(3)).unwrap();
        assert_eq!(outer.director().score(), expected(outer.assignment()));
        {
            let mut inner = outer.nested().unwrap();
            inner
                .apply(&MixedMove {
                    basic: Move::Change {
                        variable: 1,
                        value: None,
                    },
                    lists: ListMove::Unassign {
                        entity: 1,
                        start: 1,
                        end: 2,
                    },
                })
                .unwrap();
            assert_eq!(inner.director().score(), SimpleScore::new(10));
            assert!(
                inner
                    .director()
                    .graph()
                    .source_rows(&inspect.lists.values)
                    .unwrap()[&2]
                    .position
                    .is_none()
            );
            inner.commit();
        }
        assert_eq!(outer.assignment().lists().lists()[1], vec![20]);
    }
    assert_eq!(director.assignment(), &original);
    assert_eq!(director.score(), expected(&original));
    assert_eq!(snapshot[&1].1, vec![20, 30]);
    assert!(
        director
            .apply(&MixedMove {
                basic: Move::Composite(vec![]),
                lists: ListMove::Unassign {
                    entity: 0,
                    start: 0,
                    end: 1
                }
            })
            .is_err()
    );
    director.apply(&change(3)).unwrap().commit();
    director.assert_consistent().unwrap();
    assert_eq!(director.score(), expected(director.assignment()));
}

#[test]
fn direct_large_mapped_and_nonplanning_inputs_keep_live_join_results_and_protected_ids() {
    for (count, mapped) in [(2, false), (257, false), (2, true)] {
        let problem = problem(count);
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let sources = MixedStreamSources::new(&mut builder);
        let inspect = sources.clone();
        let left = if mapped {
            builder.map(sources.basic.port(), |v| *v).unwrap()
        } else {
            sources.basic.port().clone()
        };
        let joined = builder
            .join_equal(
                &left,
                sources.lists.entities.port(),
                |(i, _)| *i,
                |(i, _)| *i,
            )
            .unwrap();
        builder
            .score("joint", &joined, |((_, v), (_, row))| {
                SimpleScore::new(
                    i64::from(v.unwrap_or(0)) * row.iter().map(|x| i64::from(*x)).sum::<i64>(),
                )
            })
            .unwrap();
        let facts = builder.source::<usize, (usize, i64)>();
        let with_facts = builder
            .join_equal(sources.basic.port(), facts.port(), |(i, _)| *i, |(i, _)| *i)
            .unwrap();
        builder
            .score("fact", &with_facts, |((_, v), (_, weight))| {
                SimpleScore::new(i64::from(v.unwrap_or(0)) * weight)
            })
            .unwrap();
        let mut director = MixedStreamScoreDirector::new(
            &problem,
            builder.build(),
            sources,
            problem.initial_assignment(),
        )
        .unwrap();
        {
            let mut edit = director.edit_facts();
            assert_eq!(
                edit.apply(&inspect.basic, TupleChange::put(count, (count, Some(3)))),
                Err(GraphError::ProtectedPlanningSource)
            );
        }
        {
            let mut edit = director.edit_facts();
            edit.apply(&facts, TupleChange::put(900, (1, 7))).unwrap();
            edit.commit().unwrap();
        }
        let before = director.score();
        {
            let trial = director.apply(&change(3)).unwrap();
            assert_eq!(
                trial.score(),
                expected(trial.assignment()) + SimpleScore::new(21)
            );
        }
        assert_eq!(director.score(), before);
        director.apply(&change(3)).unwrap().commit();
        assert_eq!(
            director.score(),
            expected(director.assignment()) + SimpleScore::new(21)
        );
        {
            let mut edit = director.edit_facts();
            edit.apply(&facts, TupleChange::put(901, (1, 11))).unwrap();
            assert_eq!(edit.score().unwrap(), SimpleScore::new(214));
        }
        assert_eq!(
            director.score(),
            expected(director.assignment()) + SimpleScore::new(21)
        );
        director.assert_consistent().unwrap();
    }
}

#[derive(Debug)]
struct Key {
    value: i32,
    armed: Rc<Cell<bool>>,
    clones: Rc<Cell<usize>>,
}
impl Clone for Key {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            armed: self.armed.clone(),
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        assert!(
            !self.armed.replace(false),
            "deliberate join key comparison panic"
        );
        self.value.cmp(&other.value)
    }
}

#[test]
fn custom_join_key_panic_rolls_back_and_retained_search_rebuilds_consistently() {
    let problem = problem(2);
    let armed = Rc::new(Cell::new(false));
    let clones = Rc::new(Cell::new(0));
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut builder);
    let key = |i: usize| Key {
        value: i as i32,
        armed: armed.clone(),
        clones: clones.clone(),
    };
    let joined = builder
        .join_equal(
            sources.basic.port(),
            sources.lists.entities.port(),
            |(i, _)| key(*i),
            |(i, _)| key(*i),
        )
        .unwrap();
    builder
        .score("joint", &joined, |((_, v), (_, row))| {
            SimpleScore::new(
                i64::from(v.unwrap_or(0)) * row.iter().map(|x| i64::from(*x)).sum::<i64>(),
            )
        })
        .unwrap();
    let original = problem.initial_assignment();
    let mut director =
        MixedStreamScoreDirector::new(&problem, builder.build(), sources, original.clone())
            .unwrap();
    armed.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = director.apply(&change(3));
        }))
        .is_err()
    );
    armed.set(false);
    assert_eq!(director.assignment(), &original);
    assert_eq!(director.score(), expected(&original));
    director.assert_consistent().unwrap();
    let result = director
        .run_local_search(MixedLocalSearchConfig {
            joint_moves: true,
            search: LocalSearchConfig {
                max_steps: 4,
                max_evaluations: 500,
                selection: MoveSelection::BestAccepted,
                ..Default::default()
            },
        })
        .unwrap();
    let best = result.best.unwrap();
    assert_eq!(best.score, expected(&best.assignment));
    assert_eq!(director.assignment(), &best.assignment);
    assert!(best.score >= expected(&original));
    assert!(clones.get() > 0);
    director.assert_consistent().unwrap();
}
