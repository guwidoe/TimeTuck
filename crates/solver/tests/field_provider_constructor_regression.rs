//! Original cross-column, binding validation and real search controls.
use std::collections::BTreeSet;
use timetuck::*;
struct Model;
struct Task;
fn schema(optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Task>(vec![
        PlanningVariableDefinition::basic::<i32>("x", optional),
        PlanningVariableDefinition::basic::<i32>("y", false),
        PlanningVariableDefinition::list::<i32>("items", false),
    ])])
    .unwrap()
}
fn field<'s>(s: &'s PlanningSchema, name: &str, slots: Vec<usize>) -> SchemaVariableSlots<'s> {
    SchemaVariableSlots {
        variable: s
            .genuine_entity::<Task>()
            .unwrap()
            .genuine_variable(name)
            .unwrap(),
        slots,
    }
}
#[test]
fn constructor_validation_rejects_foreign_duplicate_list_and_misaligned_bindings() {
    let s = schema(false);
    let other = schema(false);
    for fields in [
        vec![field(&s, "x", vec![0]), field(&other, "y", vec![1])],
        vec![field(&s, "x", vec![0]), field(&s, "x", vec![1])],
        vec![field(&s, "items", vec![0])],
        vec![field(&s, "x", vec![0]), field(&s, "y", vec![1, 2])],
        vec![field(&s, "x", vec![0]), field(&s, "y", vec![0])],
        vec![field(&s, "x", vec![])],
    ] {
        assert!(
            EntitySwapProvider::from_planning_fields(
                &fields,
                EntitySwapOrder::Unordered(vec![0, 1]),
            )
            .is_err()
        );
        assert!(ScopedPillarMoveProvider::swap_fields(&fields).is_err());
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let provider = EntitySwapProvider::from_planning_field(
        &field(&s, "x", vec![5]),
        EntitySwapOrder::Unordered(vec![0]),
    )
    .unwrap();
    assert!(provider.select(&p, &p.initial_assignment(), 0).is_err());
}
#[test]
fn selected_columns_match_independent_successor_sets_for_all_binary_rows() {
    let s = schema(false);
    // Binding order differs from the canonical pillar ordering. Pair orders must
    // still identify these exact entity slots, rather than sorted rows.
    let slots = [6, 0, 4, 2];
    let x = field(&s, "x", slots.to_vec());
    let entity =
        EntitySwapProvider::from_planning_field(&x, EntitySwapOrder::Unordered(vec![0, 1, 2, 3]))
            .unwrap();
    let pillar = ScopedPillarMoveProvider::swap_field(&x).unwrap();
    for mask in 0..256 {
        let p = FiniteProblem::new(
            (0..8)
                .map(|i| {
                    Variable::new(vec![0, 1])
                        .with_initial((mask >> i) & 1)
                        .with_pinned(i % 2 == 1)
                })
                .collect(),
        )
        .unwrap();
        let a = p.initial_assignment();
        let mut expected = BTreeSet::new();
        for (i, &left) in slots.iter().enumerate() {
            for &right in &slots[i + 1..] {
                if a.values()[left] != a.values()[right] {
                    let mut row = a.values().to_vec();
                    row.swap(left, right);
                    expected.insert(row);
                }
            }
        }
        let candidates: Vec<_> = entity.select(&p, &a, 0).unwrap().collect();
        for c in &candidates {
            let mut trial = a.clone();
            let guard = p.apply(&mut trial, &c.movement).unwrap();
            assert_eq!(guard.values()[slots[c.left]], a.values()[slots[c.right]]);
            assert_eq!(guard.values()[slots[c.right]], a.values()[slots[c.left]]);
        }
        let successors: BTreeSet<_> = candidates
            .into_iter()
            .map(|c| {
                let mut trial = a.clone();
                p.apply(&mut trial, &c.movement).unwrap().commit();
                trial.values().to_vec()
            })
            .collect();
        assert_eq!(successors, expected);
        let expected_pillar = if slots.iter().all(|&i| a.values()[i] == a.values()[slots[0]]) {
            BTreeSet::new()
        } else {
            let mut row = a.values().to_vec();
            for &i in &slots {
                row[i] = row[i].map(|v| 1 - v);
            }
            BTreeSet::from([row])
        };
        let actual: BTreeSet<_> = pillar
            .select(&p, &a)
            .unwrap()
            .map(|m| {
                let mut trial = a.clone();
                p.apply(&mut trial, &m).unwrap().commit();
                for i in (1..8).step_by(2) {
                    assert_eq!(trial.values()[i], a.values()[i]);
                }
                trial.values().to_vec()
            })
            .collect();
        assert_eq!(actual, expected_pillar);
    }
}
#[test]
fn optional_change_constructor_emits_real_unassignment_and_respects_selected_pins() {
    let s = schema(true);
    let x = field(&s, "x", vec![0, 2]);
    let provider = ScopedPillarMoveProvider::subpillar_change_fields(
        &[x],
        PillarSelectionPolicy::subsets(1, 2).unwrap(),
        true,
    )
    .unwrap();
    for pinned in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .with_initial(0)
                .allowing_unassigned()
                .with_pinned(pinned),
            Variable::new(vec![9]).with_initial(9).with_pinned(true),
            Variable::new(vec![0, 1])
                .with_initial(0)
                .allowing_unassigned(),
        ])
        .unwrap();
        let a = p.initial_assignment();
        let mut saw_none = false;
        for movement in provider.select(&p, &a).unwrap() {
            let mut trial = a.clone();
            let guard = p.apply(&mut trial, &movement).unwrap();
            saw_none |= guard.values()[2].is_none();
            assert_eq!(guard.values()[1], Some(9));
            if pinned {
                assert_eq!(guard.values()[0], Some(0));
            }
        }
        assert!(saw_none);
    }
}
#[test]
fn field_constructed_pillars_improve_real_dense_and_streamed_search() {
    let s = schema(false);
    let p = FiniteProblem::new(
        (0..8)
            .map(|i| {
                if i % 2 == 1 {
                    Variable::new(vec![9]).with_initial(9).with_pinned(true)
                } else {
                    Variable::new(vec![0, 1]).with_initial(i32::from(i >= 4))
                }
            })
            .collect(),
    )
    .unwrap();
    for stream in [false, true] {
        let mut provider =
            ScopedPillarMoveProvider::swap_field(&field(&s, "x", vec![0, 2, 4, 6])).unwrap();
        let config = LocalSearchConfig {
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("weighted", source.port(), |(i, v)| {
                SimpleScore::new(if i % 2 == 0 {
                    (4 - i / 2) as i64 * i64::from(v.unwrap_or(0))
                } else {
                    0
                })
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(8);
            n.add_constraint("weighted", vec![0, 2, 4, 6], |v| {
                SimpleScore::new(
                    v.iter()
                        .enumerate()
                        .map(|(i, v)| (4 - i) as i64 * i64::from(v.copied().unwrap_or(0)))
                        .sum(),
                )
            })
            .unwrap();
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(7));
        assert_eq!(
            best.assignment.values(),
            &[
                Some(1),
                Some(9),
                Some(1),
                Some(9),
                Some(0),
                Some(9),
                Some(0),
                Some(9)
            ]
        );
    }
}
