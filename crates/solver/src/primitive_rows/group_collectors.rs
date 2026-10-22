//! Explicit graph-group adapter for per-edit inverse collector journals.
//!
//! Ordinary PrimitiveRows never uses this protocol. Only exact primitive graph
//! keys opt in. Existing shared/weak owners retain ordinary value replacement;
//! unique library collectors can change in place, with one inverse per edit.
//! Every ownership observer copies inverse-dirty collectors, so an escaped
//! snapshot cannot observe later edits or rollback through the mutable owner.
use super::*;
use crate::{IntCollector, IntSummary};
mod owned;
mod summary_output;
use crate::int_collector_counts::SummarySeed;

#[derive(Clone, Copy)]
pub(crate) enum ScalarEdit {
    Insert(i64),
    Remove(i64),
    Replace(i64, i64),
}
#[derive(Clone, Copy)]
struct Inverse {
    pos: usize,
    // Number of ordinary map edits preceding this scalar edit in its frame.
    normal: usize,
    edit: ScalarEdit,
    // A later structural replacement at the same position has a clean owner.
    // Numeric address identity is never dereferenced and preserves Send/Sync.
    owner: usize,
}
// Compact only the private inverse address. Public row positions, directory
// routing, canonical owners and ordinary map undo retain their full shape.
const WIDE_POSITION: usize = 1usize << (usize::BITS - 1);
const POSITION_BITS: usize = (usize::BITS as usize - 5) / 4;
const POSITION_MASK: usize = (1usize << POSITION_BITS) - 1;
#[derive(Default)]
struct InverseFrame {
    edits: Vec<Inverse>,
    wide: Vec<Position>,
}
impl std::ops::Deref for InverseFrame {
    type Target = [Inverse];
    fn deref(&self) -> &Self::Target {
        &self.edits
    }
}
impl InverseFrame {
    #[inline]
    fn encode(&mut self, pos: Position) -> usize {
        debug_assert!(pos.cell < CELLS);
        if pos.page.0.iter().all(|&part| part <= POSITION_MASK) {
            let mut packed = 0;
            for part in pos.page.0 {
                packed = (packed << POSITION_BITS) | part;
            }
            (packed << 4) | pos.cell
        } else {
            let index = self.wide.len();
            assert!(index < WIDE_POSITION, "inverse address index");
            self.wide.push(pos);
            WIDE_POSITION | index
        }
    }
    #[inline]
    fn decode(wide: &[Position], packed: usize) -> Position {
        if packed & WIDE_POSITION != 0 {
            return wide[packed & !WIDE_POSITION];
        }
        let cell = packed & (CELLS - 1);
        let mut rest = packed >> 4;
        let mut parts = [0; 4];
        for part in parts.iter_mut().rev() {
            *part = rest & POSITION_MASK;
            rest >>= POSITION_BITS;
        }
        Position {
            page: PageKey(parts),
            cell,
        }
    }
    fn clear(&mut self) {
        self.edits.clear();
        self.wide.clear();
    }
}
pub(crate) struct GroupCollectors<K> {
    rows: PrimitiveRows<K, IntCollector>,
    enabled: bool,
    inverses: Vec<InverseFrame>,
    spare: Vec<InverseFrame>,
    owned: Option<Box<owned::Owned<K>>>,
}
impl<K> GroupCollectors<K> {
    pub(crate) fn new() -> Self {
        Self {
            rows: PrimitiveRows::new(),
            enabled: false,
            inverses: Vec::new(),
            spare: Vec::new(),
            owned: None,
        }
    }
    fn paged(&self) -> &Paged<K, IntCollector> {
        let Storage::Paged(rows) = &self.rows.storage else {
            unreachable!("bound group collectors")
        };
        rows
    }
    fn paged_mut(&mut self) -> &mut Paged<K, IntCollector> {
        let Storage::Paged(rows) = &mut self.rows.storage else {
            unreachable!("bound group collectors")
        };
        rows
    }
    fn dirty(&self) -> Vec<(Position, usize)> {
        let mut owners: Vec<_> = self
            .inverses
            .iter()
            .flat_map(|frame| {
                frame
                    .iter()
                    .map(|edit| (InverseFrame::decode(&frame.wide, edit.pos), edit.owner))
            })
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }
    fn entry_for_observer(
        &self,
        pos: Position,
        dirty: &[(Position, usize)],
    ) -> Option<Entry<K, IntCollector>> {
        self.paged().entry(pos).map(|entry| Entry {
            key: entry.key.clone(),
            value: if dirty
                .binary_search(&(pos, Arc::as_ptr(&entry.value) as usize))
                .is_ok()
            {
                Arc::new(entry.value.as_ref().clone())
            } else {
                entry.value.clone()
            },
        })
    }
    #[cfg(test)]
    pub(crate) fn disable_inverse_for_tests(&mut self) {
        assert!(self.inverses.is_empty());
        self.enabled = false;
    }
    pub(crate) fn is_primitive(&self) -> bool {
        self.rows.is_primitive()
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&K, &IntCollector)> {
        self.rows
            .iter()
            .chain(self.owned.iter().flat_map(|rows| rows.iter()))
    }
    pub(crate) fn clear(&mut self) {
        if let Some(rows) = &mut self.owned {
            rows.clear();
            return;
        }
        self.rows.clear();
    }
    pub(crate) fn begin_journal(&mut self) -> usize {
        if let Some(rows) = &mut self.owned {
            return rows.begin();
        }
        let token = self.rows.begin_journal();
        if self.enabled {
            assert_eq!(token, self.inverses.len(), "group inverse journal order");
            self.inverses.push(self.spare.pop().unwrap_or_default());
        }
        token
    }
    pub(crate) fn commit_journal(&mut self, token: usize) {
        if let Some(rows) = &mut self.owned {
            rows.commit(token);
            return;
        }
        if self.enabled {
            assert_eq!(
                self.inverses.len(),
                token + 1,
                "group inverse journal order"
            );
            let offset = token
                .checked_sub(1)
                .map_or(0, |parent| self.paged().frames[parent].len());
            let mut frame = self.inverses.pop().unwrap();
            if let Some(parent) = self.inverses.last_mut() {
                let wide_offset = parent.wide.len();
                parent.wide.append(&mut frame.wide);
                for mut inverse in frame.edits.drain(..) {
                    inverse.normal += offset;
                    if inverse.pos & WIDE_POSITION != 0 {
                        let index = (inverse.pos & !WIDE_POSITION) + wide_offset;
                        assert!(index < WIDE_POSITION, "inverse address index");
                        inverse.pos = WIDE_POSITION | index;
                    }
                    parent.edits.push(inverse);
                }
            }
            frame.clear();
            self.spare.push(frame);
        }
        self.rows.commit_journal(token);
    }
    fn undo_scalar(rows: &mut Paged<K, IntCollector>, inverse: Inverse, pos: Position) {
        let row = rows
            .pages
            .get_mut(&pos.page)
            .expect("inverse group page")
            .cells[pos.cell]
            .as_mut()
            .expect("inverse group cell");
        // Snapshot adapters copy dirty values. make_mut is also needed by the
        // diagnostic journal-snapshot replay, which can restore shared old rows.
        let collector = Arc::make_mut(&mut row.value);
        match inverse.edit {
            ScalarEdit::Insert(value) => {
                assert!(collector.remove(value), "inverse group occurrence")
            }
            ScalarEdit::Remove(value) => collector.insert(value),
            ScalarEdit::Replace(old, new) => assert!(
                collector.replace_captured(new, old),
                "inverse group occurrence"
            ),
        }
    }
    pub(crate) fn rollback_journal(&mut self, token: usize) {
        if let Some(rows) = &mut self.owned {
            rows.rollback(token);
            return;
        }
        if self.enabled {
            assert_eq!(
                self.inverses.len(),
                token + 1,
                "group inverse journal order"
            );
            let mut frame = self.inverses.pop().unwrap();
            let rows = self.paged_mut();
            rows.cache.take();
            for inverse in frame.edits.drain(..).rev() {
                while rows.frames[token].len() > inverse.normal {
                    let undo = rows.frames[token].pop().unwrap();
                    rows.apply_undo(undo);
                }
                Self::undo_scalar(
                    rows,
                    inverse,
                    InverseFrame::decode(&frame.wide, inverse.pos),
                );
            }
            frame.clear();
            self.spare.push(frame);
        }
        self.rows.rollback_journal(token);
    }
    pub(crate) fn capture_current(&self, token: usize) -> RowsCapture<K, IntCollector> {
        if let Some(rows) = &self.owned {
            return rows.capture(token);
        }
        if !self.enabled {
            return self.rows.capture_current(token);
        }
        assert!(token < self.inverses.len(), "active group inverse token");
        let rows = self.paged();
        let mut positions: Vec<_> = rows.frames[token..]
            .iter()
            .flatten()
            .map(|undo| match undo {
                Undo::Value(pos, _) | Undo::Entry(pos, _) => *pos,
            })
            .chain(self.inverses[token..].iter().flat_map(|frame| {
                frame
                    .iter()
                    .map(|edit| InverseFrame::decode(&frame.wide, edit.pos))
            }))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        let dirty = self.dirty();
        RowsCapture::Delta(DeltaRows {
            entries: positions
                .into_iter()
                .map(|pos| (pos, self.entry_for_observer(pos, &dirty)))
                .collect(),
        })
    }
    pub(crate) fn install_capture(&mut self, capture: RowsCapture<K, IntCollector>) {
        if let Some(rows) = &mut self.owned {
            rows.install_capture(capture);
            return;
        }
        self.rows.install_capture(capture);
    }
    pub(crate) fn install_snapshot(&mut self, saved: Self) {
        if let Some(rows) = &mut self.owned {
            if let Some(owned) = saved.owned {
                rows.install(*owned);
            } else {
                rows.install_shared(saved.rows);
            }
            return;
        }
        self.rows.install_snapshot(saved.rows);
    }
    #[cfg(test)]
    pub(crate) fn journal_snapshot(&self, token: usize) -> Self {
        if let Some(rows) = &self.owned {
            let mut saved = self.clone();
            saved.owned = Some(Box::new(rows.journal_snapshot(token)));
            return saved;
        }
        if !self.enabled {
            return Self {
                rows: self.rows.journal_snapshot(token),
                enabled: false,
                inverses: Vec::new(),
                spare: Vec::new(),
                owned: None,
            };
        }
        assert!(token < self.inverses.len(), "active group inverse token");
        let mut saved = self.clone();
        for depth in (token..self.inverses.len()).rev() {
            let normal = &self.paged().frames[depth];
            let mut remaining = normal.len();
            for inverse in self.inverses[depth].iter().rev() {
                while remaining > inverse.normal {
                    remaining -= 1;
                    saved.paged_mut().apply_undo(match &normal[remaining] {
                        Undo::Value(pos, value) => Undo::Value(*pos, value.clone()),
                        Undo::Entry(pos, entry) => Undo::Entry(*pos, entry.clone()),
                    });
                }
                Self::undo_scalar(
                    saved.paged_mut(),
                    *inverse,
                    InverseFrame::decode(&self.inverses[depth].wide, inverse.pos),
                );
            }
            for undo in normal[..remaining].iter().rev() {
                saved.paged_mut().apply_undo(match undo {
                    Undo::Value(pos, value) => Undo::Value(*pos, value.clone()),
                    Undo::Entry(pos, entry) => Undo::Entry(*pos, entry.clone()),
                });
            }
        }
        saved.paged_mut().cleanup();
        saved
    }
    #[cfg(test)]
    pub(crate) fn shared_entries(&self) -> Vec<(Arc<K>, Arc<IntCollector>)> {
        if let Some(rows) = &self.owned {
            return rows.shared_entries();
        }
        if !self.enabled {
            return self.rows.shared_entries();
        }
        let dirty = self.dirty();
        self.paged()
            .pages
            .iter()
            .flat_map(|(key, page)| {
                page.cells
                    .iter()
                    .enumerate()
                    .filter_map(move |(cell, entry)| {
                        entry.as_ref().map(|_| Position { page: key, cell })
                    })
            })
            .map(|pos| {
                let entry = self.entry_for_observer(pos, &dirty).unwrap();
                (entry.key, entry.value)
            })
            .collect()
    }
}
impl<K> Clone for GroupCollectors<K> {
    fn clone(&self) -> Self {
        let mut rows = self.rows.clone();
        if self.enabled {
            let Storage::Paged(copy) = &mut rows.storage else {
                unreachable!()
            };
            let dirty = self.dirty();
            let mut positions: Vec<_> = dirty.iter().map(|(pos, _)| *pos).collect();
            positions.dedup();
            for pos in positions {
                if let Some(entry) = self.entry_for_observer(pos, &dirty) {
                    let retired = copy.replace_value(pos, entry.value);
                    drop(retired);
                }
            }
        }
        Self {
            rows,
            enabled: self.enabled,
            inverses: Vec::new(),
            spare: Vec::new(),
            owned: self.owned.clone(),
        }
    }
}
impl<K: Ord> GroupCollectors<K> {
    #[inline(always)]
    pub(crate) fn apply_owned<P: LookupPolicy<K>>(
        &mut self,
        old: Option<&(K, i64)>,
        new: Option<&(K, i64)>,
        new_key: Option<&K>,
        same: bool,
    ) -> Option<(Option<IntSummary>, Option<IntSummary>)>
    where
        K: Clone,
    {
        self.owned
            .as_mut()
            .map(|rows| rows.apply::<P>(old, new, new_key, same))
    }
    pub(crate) fn has_owned(&self) -> bool {
        self.owned.is_some()
    }
    #[inline(always)]
    pub(crate) fn apply_seeds<P: LookupPolicy<K>>(
        &mut self,
        old: Option<&(K, i64)>,
        new: Option<&(K, i64)>,
        new_key: Option<&K>,
        same: bool,
    ) -> (Option<SummarySeed>, Option<SummarySeed>)
    where
        K: Clone,
    {
        self.owned
            .as_mut()
            .expect("consumed summary groups")
            .apply_output::<P, summary_output::Deferred>(old, new, new_key, same)
    }
    pub(crate) fn get(&self, key: &K) -> Option<&IntCollector> {
        if let Some(rows) = &self.owned {
            return rows.get(key);
        }
        self.rows.get(key)
    }
    pub(crate) fn get_key_value(&self, key: &K) -> Option<(&K, &IntCollector)> {
        if let Some(rows) = &self.owned {
            return rows.get_key_value(key);
        }
        self.rows.get_key_value(key)
    }
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        if let Some(rows) = &self.owned {
            return rows.get(key).is_some();
        }
        self.rows.contains_key(key)
    }
    #[inline]
    pub(crate) fn prepare_change_with_policy<P: LookupPolicy<K>, R>(
        &mut self,
        key: &K,
        inspect: impl FnOnce(Option<&IntCollector>) -> R,
    ) -> (PreparedChange<'_, K, IntCollector>, R) {
        self.rows.prepare_change_with_policy::<P, R>(key, inspect)
    }
    pub(crate) fn insert(&mut self, key: K, value: IntCollector) {
        if let Some(rows) = &mut self.owned {
            rows.insert(key, value);
            return;
        }
        self.rows.insert(key, value);
    }
    #[inline]
    pub(crate) fn insert_with_policy<P: LookupPolicy<K>>(&mut self, key: K, value: IntCollector) {
        if let Some(rows) = &mut self.owned {
            rows.insert(key, value);
            return;
        }
        self.rows.insert_with_policy::<P>(key, value);
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        if let Some(rows) = &mut self.owned {
            return rows.remove(key);
        }
        self.rows.remove(key)
    }
    #[cfg(test)]
    pub(crate) fn try_edit(&mut self, key: &K, edit: ScalarEdit) -> Option<IntSummary> {
        self.try_edit_with_policy::<Stored>(key, edit)
    }
    #[inline(always)]
    pub(crate) fn try_edit_with_policy<P: LookupPolicy<K>>(
        &mut self,
        key: &K,
        edit: ScalarEdit,
    ) -> Option<IntSummary> {
        if !self.enabled || self.inverses.is_empty() {
            return None;
        }
        let rows = match &mut self.rows.storage {
            Storage::Paged(rows) => rows,
            _ => unreachable!(),
        };
        let pos = P::locate(rows.map, key);
        let row = rows.pages.get_mut(&pos.page)?.cells[pos.cell].as_mut()?;
        let count = row.value.occurrence_count();
        if count == 0 || (count == 1 && !matches!(edit, ScalarEdit::Insert(_))) {
            return None;
        }
        let collector = Arc::get_mut(&mut row.value)?;
        let owner = collector as *mut IntCollector as usize;
        if matches!(edit, ScalarEdit::Insert(_)) {
            count.checked_add(1).expect("collector count overflow");
        }
        // Reserve before scalar mutation. These checked library operations call
        // no user code, and the inverse publication itself then cannot allocate.
        let frame = self.inverses.last_mut().unwrap();
        frame.edits.reserve(1);
        let pos = frame.encode(pos);
        rows.cache.take();
        match edit {
            ScalarEdit::Insert(value) => collector.insert(value),
            ScalarEdit::Remove(value) => assert!(collector.remove(value), "input occurrence"),
            ScalarEdit::Replace(old, new) => {
                assert!(collector.replace_captured(old, new), "input occurrence")
            }
        }
        let summary = collector.summary();
        frame.edits.push(Inverse {
            pos,
            normal: rows.frames.last().unwrap().len(),
            edit,
            owner,
        });
        Some(summary)
    }
    pub(crate) fn journal_transaction(&mut self) -> Transaction<'_, K> {
        if !self.enabled {
            return Transaction::Shared(self.rows.journal_transaction());
        }
        let token = self.begin_journal();
        Transaction::Inverse(InverseTransaction {
            rows: self,
            token: Some(token),
        })
    }
}
impl<K: 'static> GroupCollectors<K> {
    pub(crate) fn bind_primitive(&mut self) -> bool {
        let bound = self.rows.bind_primitive();
        if bound && self.rows.is_empty() {
            self.enabled = true;
        }
        bound
    }
    pub(crate) fn enable_consumed_owned(&mut self) {
        if self.owned.is_none()
            && self.rows.is_primitive()
            && self.rows.is_empty()
            && self.inverses.is_empty()
        {
            self.owned = Some(Box::new(owned::Owned::new(self.paged().map)));
        }
    }
    pub(crate) fn enable_int_collector_pool(&mut self) -> bool {
        self.rows.enable_int_collector_pool()
    }
}
pub(crate) enum Transaction<'a, K> {
    Shared(super::Transaction<'a, K, IntCollector>),
    Inverse(InverseTransaction<'a, K>),
}
pub(crate) struct InverseTransaction<'a, K> {
    rows: &'a mut GroupCollectors<K>,
    token: Option<usize>,
}
impl<K: Ord> Transaction<'_, K> {
    pub(crate) fn insert(&mut self, key: K, value: IntCollector) {
        match self {
            Self::Shared(transaction) => transaction.insert(key, value),
            Self::Inverse(transaction) => transaction.rows.insert(key, value),
        }
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        match self {
            Self::Shared(transaction) => transaction.remove(key),
            Self::Inverse(transaction) => transaction.rows.remove(key),
        }
    }
    pub(crate) fn commit(self) {
        match self {
            Self::Shared(transaction) => transaction.commit(),
            Self::Inverse(mut transaction) => transaction
                .rows
                .commit_journal(transaction.token.take().unwrap()),
        }
    }
}
impl<K> Drop for InverseTransaction<'_, K> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.rows.rollback_journal(token);
        }
    }
}

#[cfg(test)]
mod tests;
