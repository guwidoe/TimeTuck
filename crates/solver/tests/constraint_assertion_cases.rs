// Adapted from pinned SingleConstraintAssertionTest and constraintverifier fixtures.
// Apache-2.0; see compat/provenance.json.
use timetuck::*;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind {
    First,
    Second,
    Value,
}
#[derive(Clone)]
struct Fact {
    kind: Kind,
    code: String,
    value: Option<String>,
}
#[derive(Clone, Copy)]
enum Mode {
    Penalty,
    Reward,
    Mixed,
    Justify,
    EmptyJustify,
}
#[derive(Clone, Copy)]
enum Given {
    Empty,
    Reward,
    Penalty,
    Generated,
}
#[derive(Clone, PartialEq, Eq)]
enum Justification {
    First(String),
    Second(String),
}
impl std::fmt::Debug for Justification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::First(id) => write!(f, "TestFirstJustification[id={id}]"),
            Self::Second(id) => write!(f, "TestSecondJustification[id={id}]"),
        }
    }
}
fn first(code: impl Into<String>, value: usize) -> Fact {
    Fact {
        kind: Kind::First,
        code: code.into(),
        value: Some(format!("Generated Value {value}")),
    }
}
fn generated(count: usize, extended: bool, whole_solution: bool) -> Vec<Fact> {
    let mut facts: Vec<_> = (0..count)
        .map(|i| {
            if extended && i % 2 == 1 {
                Fact {
                    kind: Kind::Second,
                    code: format!("Generated Entity {i}"),
                    value: Some(format!("Generated Value {}", i / 2)),
                }
            } else {
                first(format!("Generated Entity {i}"), i % 2)
            }
        })
        .collect();
    if whole_solution {
        facts.extend((0..2).map(|i| Fact {
            kind: Kind::Value,
            code: format!("Generated Value {i}"),
            value: None,
        }));
    }
    facts
}
fn snapshot(mode: Mode, facts: Vec<Fact>) -> ConstraintAssertion<HardSoftScore, Justification> {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, Fact>();
    let selected = b
        .filter(source.port(), move |f| {
            f.kind == Kind::First
                && f.value.is_some()
                && (!matches!(mode, Mode::EmptyJustify) || f.code == "Should not filter")
        })
        .unwrap();
    let (name, weight) = match mode {
        Mode::Penalty => ("Penalize every standard entity", HardSoftScore::new(1, 0)),
        Mode::Reward => ("Reward every standard entity", HardSoftScore::new(0, 2)),
        Mode::Mixed => ("Impact every standard entity", HardSoftScore::new(4, 0)),
        Mode::Justify => ("Justify with first justification", HardSoftScore::new(1, 0)),
        Mode::EmptyJustify => ("Justify without justifications", HardSoftScore::new(1, 0)),
    };
    let c = track_constraint(&mut b, name, &selected, weight, move |f| match mode {
        Mode::Reward => 1,
        Mode::Mixed if f.code == "A" => 1,
        _ => -1,
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for (id, fact) in facts.into_iter().enumerate() {
        tx.apply(&source, TupleChange::put(id, fact)).unwrap();
    }
    tx.commit().unwrap();
    constraint_assertion(&graph, &c, |fact, _| {
        Justification::First(fact.code.clone())
    })
    .unwrap()
}
fn check_result(result: Result<(), ConstraintAssertionError>, pass: bool, messages: &[&str]) {
    assert_eq!(result.is_ok(), pass, "{result:?}");
    if !pass {
        let text = result.unwrap_err().to_string();
        for message in messages {
            assert!(text.contains(message), "missing {message:?} in {text}");
        }
    }
}
fn scalar_case(
    mode: Mode,
    given: Given,
    expected: ConstraintExpectation,
    context: Option<&str>,
    pass: bool,
    messages: &[&str],
) {
    let facts = match given {
        Given::Empty => vec![],
        Given::Reward => vec![first("A", 0)],
        Given::Penalty => vec![first("B", 0)],
        Given::Generated => generated(3, false, false),
    };
    check_result(
        snapshot(mode, facts).check(expected, context),
        pass,
        messages,
    );
}
#[path = "support/constraint_assertion_matrix.rs"]
mod matrix;

#[test]
fn get_impact_enables_relative_comparison() {
    let a = snapshot(Mode::Penalty, vec![first("A", 0)]).impact();
    let b = snapshot(Mode::Penalty, generated(3, false, false)).impact();
    assert!(b.numeric_cmp(&a).is_gt());
}
#[test]
fn get_score_enables_relative_comparison_for_rewards() {
    let a = snapshot(Mode::Reward, vec![first("A", 0)]);
    let b = snapshot(Mode::Reward, generated(3, false, false));
    assert_eq!(a.score(), &HardSoftScore::new(0, 2));
    assert_eq!(b.score(), &HardSoftScore::new(0, 6));
    assert!(b.score() > a.score());
}
#[test]
fn get_score_compares_given_solutions() {
    let small = snapshot(Mode::Penalty, generated(2, true, true));
    let large = snapshot(Mode::Penalty, generated(4, true, true));
    assert!(small.score() > large.score());
}
#[test]
fn unique_pair_should_work_on_string_planning_id() {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<String, Fact>();
    let pairs = b
        .join_equal(
            source.port(),
            source.port(),
            |f| f.value.clone(),
            |f| f.value.clone(),
        )
        .unwrap();
    let unique = b.filter(&pairs, |(a, b)| a.code < b.code).unwrap();
    let c = track_constraint(
        &mut b,
        "Different String Entity Have Different Values",
        &unique,
        HardSoftScore::new(0, 3),
        |_| -1,
    )
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for id in ["A", "B"] {
        tx.apply(
            &source,
            TupleChange::put(
                id.to_owned(),
                Fact {
                    kind: Kind::Second,
                    code: id.to_owned(),
                    value: Some("1".into()),
                },
            ),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let a = constraint_assertion(&graph, &c, |_, _| ()).unwrap();
    a.check(
        ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(1)),
        Some("There should be penalties"),
    )
    .unwrap();
    check_result(
        a.check(
            ConstraintExpectation::RewardCount(ConstraintComparison::Exactly(1)),
            Some("There should be rewards"),
        ),
        false,
        &["There should be rewards", "Expected reward"],
    );
}
fn j(id: &str) -> Justification {
    Justification::First(id.into())
}
fn actual_justifications() -> Vec<Justification> {
    (0..3)
        .map(|i| j(&format!("Generated Entity {i}")))
        .collect()
}
fn justification_check(
    whole_solution: bool,
    empty: bool,
    expected: &[Justification],
    exact: bool,
    context: Option<&str>,
    pass: bool,
    messages: &[&str],
) {
    let mode = if empty {
        Mode::EmptyJustify
    } else {
        Mode::Justify
    };
    let a = snapshot(mode, generated(3, false, whole_solution));
    check_result(
        a.check_justifications(
            expected,
            if exact {
                JustificationMatch::Exact
            } else {
                JustificationMatch::Contains
            },
            context,
        ),
        pass,
        messages,
    );
}
fn diagnostic_messages(custom: bool, exact: bool) -> Vec<&'static str> {
    let mut messages = vec![
        if custom {
            "Custom Message"
        } else {
            "Broken expectation"
        },
        "Justification: Justify with first justification",
        "Expected",
        "TestFirstJustification[id=2]",
        "Actual",
        "TestFirstJustification[id=Generated Entity 0]",
        "TestFirstJustification[id=Generated Entity 1]",
        "TestFirstJustification[id=Generated Entity 2]",
        "Expected but not found:",
    ];
    if exact {
        messages.push("Unexpected but found:");
    }
    messages
}
#[test]
fn justifies() {
    for whole in [false, true] {
        justification_check(
            whole,
            false,
            &actual_justifications()[..2],
            false,
            None,
            true,
            &[],
        );
        justification_check(
            whole,
            false,
            &[j("2")],
            false,
            None,
            false,
            &diagnostic_messages(false, false),
        );
    }
    let mut messages = diagnostic_messages(false, false);
    messages.push("TestFirstJustification[id=Generated Entity 0]");
    justification_check(
        false,
        false,
        &[j("Generated Entity 0"), j("2")],
        false,
        None,
        false,
        &messages,
    );
    messages.push("TestSecondJustification[id=1]");
    justification_check(
        false,
        false,
        &[
            j("Generated Entity 0"),
            j("2"),
            Justification::Second("1".into()),
        ],
        false,
        None,
        false,
        &messages,
    );
}
#[test]
fn justifies_with_custom_message() {
    for whole in [false, true] {
        justification_check(
            whole,
            false,
            &[j("2")],
            false,
            Some("Custom Message"),
            false,
            &diagnostic_messages(true, false),
        );
    }
}
fn empty_justifications(exact: bool) {
    justification_check(false, true, &[], exact, None, true, &[]);
    justification_check(
        false,
        false,
        &[],
        exact,
        None,
        false,
        &[
            "Broken expectation",
            "Expected",
            "No Justification",
            "Actual",
            "TestFirstJustification[id=Generated Entity 0]",
            "TestFirstJustification[id=Generated Entity 1]",
            "TestFirstJustification[id=Generated Entity 2]",
            "Unexpected but found:",
        ],
    );
    justification_check(
        false,
        true,
        &[j("1")],
        exact,
        None,
        false,
        &[
            "Broken expectation",
            "Expected",
            "TestFirstJustification[id=1]",
            "Actual",
            "No Justification",
            "Expected but not found:",
        ],
    );
}
#[test]
fn justifies_empty_matches() {
    empty_justifications(false);
}
#[test]
fn justifies_exactly() {
    for whole in [false, true] {
        justification_check(
            whole,
            false,
            &actual_justifications(),
            true,
            None,
            true,
            &[],
        );
    }
    justification_check(
        false,
        false,
        &[j("2")],
        true,
        None,
        false,
        &diagnostic_messages(false, true),
    );
}
#[test]
fn justifies_exactly_with_custom_message() {
    for whole in [false, true] {
        justification_check(
            whole,
            false,
            &[j("2")],
            true,
            Some("Custom Message"),
            false,
            &diagnostic_messages(true, true),
        );
    }
}
#[test]
fn justifies_exactly_empty_matches() {
    empty_justifications(true);
}
