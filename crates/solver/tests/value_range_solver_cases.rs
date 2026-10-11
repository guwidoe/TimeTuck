// Adapted from pinned Community AnonymousValueRangeFactoryTest,
// ValueRangeFactoryTest, their fixtures and PlannerTestUtils (Apache-2.0).
// See compat/provenance.json. Explicit typed native bindings replace annotation
// discovery/Java assignability; no inferred-provider or JVM API parity is claimed.
use timetuck::*;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Value {
    Integer(i32),
    Long(i64),
    BigInteger(BigInt),
    BigDecimal(Decimal),
    Date(CivilDate),
    Time(CivilTime),
    DateTime(CivilDateTime),
    Year(CalendarYear),
}
#[derive(Clone, Copy, Debug)]
enum ProviderForm {
    Range,
    Array,
    List,
}
fn materialize<R: ValueRange>(range: R, wrap: impl Fn(R::Value) -> Value) -> Vec<Value> {
    range
        .materialize(32)
        .unwrap()
        .into_iter()
        .map(wrap)
        .collect()
}
fn decimal(s: &str) -> Decimal {
    s.parse().unwrap()
}
fn numeric_ranges() -> Vec<Vec<Value>> {
    vec![
        materialize(IntegerValueRange::new(0, 3, 1).unwrap(), |v| {
            Value::Integer(i32::try_from(v).unwrap())
        }),
        materialize(IntegerValueRange::new(1000, 1003, 1).unwrap(), Value::Long),
        materialize(
            BigIntegerValueRange::new(1_000_000.into(), 1_000_003.into()).unwrap(),
            Value::BigInteger,
        ),
        materialize(
            DecimalValueRange::new(decimal("0.00"), decimal("0.03")).unwrap(),
            Value::BigDecimal,
        ),
    ]
}
fn anonymous(form: ProviderForm) -> Vec<Vec<Value>> {
    let (typed, number_provider) = match form {
        ProviderForm::Range => (numeric_ranges(), vec![]),
        ProviderForm::Array => (
            vec![
                materialize(EnumeratedValueRange::new([0i32, 1]), Value::Integer),
                materialize(EnumeratedValueRange::new([0i64, 1]), Value::Long),
                materialize(
                    EnumeratedValueRange::new([BigInt::ZERO, 10.into()]),
                    Value::BigInteger,
                ),
                materialize(
                    EnumeratedValueRange::new([Decimal::ZERO, decimal("10")]),
                    Value::BigDecimal,
                ),
            ],
            materialize(EnumeratedValueRange::new([0i64, 1]), Value::Long),
        ),
        ProviderForm::List => (
            vec![
                materialize(EnumeratedValueRange::new(vec![0i32, 1]), Value::Integer),
                materialize(EnumeratedValueRange::new(vec![0i64, 1]), Value::Long),
                materialize(
                    EnumeratedValueRange::new(vec![BigInt::ZERO, 10.into()]),
                    Value::BigInteger,
                ),
                materialize(
                    EnumeratedValueRange::new(vec![Decimal::ZERO, decimal("10")]),
                    Value::BigDecimal,
                ),
            ],
            vec![Value::Integer(0), Value::BigInteger(10.into())],
        ),
    };
    // Number accepts every numeric provider, including the heterogeneous list
    // [Integer(0), BigInteger(10)] and the Number[]'s Long values. Native finite
    // variables require distinct values: normalize duplicate occurrences only,
    // preserving every numeric type/value. Sampling multiplicity is not asserted.
    let numbers = SetValueRange::new(typed.iter().flatten().cloned().chain(number_provider));
    let mut fields = vec![numbers.materialize(32).unwrap()];
    fields.extend(typed);
    fields
}
fn named() -> Vec<Vec<Value>> {
    let mut fields = numeric_ranges();
    let date = CivilDate::new(2000, 1, 1).unwrap();
    let start = CivilTime::new(10, 0, 0, 0).unwrap();
    let end = CivilTime::new(10, 3, 0, 0).unwrap();
    fields.extend([
        materialize(
            TemporalValueRange::new(
                date,
                CivilDate::new(2000, 1, 4).unwrap(),
                1,
                TemporalUnit::Days,
            )
            .unwrap(),
            Value::Date,
        ),
        materialize(
            TemporalValueRange::new(start, end, 1, TemporalUnit::Minutes).unwrap(),
            Value::Time,
        ),
        materialize(
            TemporalValueRange::new(
                CivilDateTime::new(date, start),
                CivilDateTime::new(date, end),
                1,
                TemporalUnit::Minutes,
            )
            .unwrap(),
            Value::DateTime,
        ),
        materialize(
            TemporalValueRange::new(
                CalendarYear::new(2000).unwrap(),
                CalendarYear::new(2003).unwrap(),
                1,
                TemporalUnit::Years,
            )
            .unwrap(),
            Value::Year,
        ),
    ]);
    fields
}
fn plan() -> SolverSequence<'static, SimpleScore> {
    SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            }),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 10,
                move_order: MoveOrder::ShuffledVariables { seed: 0 },
                assert_scores: true,
                ..Default::default()
            }),
        ],
        limits: SolverLimits {
            max_move_evaluations: Some(10_000),
            assert_phase_scores: true,
            ..Default::default()
        },
    }
}
fn solve(fields: &[Vec<Value>], stream: bool) -> SolverResult<Assignment<Value>, SimpleScore> {
    // The source's e1/e2 each own every field, initially unassigned. Slot order
    // is entity-major, with heterogeneous values retaining their numeric kind.
    let problem = FiniteProblem::new(
        (0..2)
            .flat_map(|_| fields.iter().map(|values| Variable::new(values.clone())))
            .collect(),
    )
    .unwrap();
    assert!(
        problem
            .initial_assignment()
            .values()
            .iter()
            .all(Option::is_none)
    );
    let mut best_events = 0;
    let mut observe = |_: SolverUpdate<'_, Assignment<Value>, SimpleScore>| best_events += 1;
    let result = if stream {
        solve_stream_with_observer(
            &problem,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<Value>)>();
                b.score("dummy zero", source.port(), |_| SimpleScore::ZERO)?;
                Ok((b.build(), source))
            },
            plan(),
            &mut observe,
        )
        .unwrap()
    } else {
        let mut network = ConstraintNetwork::new(2 * fields.len());
        network
            .add_constraint("dummy zero", (0..2 * fields.len()).collect(), |_| {
                SimpleScore::ZERO
            })
            .unwrap();
        solve_with_observer(&problem, &network, plan(), &mut observe).unwrap()
    };
    // Complete invoked PlannerTestUtils.solve helper assertion.
    assert!(best_events > 0);
    assert!(result.solution().is_some());
    // Additional native checks: full assignment, finite budget and all selected
    // values in their field's complete provider domain. These earn no extra credit.
    assert!(result.complete);
    assert!(result.work.move_evaluations <= 10_000);
    for (slot, value) in result.assignment.values().iter().enumerate() {
        assert!(fields[slot % fields.len()].contains(value.as_ref().unwrap()));
    }
    result
}
fn assert_anonymous(form: ProviderForm) {
    let fields = anonymous(form);
    for stream in [false, true] {
        let result = solve(&fields, stream);
        assert_eq!(result.score, Some(SimpleScore::ZERO));
        for entity in result.assignment.values().chunks_exact(5) {
            assert!(entity[0].is_some()); // numberValue
            assert!(entity[1].is_some()); // integerValue
            assert!(entity[2].is_some()); // longValue
            assert!(entity[3].is_some()); // bigIntegerValue
            assert!(entity[4].is_some()); // bigDecimalValue
        }
        assert!(result.solution().is_some());
    }
}
#[test]
fn solve_value_range() {
    assert_anonymous(ProviderForm::Range);
}
#[test]
fn solve_array() {
    assert_anonymous(ProviderForm::Array);
}
#[test]
fn solve_list() {
    assert_anonymous(ProviderForm::List);
}
#[test]
fn solve_named_value_ranges() {
    for stream in [false, true] {
        assert!(solve(&named(), stream).solution().is_some());
    }
}

// Original domain/binding reference, not another adapted source method.
#[test]
fn all_numeric_kinds_and_temporal_endpoints_remain_distinct() {
    for form in [ProviderForm::Range, ProviderForm::Array, ProviderForm::List] {
        let fields = anonymous(form);
        assert_eq!(fields.len(), 5);
        let (ints, longs, bigs, decimals) = match form {
            ProviderForm::Range => (
                vec![0, 1, 2],
                vec![1000, 1001, 1002],
                vec![1_000_000, 1_000_001, 1_000_002],
                vec!["0.00", "0.01", "0.02"],
            ),
            _ => (vec![0, 1], vec![0, 1], vec![0, 10], vec!["0", "10"]),
        };
        let expected = vec![
            ints.into_iter().map(Value::Integer).collect::<Vec<_>>(),
            longs.into_iter().map(Value::Long).collect(),
            bigs.into_iter()
                .map(|n| Value::BigInteger(n.into()))
                .collect(),
            decimals
                .into_iter()
                .map(|s| Value::BigDecimal(decimal(s)))
                .collect(),
        ];
        assert_eq!(&fields[1..], expected);
        assert_eq!(
            fields[0],
            expected.into_iter().flatten().collect::<Vec<_>>()
        );
    }
    let fields = named();
    assert_eq!(fields.len(), 8);
    for offset in 0..3u8 {
        let date = CivilDate::new(2000, 1, offset + 1).unwrap();
        let time = CivilTime::new(10, offset, 0, 0).unwrap();
        assert_eq!(fields[4][usize::from(offset)], Value::Date(date));
        assert_eq!(fields[5][usize::from(offset)], Value::Time(time));
        assert_eq!(
            fields[6][usize::from(offset)],
            Value::DateTime(CivilDateTime::new(
                CivilDate::new(2000, 1, 1).unwrap(),
                time
            ))
        );
        assert_eq!(
            fields[7][usize::from(offset)],
            Value::Year(CalendarYear::new(2000 + i32::from(offset)).unwrap())
        );
    }
    assert!(fields.iter().all(|field| field.len() == 3));
}
