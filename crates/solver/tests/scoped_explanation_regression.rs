//! Original explanation regressions; no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn ds(text: &str) -> SimpleDecimalScore {
    text.parse().unwrap()
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Basic(usize),
    Entity(usize),
    Value(usize),
}

#[test]
fn basic_explanations_preserve_scopes_structure_and_cached_weighted_scores() {
    let calls = Cell::new(0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![3, 7]).with_initial(3),
        Variable::new(vec![1]),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_weighted_constraint("hard", vec![0, 0], HardSoftScore::new(-1, 0), |values| {
        calls.set(calls.get() + 1);
        values.iter().filter_map(|v| v.copied()).sum()
    })
    .unwrap();
    n.add_constraint("soft", vec![0], |_| {
        calls.set(calls.get() + 1);
        HardSoftScore::new(0, 4)
    })
    .unwrap();
    n.add_constraint("constant", vec![], |_| HardSoftScore::ZERO)
        .unwrap();
    n.add_weighted_constraint("disabled", vec![0], HardSoftScore::ZERO, |_| {
        panic!("disabled score")
    })
    .unwrap();
    let mut director = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let snapshot = |director: &ScoreDirector<'_, '_, i64, HardSoftScore>| {
        director.explain_default(|name, scope, values| {
            assert_ne!(name, "disabled");
            assert_eq!(scope.len(), values.len());
            if name == "hard" {
                assert_eq!(values[0], values[1]);
            }
            scope.iter().copied().map(Fact::Basic).collect()
        })
    };
    let calls_before = calls.get();
    let before = snapshot(&director);
    assert_eq!(calls.get(), calls_before);
    assert_eq!(before.score, HardSoftScore::new(-6, 4).with_structural(-1));
    assert_eq!(before.matches.score, HardSoftScore::new(-6, 4));
    assert_eq!(
        before.matches.constraints["hard"].matches[&0]
            .justification
            .facts,
        [Fact::Basic(0), Fact::Basic(0)]
    );
    assert_eq!(
        before.matches.indictments[&Fact::Basic(0)].score,
        HardSoftScore::new(-6, 4)
    );
    assert_eq!(before.matches.indictments[&Fact::Basic(0)].matches.len(), 2);
    assert_eq!(before.matches.constraints["constant"].matches.len(), 1);
    assert!(
        before.matches.constraints["constant"]
            .indictments
            .is_empty()
    );
    assert!(before.matches.constraints["disabled"].matches.is_empty());
    let movement = Move::Change {
        variable: 0,
        value: Some(7),
    };
    drop(director.apply(&movement).unwrap());
    assert_eq!(snapshot(&director), before);
    director.apply(&movement).unwrap().commit();
    let after = snapshot(&director);
    let diff = after.matches.diff(&before.matches);
    assert_eq!(diff.score_delta, HardSoftScore::new(-8, 0));
    assert_eq!(diff.constraints.len(), 1);
    assert_eq!(
        diff.indictments[&Fact::Basic(0)].score_delta,
        HardSoftScore::new(-8, 0)
    );
    assert_eq!(
        before.matches.constraints["hard"].score,
        HardSoftScore::new(-6, 0)
    );
    director
        .apply(&Move::Change {
            variable: 1,
            value: Some(1),
        })
        .unwrap()
        .commit();
    let initialized = snapshot(&director);
    assert_eq!(initialized.matches, after.matches);
    assert_eq!(initialized.score.structural(), 0);
    assert_eq!(after.score.structural(), -1);
    let custom = director
        .explain(|name, _, _, score| (format!("{name}: {score:?}"), vec![Fact::Entity(99)]));
    assert_eq!(
        custom.matches.indictments[&Fact::Entity(99)].score,
        HardSoftScore::new(-14, 4)
    );
    assert_eq!(
        custom.matches.constraints["constant"].matches[&0].justification,
        "constant: HardSoftScore { structural: 0, levels: [0, 0] }"
    );
}

#[test]
fn list_explanations_capture_owned_shadows_unassigned_facts_and_decimal_precision() {
    let calls = Cell::new(0);
    let p = ListProblem::new(
        vec![7, 9, 11],
        vec![ListEntity::new(vec![7, 9]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 3);
    let scope = vec![
        ListScope::Entity(0),
        ListScope::Value(0),
        ListScope::Value(0),
        ListScope::Value(2),
    ];
    n.add_decimal_weighted_constraint("load", scope, ds("0.1"), |facts| {
        calls.set(calls.get() + 1);
        let ListFact::Entity(values) = facts[0] else {
            unreachable!()
        };
        Decimal::new(values.len().into(), 2)
    })
    .unwrap();
    let mut director = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let snapshot = |director: &ListScoreDirector<'_, '_, i32, SimpleDecimalScore>| {
        director.explain(|_, scopes, facts, score| {
            let ListFact::Value { value, element } = &facts[1] else {
                unreachable!()
            };
            assert_eq!(**value, 7);
            let ListFact::Value {
                value: missing,
                element: None,
            } = &facts[3]
            else {
                unreachable!()
            };
            assert_eq!(**missing, 11);
            (
                (score.clone(), element.as_ref().map(|e| e.position)),
                scopes
                    .iter()
                    .map(|s| match s {
                        ListScope::Entity(i) => Fact::Entity(*i),
                        ListScope::Value(i) => Fact::Value(*i),
                    })
                    .collect(),
            )
        })
    };
    let calls_before = calls.get();
    let before = snapshot(&director);
    assert_eq!(calls.get(), calls_before);
    assert_eq!(before.score, ds("0.002").with_structural(-1));
    assert_eq!(
        before.matches.constraints["load"].matches[&0].justification,
        (
            ds("0.002"),
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        )
    );
    assert_eq!(
        before.matches.indictments[&Fact::Value(0)].score,
        ds("0.002")
    );
    assert_eq!(before.matches.indictments[&Fact::Value(0)].matches.len(), 1);
    let movement = ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    drop(director.apply(&movement).unwrap());
    assert_eq!(snapshot(&director), before);
    director.apply(&movement).unwrap().commit();
    let after = snapshot(&director);
    assert_eq!(
        after.matches.constraints["load"].matches[&0].justification,
        (
            ds("0.001"),
            Some(ListPosition {
                entity: 1,
                index: 0
            })
        )
    );
    let diff = after.matches.diff(&before.matches);
    assert_eq!(diff.score_delta, ds("-0.001"));
    assert_eq!(diff.indictments.len(), 3);
    let defaults =
        director.explain_default(|_, scope, _| scope.iter().map(|s| format!("{s:?}")).collect());
    assert_eq!(
        defaults.matches.constraints["load"].matches[&0]
            .justification
            .facts
            .len(),
        4
    );
    director.assert_consistent().unwrap();
}

#[test]
fn mixed_explanations_follow_joint_commits_and_keep_basic_list_identities_distinct() {
    let calls = Cell::new(0);
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_weighted_constraint(
        "joint",
        vec![
            MixedScope::Basic(0),
            MixedScope::ListEntity(0),
            MixedScope::ListValue(0),
        ],
        SimpleScore::new(-2),
        |facts| {
            calls.set(calls.get() + 1);
            let (MixedFact::Basic(v), MixedFact::ListEntity(list)) = (&facts[0], &facts[1]) else {
                unreachable!()
            };
            v.copied().unwrap_or(0) + list.len() as i64
        },
    )
    .unwrap();
    let mut director = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    let snapshot = |director: &MixedScoreDirector<'_, '_, i64, i32, SimpleScore>| {
        director.explain_default(|_, scopes, facts| {
            assert_eq!(scopes.len(), facts.len());
            scopes
                .iter()
                .map(|s| match s {
                    MixedScope::Basic(i) => Fact::Basic(*i),
                    MixedScope::ListEntity(i) => Fact::Entity(*i),
                    MixedScope::ListValue(i) => Fact::Value(*i),
                })
                .collect()
        })
    };
    let before = snapshot(&director);
    assert_eq!(calls.get(), 1);
    assert_eq!(before.matches.indictments.len(), 3);
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2),
        },
        lists: ListMove::Change {
            source: ListPosition {
                entity: 0,
                index: 0,
            },
            destination: ListPosition {
                entity: 1,
                index: 0,
            },
        },
    };
    drop(director.apply(&movement).unwrap());
    assert_eq!(snapshot(&director), before);
    director.apply(&movement).unwrap().commit();
    let before_calls = calls.get();
    let custom = director.explain(|_, _, facts, _| {
        let (
            MixedFact::Basic(Some(v)),
            MixedFact::ListEntity(list),
            MixedFact::ListValue {
                element: Some(element),
                ..
            },
        ) = (&facts[0], &facts[1], &facts[2])
        else {
            unreachable!()
        };
        ((**v, list.to_vec(), element.position), vec![Fact::Value(0)])
    });
    assert_eq!(calls.get(), before_calls);
    assert_eq!(
        custom.matches.constraints["joint"].matches[&0].justification,
        (
            2,
            vec![],
            ListPosition {
                entity: 1,
                index: 0
            }
        )
    );
    assert_eq!(snapshot(&director).score, before.score); // Equal score, different solution.
    director.assert_consistent().unwrap();
}

#[test]
fn explanation_panics_and_empty_networks_leave_all_directors_usable() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    macro_rules! check {
        ($p:expr,$n:expr,$director:ident) => {{
            let p = $p;
            let mut n = $n;
            let director = $director::new(&p, &n, p.initial_assignment()).unwrap();
            let empty = director.explain_default(|_, _, _| Vec::<usize>::new());
            assert!(empty.matches.constraints.is_empty());
            assert_eq!(empty.score, director.score());
            drop(director);
            n.add_constraint("zero", vec![], |_| SimpleScore::ZERO)
                .unwrap();
            n.add_weighted_constraint("disabled", vec![], SimpleScore::ZERO, |_| {
                panic!("disabled scoring")
            })
            .unwrap();
            let director = $director::new(&p, &n, p.initial_assignment()).unwrap();
            let before = director.explain_default(|name, _, _| {
                assert_ne!(name, "disabled");
                vec![1]
            });
            assert!(before.matches.constraints["disabled"].matches.is_empty());
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _: ScopedScoreExplanation<_, (), usize> =
                        director.explain(|_, _, _, _| panic!("presentation"));
                }))
                .is_err()
            );
            assert_eq!(director.explain_default(|_, _, _| vec![1]), before);
            director.assert_consistent().unwrap();
        }};
    }
    check!(
        FiniteProblem::<i32>::new(vec![]).unwrap(),
        ConstraintNetwork::<i32, SimpleScore>::new(0),
        ScoreDirector
    );
    check!(
        ListProblem::<i32>::new(vec![], vec![]).unwrap(),
        ListConstraintNetwork::<i32, SimpleScore>::new(0, 0),
        ListScoreDirector
    );
    check!(
        MixedProblem::new(
            FiniteProblem::<i32>::new(vec![]).unwrap(),
            ListProblem::<i32>::new(vec![], vec![]).unwrap()
        ),
        MixedConstraintNetwork::<i32, i32, SimpleScore>::new(0, 0, 0),
        MixedScoreDirector
    );
}
