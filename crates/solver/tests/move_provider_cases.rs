// Adapted from pinned Community custom provider tests; Apache-2.0.
// Exact identities/substitutions are recorded in compat/provenance.json.
use timetuck::*;
#[test]
fn custom_scoring_rule_selects_the_greatest_value() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let mut provider = |p: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        p.variables()
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_pinned())
            .filter_map(|(i, v)| {
                v.domain().iter().max().map(|&x| Move::Change {
                    variable: i,
                    value: Some(x),
                })
            })
            .collect()
    };
    let actual: Vec<_> = provider.moves(&p, &a, 0).collect();
    assert_eq!(
        actual,
        vec![
            Move::Change {
                variable: 0,
                value: Some(1)
            },
            Move::Change {
                variable: 1,
                value: Some(1)
            }
        ]
    );
    for variable in 0..2 {
        assert!(!actual.contains(&Move::Change {
            variable,
            value: Some(0)
        }));
    }
}
struct Nearby;
impl MoveProvider<ListProblem<&'static str>, ListAssignment<&'static str>, ListMove<&'static str>>
    for Nearby
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<&'static str>,
        a: &'a ListAssignment<&'static str>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<&'static str>> + 'a>
    where
        ListMove<&'static str>: 'a,
    {
        Box::new(list_swap_moves(p, a).unwrap().filter(|m| match m {
            ListMove::Swap { left, right } => {
                left.entity != right.entity && left.index.abs_diff(right.index) <= 1
            }
            _ => false,
        }))
    }
}
#[test]
fn produces_nearby_swap_and_none_across_a_distance_of_two() {
    let p = ListProblem::new(
        vec!["a0", "a1", "a2", "b0", "b1", "b2"],
        vec![
            ListEntity::new(vec!["a0", "a1", "a2"]).with_value_range(vec!["a0", "a1", "a2", "b0"]),
            ListEntity::new(vec!["b0", "b1", "b2"]).with_value_range(vec!["b0", "b1", "b2", "a1"]),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let actual: Vec<_> = Nearby.moves(&p, &a, 0).collect();
    assert!(actual.contains(&ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 1
        },
        right: ListPosition {
            entity: 1,
            index: 0
        }
    }));
    assert!(!actual.contains(&ListMove::Swap {
        left: ListPosition {
            entity: 0,
            index: 0
        },
        right: ListPosition {
            entity: 1,
            index: 2
        }
    }));
}
