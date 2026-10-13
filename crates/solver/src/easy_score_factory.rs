//! Caller-schema-preserving factories for independently owned easy calculators.
use crate::{AdditiveScore, Assignment, ConstraintNetwork, PlanningSchema, ScoringError};
use std::{collections::BTreeMap, error::Error, fmt, str::FromStr};
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CalculatorProperties(BTreeMap<String, String>);
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalculatorPropertyError {
    Missing(String),
    Invalid { name: String, value: String },
}
impl fmt::Display for CalculatorPropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "calculator property: {self:?}")
    }
}
impl Error for CalculatorPropertyError {}
impl CalculatorProperties {
    pub fn new(values: impl IntoIterator<Item = (String, String)>) -> Self {
        Self(values.into_iter().collect())
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }
    pub fn required(&self, name: &str) -> Result<&str, CalculatorPropertyError> {
        self.get(name)
            .ok_or_else(|| CalculatorPropertyError::Missing(name.into()))
    }
    pub fn parse<T: FromStr>(&self, name: &str) -> Result<T, CalculatorPropertyError> {
        let value = self.required(name)?;
        value.parse().map_err(|_| CalculatorPropertyError::Invalid {
            name: name.into(),
            value: value.into(),
        })
    }
}
/// A full assignment/solution calculator; ordinary score contributions must have
/// zero structural levels when adapted into a network. Implement this trait on
/// typed calculators, or supply a plain closure. No reflection/setter discovery.
pub trait EasyScoreCalculator<A> {
    type Score: AdditiveScore;
    fn calculate_score(&self, working: &A) -> Self::Score;
}
impl<A, S: AdditiveScore, F: Fn(&A) -> S> EasyScoreCalculator<A> for F {
    type Score = S;
    fn calculate_score(&self, working: &A) -> S {
        self(working)
    }
}
/// Retain exactly the caller's schema identity and typed property construction.
/// Every build invokes the builder anew; the builder supplies independently owned
/// calculator state. Factory/schema/property data are borrowed or immutable; live
/// working solutions, calculator caches and director state are never shared by
/// this factory. Builder side effects are outside rollback after errors/panics.
pub struct EasyScoreFactory<'s, F> {
    schema: &'s PlanningSchema,
    properties: CalculatorProperties,
    builder: F,
}
impl<'s, F> EasyScoreFactory<'s, F> {
    pub fn new(schema: &'s PlanningSchema, properties: CalculatorProperties, builder: F) -> Self {
        Self {
            schema,
            properties,
            builder,
        }
    }
    pub fn schema(&self) -> &'s PlanningSchema {
        self.schema
    }
    pub fn properties(&self) -> &CalculatorProperties {
        &self.properties
    }
    pub fn build<A, C, E>(&self, working: A) -> Result<EasyScoreSession<'s, A, C>, E>
    where
        F: Fn(&CalculatorProperties) -> Result<C, E>,
        C: EasyScoreCalculator<A>,
    {
        Ok(EasyScoreSession {
            schema: self.schema,
            working,
            calculator: (self.builder)(&self.properties)?,
        })
    }
}
/// Own one working solution and calculator. Generic solutions may use direct easy
/// scoring; basic assignments additionally adapt into existing network directors
/// and configured solving. Neither this session nor the factory re-derives schema.
pub struct EasyScoreSession<'s, A, C> {
    schema: &'s PlanningSchema,
    working: A,
    calculator: C,
}
impl<'s, A, C> EasyScoreSession<'s, A, C> {
    pub fn schema(&self) -> &'s PlanningSchema {
        self.schema
    }
    pub fn calculator(&self) -> &C {
        &self.calculator
    }
    pub fn calculator_mut(&mut self) -> &mut C {
        &mut self.calculator
    }
    pub fn working(&self) -> &A {
        &self.working
    }
    pub fn working_mut(&mut self) -> &mut A {
        &mut self.working
    }
}
impl<A, C: EasyScoreCalculator<A>> EasyScoreSession<'_, A, C> {
    pub fn calculate_score(&self) -> C::Score {
        self.calculator.calculate_score(&self.working)
    }
}
impl<V: Clone, C: EasyScoreCalculator<Assignment<V>>> EasyScoreSession<'_, Assignment<V>, C> {
    /// Register one real full-assignment constraint in the existing native network.
    /// All basic slots are dependencies, including an empty scope for an empty model.
    /// Network/director structural score and checked arithmetic remain authoritative.
    /// Each full easy invocation owns a cloned assignment view; this adapter does not
    /// claim incremental calculator performance. Retained application facts belong in
    /// the calculator, and arbitrary Java domain discovery is not inferred here.
    pub fn network(
        &self,
        variable_count: usize,
    ) -> Result<ConstraintNetwork<'_, V, C::Score>, ScoringError> {
        let mut network = ConstraintNetwork::new(variable_count);
        network.add_constraint("easy calculator", (0..variable_count).collect(), |values| {
            let a = Assignment::new(values.iter().map(|v| v.cloned()).collect());
            self.calculator.calculate_score(&a)
        })?;
        Ok(network)
    }
}
