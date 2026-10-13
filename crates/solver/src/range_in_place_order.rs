//! Explicit in-place ordering capabilities for native range owners.
use crate::{
    BigIntegerValueRange, BooleanValueRange, DecimalValueRange, EnumeratedValueRange,
    IntegerValueRange, ValueRange,
};
use std::{cmp::Ordering, error::Error, fmt};
pub type RangeOrderComparator<'a, V> = dyn Fn(&V, &V) -> Ordering + 'a;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RangeOrderError {
    UnsupportedSorting,
}
impl fmt::Display for RangeOrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("this generated range cannot be sorted in place")
    }
}
impl Error for RangeOrderError {}
/// Arithmetic/boolean generators have a fixed intrinsic index order and reject
/// in-place sorting without enumerating values or invoking the comparator.
/// Stored enumerations can explicitly publish a reordered value sequence. A
/// sorted copy remains available through the existing bounded `sorted_by` API.
pub trait RangeInPlaceOrder: ValueRange {
    fn sort(
        &mut self,
        _compare: Option<&RangeOrderComparator<'_, Self::Value>>,
    ) -> Result<(), RangeOrderError> {
        Err(RangeOrderError::UnsupportedSorting)
    }
}
impl RangeInPlaceOrder for IntegerValueRange {}
impl RangeInPlaceOrder for BigIntegerValueRange {}
impl RangeInPlaceOrder for DecimalValueRange {}
impl RangeInPlaceOrder for BooleanValueRange {}
impl<V: Clone + Eq + Ord> RangeInPlaceOrder for EnumeratedValueRange<V> {
    fn sort(
        &mut self,
        compare: Option<&RangeOrderComparator<'_, V>>,
    ) -> Result<(), RangeOrderError> {
        let mut values = self.values().to_vec();
        values.sort_by(|a, b| compare.map_or_else(|| a.cmp(b), |compare| compare(a, b)));
        *self = Self::new(values);
        Ok(())
    }
}
