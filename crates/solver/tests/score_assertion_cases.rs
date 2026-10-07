// Adapted from pinned MultiConstraintAssertionTest, SingleConstraintAssertionTest,
// DuplicateConstraintAssertionTest and their fixtures. Apache-2.0; see compat/provenance.json.
use timetuck::*;

fn provider(
    first: Vec<(String, usize)>,
    second: Vec<(String, String)>,
) -> ScoreAssertion<HardSoftScore> {
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let standard = b.source::<String, usize>();
    let strings = b.source::<String, (String, String)>();
    // Codes, retained as tuple payloads, determine the mixed constraint's sign.
    let codes = b.source::<String, String>();
    b.score_weighted(
        "Penalize every standard entity",
        standard.port(),
        HardSoftScore::new(1, 0),
        |_| -1,
    )
    .unwrap();
    b.score_weighted(
        "Reward every standard entity",
        standard.port(),
        HardSoftScore::new(0, 2),
        |_| 1,
    )
    .unwrap();
    b.score_weighted(
        "Impact every standard entity",
        codes.port(),
        HardSoftScore::new(4, 0),
        |code| if code == "A" { 1 } else { -1 },
    )
    .unwrap();
    let joined = b
        .join_equal(
            strings.port(),
            strings.port(),
            |(_, value)| value.clone(),
            |(_, value)| value.clone(),
        )
        .unwrap();
    let pairs = b.filter(&joined, |(a, b)| a.0 < b.0).unwrap();
    b.score_weighted(
        "Different String Entity Have Different Values",
        &pairs,
        HardSoftScore::new(0, 3),
        |_| -1,
    )
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (code, value) in first {
        tx.apply(&standard, TupleChange::put(code.clone(), value))
            .unwrap();
        tx.apply(&codes, TupleChange::put(code.clone(), code))
            .unwrap();
    }
    for (code, value) in second {
        tx.apply(&strings, TupleChange::put(code.clone(), (code, value)))
            .unwrap();
    }
    tx.commit().unwrap();
    score_assertion(&g)
}
#[test]
fn checks_score() {
    // generateSolution(4 values, 5 entities): first 0/2/4, second 1/3 both value0.
    let a = provider(
        (0..5)
            .filter(|i| i % 2 == 0)
            .map(|i| (format!("Generated Entity {i}"), i % 4))
            .collect(),
        (0..5)
            .filter(|i| i % 2 == 1)
            .map(|i| {
                (
                    format!("Generated Entity {i}"),
                    format!("Generated Value {}", i / 4),
                )
            })
            .collect(),
    );
    a.check(
        ConstraintComparison::Exactly(HardSoftScore::new(-15, 3)),
        Some("There should be no penalties"),
    )
    .unwrap();
    let error = a
        .check(
            ConstraintComparison::Exactly(HardSoftScore::new(1, 1)),
            Some("There should be penalties"),
        )
        .unwrap_err();
    assert!(error.to_string().contains("There should be penalties"));
    assert_eq!(a.constraints().len(), 4);
}
#[test]
fn get_score_returns_typed_score_with_given_facts() {
    let a = provider(vec![("entity1".into(), 0)], vec![]);
    let score: &HardSoftScore = a.score();
    assert_eq!(score, &HardSoftScore::new(-5, 2));
}
#[test]
fn get_score_enables_relative_comparison_with_given_facts() {
    let a = provider(vec![("A".into(), 0)], vec![]);
    let b = provider(vec![("B1".into(), 0), ("B2".into(), 1)], vec![]);
    assert!(a.score() > b.score());
    a.check(ConstraintComparison::MoreThan(*b.score()), None)
        .unwrap();
}

// Native membership tracks ownership/index directly and requires no inverse field
// on value objects. This covers both source fixture representations.
type ListAssertions = (
    ScoreAssertion<SimpleScore>,
    ConstraintAssertion<SimpleScore, &'static str>,
    ConstraintAssertion<SimpleScore, &'static str>,
);
fn list_fixture(optional: bool) -> ListAssertions {
    let p = ListProblem::new(vec!["v1", "v2"], vec![ListEntity::new(vec!["v1"])]).unwrap();
    let p = if optional { p.allowing_unassigned() } else { p };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let assigned = b
        .filter(sources.values.port(), |v| v.position.is_some())
        .unwrap();
    let assigned = track_constraint(
        &mut b,
        "Penalize every unassigned value",
        &assigned,
        SimpleScore::new(1),
        |_| -1,
    )
    .unwrap();
    let all = track_constraint(
        &mut b,
        "Penalize every value",
        sources.values.port(),
        SimpleScore::new(1),
        |_| -1,
    )
    .unwrap();
    let director =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    (
        score_assertion(director.graph()),
        constraint_assertion(director.graph(), &assigned, |v, _| v.value).unwrap(),
        constraint_assertion(director.graph(), &all, |v, _| v.value).unwrap(),
    )
}
mod multi {
    use super::*;
    #[test]
    fn list_var_unassigned_while_allows_unassigned() {
        let (a, _, _) = list_fixture(true);
        a.check(
            ConstraintComparison::Exactly(SimpleScore::new(-3)),
            Some("There should be no penalties"),
        )
        .unwrap();
        let error = a
            .check(
                ConstraintComparison::Exactly(SimpleScore::new(-2)),
                Some("There should be penalties"),
            )
            .unwrap_err();
        assert!(error.to_string().contains("There should be penalties"));
    }
    #[test]
    fn list_var_unassigned_while_disallows_unassigned() {
        list_fixture(false)
            .0
            .check(
                ConstraintComparison::Exactly(SimpleScore::new(-3)),
                Some("There should be no penalties"),
            )
            .unwrap();
    }
    #[test]
    fn list_var_unassigned_while_disallows_unassigned_no_inverse_relation_shadow_var() {
        list_fixture(false)
            .0
            .check(
                ConstraintComparison::Exactly(SimpleScore::new(-3)),
                Some("There should be no penalties"),
            )
            .unwrap();
    }
}
mod single {
    use super::*;
    fn expect(
        a: &ConstraintAssertion<SimpleScore, &str>,
        n: usize,
        context: &str,
    ) -> Result<(), ConstraintAssertionError> {
        a.check(
            ConstraintExpectation::PenaltyCount(ConstraintComparison::Exactly(n)),
            Some(context),
        )
    }
    #[test]
    fn list_var_unassigned_while_allows_unassigned() {
        let (_, assigned, all) = list_fixture(true);
        for (a, n) in [(&assigned, 0), (&all, 1)] {
            assert!(
                expect(a, n, "There should be no penalties")
                    .unwrap_err()
                    .to_string()
                    .contains("There should be no penalties")
            );
        }
        expect(&assigned, 1, "There should be penalties").unwrap();
        expect(&all, 2, "There should be penalties").unwrap();
    }
    #[test]
    fn list_var_unassigned_while_disallows_unassigned() {
        let (_, assigned, all) = list_fixture(false);
        expect(&assigned, 1, "There should be penalties").unwrap();
        expect(&all, 2, "There should be penalties").unwrap();
    }
    #[test]
    fn list_var_unassigned_while_disallows_unassigned_no_inverse_relation_shadow_var() {
        let (_, assigned, all) = list_fixture(false);
        expect(&assigned, 1, "There should be penalties").unwrap();
        expect(&all, 2, "There should be penalties").unwrap();
    }
}
#[test]
fn throws_exception_on_duplicate_constraint_id() {
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let source = b.source::<usize, usize>();
    let name = "Penalize every standard entity";
    b.score_weighted(name, source.port(), HardSoftScore::new(1, 0), |_| -1)
        .unwrap();
    let error = match b.score_weighted(name, source.port(), HardSoftScore::new(1, 0), |_| -1) {
        Err(error) => error,
        Ok(_) => panic!("duplicate constraint accepted"),
    };
    assert!(error.to_string().contains(name));
}
