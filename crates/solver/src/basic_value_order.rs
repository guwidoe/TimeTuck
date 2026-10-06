//! Reusable per-variable domain-index ordering for configured move families.
use crate::FiniteProblem;
use std::cmp::Ordering;

/// A complete permutation of a variable's domain indices, including None exactly
/// once for an optional variable. Indices refer to the model's current domain
/// order; this is an explicit configuration, not a dynamic sorting callback.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![30,10,20]).allowing_unassigned()])?;
/// let orders=[BasicValueOrder::sorted_by(&p,0,|a,b|a.cmp(&b))?];
/// assert_eq!(orders[0].values,vec![None,Some(1),Some(2),Some(0)]);
/// let c=BasicNeighborhood{neighborhood:Neighborhood::Change,value_orders:&orders,..Default::default()};
/// let a=p.initial_assignment();
/// assert_eq!(basic_moves_configured(&p,&a,c)?.next(),Some(Move::Change{variable:0,value:Some(10)}));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicValueOrder {
    pub variable: usize,
    pub values: Vec<Option<usize>>,
}
impl BasicValueOrder {
    pub fn new(variable: usize, values: Vec<Option<usize>>) -> Self {
        Self { variable, values }
    }
    /// Stable-sort the actual domain and optional null target without cloning V.
    /// The comparator explicitly defines null placement. Equal keys retain the
    /// model's encounter order. Rebuild after changing domains or sort keys.
    pub fn sorted_by<V: Eq>(
        p: &FiniteProblem<V>,
        variable: usize,
        mut compare: impl FnMut(Option<&V>, Option<&V>) -> Ordering,
    ) -> Result<Self, BasicValueOrderError> {
        let v = p
            .variables()
            .get(variable)
            .ok_or(BasicValueOrderError::UnknownVariable {
                variable,
                count: p.variables().len(),
            })?;
        let mut values = (0..v.domain().len())
            .map(Some)
            .chain(v.allows_unassigned().then_some(None))
            .collect::<Vec<_>>();
        values.sort_by(|a, b| compare(a.map(|i| &v.domain()[i]), b.map(|i| &v.domain()[i])));
        Ok(Self { variable, values })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicValueOrderError {
    UnknownVariable {
        variable: usize,
        count: usize,
    },
    DuplicateVariable {
        variable: usize,
    },
    WrongTargetCount {
        variable: usize,
        expected: usize,
        actual: usize,
    },
    UnknownValue {
        variable: usize,
        index: usize,
        count: usize,
    },
    RequiredNull {
        variable: usize,
    },
    DuplicateValue {
        variable: usize,
        value: Option<usize>,
    },
}
impl std::fmt::Display for BasicValueOrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownVariable { variable, count } => {
                write!(f, "value order variable {variable} outside {count} slots")
            }
            Self::DuplicateVariable { variable } => {
                write!(f, "duplicate value order for variable {variable}")
            }
            Self::WrongTargetCount {
                variable,
                expected,
                actual,
            } => write!(
                f,
                "value order for variable {variable} has {actual} targets, expected {expected}"
            ),
            Self::UnknownValue {
                variable,
                index,
                count,
            } => write!(
                f,
                "value index {index} outside {count} values of variable {variable}"
            ),
            Self::RequiredNull { variable } => write!(
                f,
                "value order for required variable {variable} contains null"
            ),
            Self::DuplicateValue { variable, value } => {
                write!(f, "value order for variable {variable} repeats {value:?}")
            }
        }
    }
}
impl std::error::Error for BasicValueOrderError {}
pub(crate) fn validate<V: Eq>(
    p: &FiniteProblem<V>,
    orders: &[BasicValueOrder],
) -> Result<(), BasicValueOrderError> {
    for (at, order) in orders.iter().enumerate() {
        let i = order.variable;
        let v = p
            .variables()
            .get(i)
            .ok_or(BasicValueOrderError::UnknownVariable {
                variable: i,
                count: p.variables().len(),
            })?;
        if orders[..at].iter().any(|o| o.variable == i) {
            return Err(BasicValueOrderError::DuplicateVariable { variable: i });
        }
        let expected = v.domain().len() + usize::from(v.allows_unassigned());
        if order.values.len() != expected {
            return Err(BasicValueOrderError::WrongTargetCount {
                variable: i,
                expected,
                actual: order.values.len(),
            });
        }
        let mut seen = vec![false; expected];
        for &value in &order.values {
            let index = match value {
                Some(index) if index >= v.domain().len() => {
                    return Err(BasicValueOrderError::UnknownValue {
                        variable: i,
                        index,
                        count: v.domain().len(),
                    });
                }
                Some(index) => index,
                None if !v.allows_unassigned() => {
                    return Err(BasicValueOrderError::RequiredNull { variable: i });
                }
                None => v.domain().len(),
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(BasicValueOrderError::DuplicateValue { variable: i, value });
            }
        }
    }
    Ok(())
}
pub(crate) fn targets<'a, V: Eq>(
    p: &'a FiniteProblem<V>,
    variable: usize,
    orders: &'a [BasicValueOrder],
    assigned: bool,
    unassigned: bool,
) -> impl Iterator<Item = Option<&'a V>> + 'a {
    let v = &p.variables()[variable];
    // Filtering to the sole null target needs no domain or ordering scan.
    let explicit = assigned
        .then(|| orders.iter().find(|o| o.variable == variable))
        .flatten();
    let len = if assigned {
        explicit.map_or_else(
            || v.domain().len() + usize::from(v.allows_unassigned()),
            |o| o.values.len(),
        )
    } else {
        usize::from(unassigned && v.allows_unassigned())
    };
    (0..len)
        .map(move |i| {
            if !assigned {
                None
            } else {
                explicit.map_or_else(|| (i < v.domain().len()).then_some(i), |o| o.values[i])
            }
        })
        .filter(move |i| if i.is_some() { assigned } else { unassigned })
        .map(move |i| i.map(|index| &v.domain()[index]))
}
