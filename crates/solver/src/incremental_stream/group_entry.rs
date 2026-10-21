//! Reuse prepared primitive group entries and their computed summaries.
//!
//! Exact primitive keys have no user Clone/Ord/Drop hooks. Their collector
//! summaries contain only integers, so computing emissions before publishing the
//! input cannot expose user behavior. Input preparation/publication, output-key
//! clones and poison boundaries stay in GroupNode's common path. Generic group
//! keys keep its original lookup and publication sequence.
use super::*;
use crate::primitive_rows::{
    LookupPolicy,
    group_collectors::{GroupCollectors, ScalarEdit},
};

#[inline(always)]
pub(super) fn apply<K: Ord + Clone, P: LookupPolicy<K>>(
    groups: &mut GroupCollectors<K>,
    old: Option<&(K, i64)>,
    new: Option<&(K, i64)>,
    new_key: Option<&K>,
    same_group: bool,
) -> (Option<IntSummary>, Option<IntSummary>) {
    debug_assert!(groups.is_primitive());
    if let Some(output) = groups.apply_owned::<P>(old, new, new_key, same_group) {
        return output;
    }
    if same_group {
        let (key, old) = old.expect("existing input");
        let new = new.expect("replacement input").1;
        if *old != new
            && let Some(summary) =
                groups.try_edit_with_policy::<P>(key, ScalarEdit::Replace(*old, new))
        {
            return (Some(summary), None);
        }
        let (prepared, mut collector) =
            groups.prepare_change_with_policy::<P, _>(key, |row| row.expect("input group").clone());
        if collector.summary().count == 1 {
            assert!(collector.remove(*old), "input occurrence");
            // Preserve actual removal and reinsertion, including the new key
            // representative and each intermediate journal entry.
            prepared.remove();
            collector.insert(new);
            let summary = collector.summary();
            groups.insert_with_policy::<P>(new_key.unwrap().clone(), collector);
            return (Some(summary), None);
        }
        if *old != new {
            assert!(collector.replace_captured(*old, new), "input occurrence");
            let summary = collector.summary();
            prepared.insert(key.clone(), collector);
            return (Some(summary), None);
        }
        // Equal replacements still emit the current summary without changing
        // either the collector owner or the group's key representative.
        return (Some(collector.summary()), None);
    }
    let old_summary = old.and_then(|(key, value)| {
        if let Some(summary) = groups.try_edit_with_policy::<P>(key, ScalarEdit::Remove(*value)) {
            return Some(summary);
        }
        let (prepared, mut collector) =
            groups.prepare_change_with_policy::<P, _>(key, |row| row.expect("input group").clone());
        assert!(collector.remove(*value), "input occurrence");
        let summary = collector.summary();
        if summary.count == 0 {
            prepared.remove();
            None
        } else {
            prepared.insert(key.clone(), collector);
            Some(summary)
        }
    });
    let new_summary = new.map(|(key, value)| {
        if let Some(summary) = groups.try_edit_with_policy::<P>(key, ScalarEdit::Insert(*value)) {
            return summary;
        }
        let (prepared, mut collector) =
            groups.prepare_change_with_policy::<P, _>(key, |row| row.cloned().unwrap_or_default());
        collector.insert(*value);
        let summary = collector.summary();
        prepared.insert(key.clone(), collector);
        summary
    });
    (old_summary, new_summary)
}

#[cfg(test)]
mod tests;
