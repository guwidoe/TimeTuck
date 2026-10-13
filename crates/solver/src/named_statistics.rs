//! Named model statistics with validated native slot bindings.
use crate::{AssignmentSpace, FiniteProblem, ListProblem, MixedProblem, ProblemStatistics};
use num_bigint::BigUint;
use std::{collections::BTreeMap, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatisticsVariableKind {
    Basic,
    List,
}
/// One variable field across all instances of an entity type. Slots follow entity
/// order. A basic field with no instances needs its declared range size, since an
/// empty FiniteProblem has no independent value registry. List ranges remain known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticsVariable {
    pub name: String,
    pub kind: StatisticsVariableKind,
    pub slots: Vec<usize>,
    pub empty_value_count: Option<usize>,
}
impl StatisticsVariable {
    pub fn basic(name: impl Into<String>, slots: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            kind: StatisticsVariableKind::Basic,
            slots,
            empty_value_count: None,
        }
    }
    pub fn list(name: impl Into<String>, slots: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            kind: StatisticsVariableKind::List,
            slots,
            empty_value_count: None,
        }
    }
    pub fn with_empty_value_count(mut self, count: usize) -> Self {
        self.empty_value_count = Some(count);
        self
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatisticsEntityType {
    pub name: String,
    pub entity_count: usize,
    pub variables: Vec<StatisticsVariable>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatisticsSchemaError {
    DuplicateEntityType(String),
    DuplicateVariable {
        entity_type: String,
        variable: String,
    },
    SlotCount {
        entity_type: String,
        variable: String,
        expected: usize,
        actual: usize,
    },
    InvalidSlot {
        kind: StatisticsVariableKind,
        slot: usize,
    },
    DuplicateSlot {
        kind: StatisticsVariableKind,
        slot: usize,
    },
    UnboundSlots {
        kind: StatisticsVariableKind,
        slots: Vec<usize>,
    },
    MissingEmptyRange {
        entity_type: String,
        variable: String,
    },
    CountOverflow,
}
impl fmt::Display for StatisticsSchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid statistics schema: {self:?}")
    }
}
impl Error for StatisticsSchemaError {}
/// Entity counts use declared entity rows rather than native flattened slots.
/// Every native slot must be bound exactly once. Value counts are distinct domain
/// values within each type/field, summed across fields (separate namespaces).
/// Assignment space keeps the native exact/upper-bound distinction and pins.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]);3])?;
/// let schema=[StatisticsEntityType {name:"Task".into(),entity_count:3,
///     variables:vec![StatisticsVariable::basic("worker",vec![0,1,2])]}];
/// let report=NamedProblemSize::from_basic(&p,&schema)?;
/// assert_eq!(report.entities_by_type["Task"],3);
/// assert_eq!(report.scale().formatted(),"8");
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedProblemSize {
    pub entity_count: usize,
    pub variable_count: usize,
    pub approximate_value_count: usize,
    pub entities_by_type: BTreeMap<String, usize>,
    pub values_by_type_variable: BTreeMap<String, BTreeMap<String, usize>>,
    pub assignment_space: AssignmentSpace,
}
impl NamedProblemSize {
    pub fn from_basic<V: Eq + Clone>(
        problem: &FiniteProblem<V>,
        schema: &[StatisticsEntityType],
    ) -> Result<Self, StatisticsSchemaError> {
        Self::collect::<V, ()>(
            Some(problem),
            None,
            schema,
            problem.problem_size_statistics().assignment_space,
        )
    }
    pub fn from_list<V: Eq + Clone>(
        problem: &ListProblem<V>,
        schema: &[StatisticsEntityType],
    ) -> Result<Self, StatisticsSchemaError> {
        Self::collect::<(), V>(
            None,
            Some(problem),
            schema,
            problem.problem_size_statistics().assignment_space,
        )
    }
    pub fn from_mixed<B: Eq + Clone, L: Eq + Clone>(
        problem: &MixedProblem<B, L>,
        schema: &[StatisticsEntityType],
    ) -> Result<Self, StatisticsSchemaError> {
        Self::collect(
            Some(problem.basic()),
            Some(problem.lists()),
            schema,
            problem.problem_size_statistics().assignment_space,
        )
    }
    pub fn scale(&self) -> ProblemScale {
        ProblemScale::from_count(&self.assignment_space.count)
    }
    fn collect<B: Eq + Clone, L: Eq + Clone>(
        basic: Option<&FiniteProblem<B>>,
        lists: Option<&ListProblem<L>>,
        schema: &[StatisticsEntityType],
        assignment_space: AssignmentSpace,
    ) -> Result<Self, StatisticsSchemaError> {
        let mut out = Self {
            entity_count: 0,
            variable_count: 0,
            approximate_value_count: 0,
            entities_by_type: BTreeMap::new(),
            values_by_type_variable: BTreeMap::new(),
            assignment_space,
        };
        let mut seen_basic = vec![false; basic.map_or(0, |p| p.variables().len())];
        let mut seen_list = vec![false; lists.map_or(0, |p| p.entities().len())];
        for entity in schema {
            if out
                .entities_by_type
                .insert(entity.name.clone(), entity.entity_count)
                .is_some()
            {
                return Err(StatisticsSchemaError::DuplicateEntityType(
                    entity.name.clone(),
                ));
            }
            out.entity_count = out
                .entity_count
                .checked_add(entity.entity_count)
                .ok_or(StatisticsSchemaError::CountOverflow)?;
            let mut fields = BTreeMap::new();
            for field in &entity.variables {
                if fields.contains_key(&field.name) {
                    return Err(StatisticsSchemaError::DuplicateVariable {
                        entity_type: entity.name.clone(),
                        variable: field.name.clone(),
                    });
                }
                if field.slots.len() != entity.entity_count {
                    return Err(StatisticsSchemaError::SlotCount {
                        entity_type: entity.name.clone(),
                        variable: field.name.clone(),
                        expected: entity.entity_count,
                        actual: field.slots.len(),
                    });
                }
                let seen = match field.kind {
                    StatisticsVariableKind::Basic => &mut seen_basic,
                    StatisticsVariableKind::List => &mut seen_list,
                };
                for &slot in &field.slots {
                    let used = seen
                        .get_mut(slot)
                        .ok_or(StatisticsSchemaError::InvalidSlot {
                            kind: field.kind,
                            slot,
                        })?;
                    if std::mem::replace(used, true) {
                        return Err(StatisticsSchemaError::DuplicateSlot {
                            kind: field.kind,
                            slot,
                        });
                    }
                }
                let count = match field.kind {
                    StatisticsVariableKind::Basic => {
                        if field.slots.is_empty() {
                            field.empty_value_count.ok_or_else(|| {
                                StatisticsSchemaError::MissingEmptyRange {
                                    entity_type: entity.name.clone(),
                                    variable: field.name.clone(),
                                }
                            })?
                        } else {
                            let p = basic.expect("validated basic slots");
                            let mut values = vec![];
                            for &slot in &field.slots {
                                for value in p.variables()[slot].domain() {
                                    if !values.contains(&value) {
                                        values.push(value);
                                    }
                                }
                            }
                            values.len()
                        }
                    }
                    StatisticsVariableKind::List => {
                        let p = lists.ok_or(StatisticsSchemaError::InvalidSlot {
                            kind: field.kind,
                            slot: 0,
                        })?;
                        if field.slots.is_empty() {
                            p.values().len()
                        } else {
                            let mut values = vec![];
                            for &slot in &field.slots {
                                for value in p.entities()[slot].value_range().unwrap_or(p.values())
                                {
                                    if !values.contains(&value) {
                                        values.push(value);
                                    }
                                }
                            }
                            values.len()
                        }
                    }
                };
                out.variable_count = out
                    .variable_count
                    .checked_add(field.slots.len())
                    .ok_or(StatisticsSchemaError::CountOverflow)?;
                out.approximate_value_count = out
                    .approximate_value_count
                    .checked_add(count)
                    .ok_or(StatisticsSchemaError::CountOverflow)?;
                fields.insert(field.name.clone(), count);
            }
            out.values_by_type_variable
                .insert(entity.name.clone(), fields);
        }
        for (kind, seen) in [
            (StatisticsVariableKind::Basic, seen_basic),
            (StatisticsVariableKind::List, seen_list),
        ] {
            let slots: Vec<_> = seen
                .iter()
                .enumerate()
                .filter_map(|(i, used)| (!used).then_some(i))
                .collect();
            if !slots.is_empty() {
                return Err(StatisticsSchemaError::UnboundSlots { kind, slots });
            }
        }
        Ok(out)
    }
}
/// Base-10 size estimate that remains finite for very large integer spaces.
/// Formatted output uses US digit grouping below 10^10 and six decimal places
/// in scientific notation above it. Nonfinite imported logarithms format as zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProblemScale {
    logarithm: f64,
}
impl ProblemScale {
    /// Fixed-point precision of the pinned Community problem-scale statistic.
    pub const FIXED_POINT_PRECISION: u64 = 1_000_000;
    pub fn from_log10(logarithm: f64) -> Self {
        Self { logarithm }
    }
    pub fn from_count(count: &BigUint) -> Self {
        if count.bits() == 0 {
            return Self::from_log10(f64::NEG_INFINITY);
        }
        let shift = count.bits().saturating_sub(53);
        let leading = (count >> shift).to_u64_digits()[0];
        Self::from_log10((leading as f64).log10() + shift as f64 * std::f64::consts::LOG10_2)
    }
    pub fn log10(self) -> f64 {
        self.logarithm
    }
    /// Explicit native precision; ties away from zero and saturated i64 bounds.
    /// fixed_point_log supplies the pinned statistic's default precision/rule.
    pub fn scaled_log(self, precision: u64) -> i64 {
        (self.logarithm * precision as f64).round() as i64
    }
    /// Default fixed-point statistic: nearest integer, ties toward positive
    /// infinity, saturated i64 bounds and zero for NaN. Explicit scaled_log
    /// retains its existing native rounding contract.
    pub fn fixed_point_log(self) -> i64 {
        let scaled = self.logarithm * Self::FIXED_POINT_PRECISION as f64;
        let lower = scaled.floor();
        // Adding 0.5 directly loses integral values above f64's fractional range.
        (if scaled - lower >= 0.5 {
            lower + 1.0
        } else {
            lower
        }) as i64
    }
    pub fn formatted(self) -> String {
        if !self.logarithm.is_finite() {
            return "0".into();
        }
        if self.logarithm < 10.0 {
            let digits = format!("{:.0}", 10f64.powf(self.logarithm));
            let mut out = String::new();
            for (i, c) in digits.chars().enumerate() {
                if i > 0 && (digits.len() - i) % 3 == 0 {
                    out.push(',');
                }
                out.push(c);
            }
            return out;
        }
        let mut exponent = self.logarithm.floor();
        let mut mantissa = 10f64.powf(self.logarithm - exponent);
        if mantissa >= 9.9999995 {
            mantissa = 1.0;
            exponent += 1.0;
        }
        format!("{mantissa:.6} × 10^{exponent:.0}")
    }
}
