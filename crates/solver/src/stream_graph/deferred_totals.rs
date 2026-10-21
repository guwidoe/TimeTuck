//! Drain-local score probes for certified ownership-consuming builtin graphs.
//! Event application, per-terminal overflow checks, callbacks and accounting
//! are unchanged. Generic scores/public graphs retain the original probe path.
use crate::AdditiveScore;

pub(super) struct Totals<S> {
    enabled: bool,
    seen: Vec<bool>,
    old: Vec<(usize, S)>,
}
impl<S> Default for Totals<S> {
    fn default() -> Self {
        Self {
            enabled: false,
            seen: Vec::new(),
            old: Vec::new(),
        }
    }
}
impl<S> Totals<S> {
    pub(super) fn begin(&mut self, enabled: bool, nodes: usize) {
        debug_assert!(self.old.is_empty());
        self.enabled = enabled;
        if enabled {
            self.seen.resize(nodes, false);
        }
    }
    #[inline(always)]
    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }
    #[inline(always)]
    pub(super) fn record(&mut self, id: usize, old: impl FnOnce() -> S) {
        debug_assert!(self.enabled);
        if !self.seen[id] {
            self.old.push((id, old()));
            self.seen[id] = true;
        }
    }
    pub(super) fn clear(&mut self) {
        for (id, _) in &self.old {
            self.seen[*id] = false;
        }
        self.old.clear();
        self.enabled = false;
    }
}
impl<S: AdditiveScore> Totals<S> {
    pub(super) fn append_changes(
        &self,
        mut current: impl FnMut(usize) -> S,
        changes: &mut Vec<(S, S)>,
    ) {
        for (id, old) in &self.old {
            let new = current(*id);
            if old != &new {
                changes.push((old.clone(), new));
            }
        }
    }
}

#[cfg(test)]
mod tests;
