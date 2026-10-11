// Adapted from Timefold Community NeighborhoodsTest and its domain fixtures.
// Apache-2.0; exact mappings and native boundaries: compat/provenance.json.
use std::{cell::RefCell, collections::BTreeSet, num::NonZeroU64};
use timetuck::*;

#[test]
fn change_move_based_local_search() {
    // Source generateSolution(2,5), then every entity is set to the second value.
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(1); 5]).unwrap();
    let mut n = ConstraintNetwork::new(5);
    n.add_constraint("distinct non-first values", (0..5).collect(), |values| {
        let distinct: BTreeSet<_> = values
            .iter()
            .map(|v| v.copied())
            .filter(|v| *v != Some(0))
            .collect();
        SimpleScore::new(-(distinct.len() as i64))
    })
    .unwrap();
    // Source uses Random(0); native fixed seed/order and explicit finite work
    // limits replace its unbounded run, without promising identical trajectories.
    let config = LocalSearchConfig {
        neighborhood: Neighborhood::Change,
        acceptance: Acceptance::LateAcceptance {
            history_size: 400,
            hill_climbing: false,
        },
        accepted_candidate_limit: NonZeroU64::new(1),
        best_score_limit: Some(SimpleScore::ZERO),
        move_order: MoveOrder::ShuffledMoves {
            seed: 0,
            window: 32.try_into().unwrap(),
        },
        max_steps: 1000,
        max_move_evaluations: Some(10_000),
        assert_scores: true,
        ..Default::default()
    };
    // Execute a genuine change provider through the local-search phase boundary.
    let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
        change_moves(p, a).unwrap().collect::<Vec<_>>()
    };
    let result =
        local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
            .unwrap();
    // Source asserts no exception. These stronger native quality/budget checks
    // prevent a finite limit from disguising failure to reach its score goal.
    assert_eq!(result.best.unwrap().score, SimpleScore::ZERO);
    assert!(result.completed_steps <= 1000);
}

#[test]
fn allows_null_values() {
    // generateSolution(2,3) initializes [null,1,0]; source edits yield [1,null,0].
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 3]).unwrap();
    let initial = Assignment::new(vec![Some(1), None, Some(0)]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let assigned = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
    let missing = b.filter(source.port(), |(_, v)| v.is_none()).unwrap();
    let pairs = b.join_equal(&assigned, &missing, |_| (), |_| ()).unwrap();
    let moves = b
        .map(&pairs, |((left, _), (right, _))| Move::Swap {
            left: *left,
            right: *right,
        })
        .unwrap();
    let dataset = b.dataset(&moves).unwrap();
    let director = StreamScoreDirector::new(&p, b.build(), source, initial).unwrap();
    let context = NeighborhoodTester::new(director, move |d| {
        Ok::<_, GraphError>(Box::new(
            d.graph().dataset_rows(&dataset)?.values().cloned(),
        ))
    });
    let move1 = Move::Swap { left: 0, right: 1 };
    context
        .produces_all_of([move1.clone(), Move::Swap { left: 2, right: 1 }])
        .unwrap();
    context.update(|d| d.apply(&move1).unwrap().commit());
    context
        .produces_all_of([
            Move::Swap { left: 2, right: 0 },
            Move::Swap { left: 1, right: 0 },
        ])
        .unwrap();
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Value {
    id: usize,
    position: i32,
    duration: i32,
}
#[derive(Clone, Copy)]
struct Entity {
    position: i32,
    start_time: i32,
}
#[test]
fn notify_when_value_index_changes_on_simple_model() {
    let entities = [Entity {
        position: 0,
        start_time: 0,
    }; 2];
    let values: Vec<_> = (0..3)
        .map(|id| Value {
            id,
            position: 0,
            duration: 0,
        })
        .collect();
    let p = ListProblem::new(
        values.clone(),
        vec![
            ListEntity::new(vec![values[0].clone()]),
            ListEntity::new(values[1..].to_vec()),
        ],
    )
    .unwrap();
    let updates = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = ListStreamSources::new(&mut b);
    // Native cascade supplies the source fixture's start/end rules and keeps the
    // full owner/index/previous projection even when all times stay zero.
    let timed = list_cascade_stream(
        &mut b,
        &source,
        &p,
        |c: ListCascadeStep<'_, Value, (i32, i32)>| {
            let start = if let Some(previous) = c.previous {
                c.previous_shadow?.1 + (c.value.position - previous.position).abs()
            } else {
                let entity = entities[c.position.entity];
                entity.start_time + (c.value.position - entity.position).abs()
            };
            Some((start, start + c.value.duration))
        },
    )
    .unwrap();
    let observed = b
        .map(&timed, |(row, shadow)| {
            updates.borrow_mut().push(row.value.id);
            (row.clone(), shadow.clone())
        })
        .unwrap();
    let dataset = b.dataset(&observed).unwrap();
    // Source forEach(Object) dummy penalty covers two entities and three values.
    b.score("dummy entities", source.entities.port(), |_| {
        SimpleScore::new(-1)
    })
    .unwrap();
    b.score("dummy values", source.values.port(), |_| {
        SimpleScore::new(-1)
    })
    .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(d.score(), SimpleScore::new(-5));
    // Source only requires atLeastOnce across initialization+move. Clearing here
    // additionally proves the actual relocation reaches all three value rows.
    updates.borrow_mut().clear();
    d.apply(&ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), SimpleScore::new(-5));
    for value in 0..3 {
        assert!(updates.borrow().contains(&value));
    }
    for (id, (row, shadow)) in d.graph().dataset_rows(&dataset).unwrap().iter() {
        assert_eq!(
            row.position,
            Some(ListPosition {
                entity: 1,
                index: *id
            })
        );
        assert_eq!(shadow.value, Some((0, 0)));
    }
    d.assert_consistent().unwrap();
}
