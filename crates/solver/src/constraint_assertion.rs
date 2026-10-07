//! Assertions over retained constraint matches, without rerunning scoring callbacks.
use crate::{
    AdditiveScore, Decimal, DecimalWeightedScore, GraphError, StreamConstraint, StreamGraph,
    StreamGraphBuilder, StreamPort,
};
use std::{cmp::Ordering, fmt};

/// A terminal retaining the original tuple and its signed, unweighted impact.
/// Tracking is opt-in and adds a mapping node. Normal zero-weight pruning applies:
/// a disabled constraint has no live matches. Raw factor sign defines penalty or
/// reward even if a signed constraint-weight override reverses the weighted score.
#[derive(Clone)]
pub struct TrackedConstraint<I, T, W> {
    terminal: StreamConstraint<I, (T, W)>,
}
impl<I, T, W> TrackedConstraint<I, T, W> {
    pub fn terminal(&self) -> &StreamConstraint<I, (T, W)> {
        &self.terminal
    }
}

/// Register integer impacts, computing each factor once per propagated tuple
/// update and retaining it for later assertions. Uses the graph's score arithmetic
/// and weight overrides. This composes ordinary map/terminal builder operations;
/// as with other multi-node builder operations, discard the builder on error.
pub fn track_constraint<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    S: AdditiveScore + 'static,
>(
    builder: &mut StreamGraphBuilder<'a, S>,
    name: impl Into<String>,
    input: &StreamPort<I, T>,
    weight: S,
    factor: impl Fn(&T) -> i64 + 'a,
) -> Result<TrackedConstraint<I, T, i64>, GraphError> {
    let tracked = builder.map(input, move |tuple| (tuple.clone(), factor(tuple)))?;
    let terminal = builder.score_weighted(name, &tracked, weight, |(_, factor)| *factor)?;
    Ok(TrackedConstraint { terminal })
}
/// Decimal counterpart, retaining exact fractional impacts without float conversion.
pub fn track_decimal_constraint<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    S: DecimalWeightedScore + 'static,
>(
    builder: &mut StreamGraphBuilder<'a, S>,
    name: impl Into<String>,
    input: &StreamPort<I, T>,
    weight: S,
    factor: impl Fn(&T) -> Decimal + 'a,
) -> Result<TrackedConstraint<I, T, Decimal>, GraphError> {
    let tracked = builder.map(input, move |tuple| (tuple.clone(), factor(tuple)))?;
    let terminal =
        builder.score_weighted_decimal(name, &tracked, weight, |(_, factor)| factor.clone())?;
    Ok(TrackedConstraint { terminal })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintComparison<T> {
    Exactly(T),
    MoreThan(T),
    LessThan(T),
}
impl<T> ConstraintComparison<T> {
    fn value(&self) -> &T {
        match self {
            Self::Exactly(v) | Self::MoreThan(v) | Self::LessThan(v) => v,
        }
    }
    fn accepts(&self, order: Ordering) -> bool {
        match self {
            Self::Exactly(_) => order.is_eq(),
            Self::MoreThan(_) => order.is_gt(),
            Self::LessThan(_) => order.is_lt(),
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Exactly(_) => "",
            Self::MoreThan(_) => " more than",
            Self::LessThan(_) => " less than",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConstraintExpectation {
    PenaltyCount(ConstraintComparison<usize>),
    RewardCount(ConstraintComparison<usize>),
    PenaltyImpact(ConstraintComparison<Decimal>),
    RewardImpact(ConstraintComparison<Decimal>),
    /// No nonzero raw impacts; cancelling penalties/rewards do not qualify.
    NoImpact,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JustificationMatch {
    Contains,
    Exact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstraintAssertionError {
    pub constraint: String,
    pub context: Option<String>,
    pub expected: String,
    pub actual: String,
    pub details: Vec<String>,
}
impl fmt::Display for ConstraintAssertionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}; Constraint: {}; {}; Actual {}",
            self.context.as_deref().unwrap_or("Broken expectation"),
            self.constraint,
            self.expected,
            self.actual
        )?;
        for detail in &self.details {
            write!(f, "; {detail}")?;
        }
        Ok(())
    }
}
impl std::error::Error for ConstraintAssertionError {}

/// Detached snapshot. Counts preserve tuple occurrences, including equal payloads
/// at different IDs. Numeric impact checks ignore decimal scale, while scores and
/// justifications keep their own equality semantics. Snapshots never repair or
/// mutate graphs. They remain valid observations after later graph mutations.
#[derive(Clone, Debug)]
pub struct ConstraintAssertion<S, J> {
    name: String,
    score: S,
    penalty_count: usize,
    reward_count: usize,
    penalty_impact: Decimal,
    reward_impact: Decimal,
    justifications: Vec<J>,
}
impl<S, J> ConstraintAssertion<S, J> {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn score(&self) -> &S {
        &self.score
    }
    pub fn penalty_count(&self) -> usize {
        self.penalty_count
    }
    pub fn reward_count(&self) -> usize {
        self.reward_count
    }
    pub fn penalty_impact(&self) -> &Decimal {
        &self.penalty_impact
    }
    pub fn reward_impact(&self) -> &Decimal {
        &self.reward_impact
    }
    /// Sum of penalty and reward magnitudes, independent of constraint weight.
    pub fn impact(&self) -> Decimal {
        self.penalty_impact.add(&self.reward_impact)
    }
    pub fn justifications(&self) -> &[J] {
        &self.justifications
    }
    fn failure(
        &self,
        expected: String,
        actual: String,
        context: Option<&str>,
    ) -> ConstraintAssertionError {
        ConstraintAssertionError {
            constraint: self.name.clone(),
            context: context.map(str::to_owned),
            expected,
            actual,
            details: vec![],
        }
    }
    pub fn check(
        &self,
        expected: ConstraintExpectation,
        context: Option<&str>,
    ) -> Result<(), ConstraintAssertionError> {
        use ConstraintExpectation::*;
        let (pass, description, actual) = match expected {
            PenaltyCount(ref c) | RewardCount(ref c) => {
                let penalty = matches!(expected, PenaltyCount(_));
                let (kind, actual) = if penalty {
                    ("penalty", self.penalty_count)
                } else {
                    ("reward", self.reward_count)
                };
                (
                    c.accepts(actual.cmp(c.value())),
                    format!("Expected {kind}{} {} (match count)", c.label(), c.value()),
                    format!("{kind} count {actual}"),
                )
            }
            PenaltyImpact(ref c) | RewardImpact(ref c) => {
                let (kind, actual) = if matches!(expected, PenaltyImpact(_)) {
                    ("penalty", &self.penalty_impact)
                } else {
                    ("reward", &self.reward_impact)
                };
                (
                    c.accepts(actual.numeric_cmp(c.value())),
                    format!(
                        "Expected {kind}{} {} (unweighted impact)",
                        c.label(),
                        c.value()
                    ),
                    format!("{kind} impact {actual}"),
                )
            }
            NoImpact => (
                self.penalty_count == 0 && self.reward_count == 0,
                "Expected no impact".into(),
                format!(
                    "impact: penalty {}, reward {}",
                    self.penalty_impact, self.reward_impact
                ),
            ),
        };
        if pass {
            Ok(())
        } else {
            Err(self.failure(description, actual, context))
        }
    }
}
impl<S, J: PartialEq + fmt::Debug> ConstraintAssertion<S, J> {
    /// Order-independent occurrence matching. Contains allows extra occurrences;
    /// Exact does not. An empty expectation explicitly requires no justifications
    /// in either mode. Equal occurrences are matched once, preserving multiplicity.
    pub fn check_justifications(
        &self,
        expected: &[J],
        mode: JustificationMatch,
        context: Option<&str>,
    ) -> Result<(), ConstraintAssertionError> {
        let mut used = vec![false; self.justifications.len()];
        let mut missing = Vec::new();
        for wanted in expected {
            if let Some(index) = self
                .justifications
                .iter()
                .enumerate()
                .position(|(i, actual)| !used[i] && actual == wanted)
            {
                used[index] = true;
            } else {
                missing.push(wanted);
            }
        }
        let unexpected: Vec<_> = if mode == JustificationMatch::Exact || expected.is_empty() {
            self.justifications
                .iter()
                .enumerate()
                .filter_map(|(i, j)| (!used[i]).then_some(j))
                .collect()
        } else {
            vec![]
        };
        if missing.is_empty() && unexpected.is_empty() {
            return Ok(());
        }
        fn render<J: fmt::Debug>(values: &[J]) -> String {
            if values.is_empty() {
                "No Justification".into()
            } else {
                format!("{values:?}")
            }
        }
        let mut error = self.failure(
            format!(
                "Expected Justification: {}: {}",
                self.name,
                render(expected)
            ),
            render(&self.justifications),
            context,
        );
        if !missing.is_empty() {
            error
                .details
                .push(format!("Expected but not found: {missing:?}"));
        }
        if !unexpected.is_empty() {
            error
                .details
                .push(format!("Unexpected but found: {unexpected:?}"));
        }
        Err(error)
    }
}

/// Snapshot a tracked terminal. Justification construction runs once per retained
/// match, in tuple-ID order; scoring/factor callbacks are never rerun. A foreign
/// handle is rejected before callbacks run. Zero-factor matches still contribute
/// justifications but count as neither penalties nor rewards.
///
/// ```
/// use timetuck::*;
/// let mut b = StreamGraphBuilder::<HardSoftScore>::new();
/// let source = b.source::<usize, i64>();
/// let constraint = track_constraint(&mut b, "cost", source.port(), HardSoftScore::new(0, 2), |v| -*v)?;
/// let mut graph = b.build();
/// let mut tx = graph.transaction();
/// tx.apply(&source, TupleChange::put(0, 3))?;
/// tx.commit()?;
/// let assertion = constraint_assertion(&graph, &constraint, |v, _| *v)?;
/// assertion.check(ConstraintExpectation::PenaltyImpact(ConstraintComparison::Exactly(Decimal::from(3))), None)?;
/// assert_eq!(assertion.score(), &HardSoftScore::new(0, -6));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn constraint_assertion<
    I: Ord + 'static,
    T: 'static,
    W: Clone + Into<Decimal> + 'static,
    S: AdditiveScore + 'static,
    J,
>(
    graph: &StreamGraph<'_, S>,
    constraint: &TrackedConstraint<I, T, W>,
    mut justify: impl FnMut(&T, &S) -> J,
) -> Result<ConstraintAssertion<S, J>, GraphError> {
    let name = graph.constraint_name(&constraint.terminal)?;
    let score = graph
        .constraint_scores()
        .find(|(n, _)| *n == name)
        .expect("registered constraint")
        .1;
    let mut out = ConstraintAssertion {
        name: name.to_owned(),
        score,
        penalty_count: 0,
        reward_count: 0,
        penalty_impact: Decimal::ZERO,
        reward_impact: Decimal::ZERO,
        justifications: vec![],
    };
    for (_, ((tuple, factor), score)) in graph.constraint_matches(&constraint.terminal)?.iter() {
        let factor: Decimal = factor.clone().into();
        match factor.numeric_cmp(&Decimal::ZERO) {
            Ordering::Less => {
                out.penalty_count += 1;
                out.penalty_impact = out.penalty_impact.subtract(&factor);
            }
            Ordering::Greater => {
                out.reward_count += 1;
                out.reward_impact = out.reward_impact.add(&factor);
            }
            Ordering::Equal => {}
        }
        out.justifications.push(justify(tuple, score));
    }
    Ok(out)
}
