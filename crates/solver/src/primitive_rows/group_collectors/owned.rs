mod arena;

use super::super::directory::PageDirectory;
use super::*;

struct OwnedEntry<K> {
    key: Arc<K>,
    value: usize,
}
impl<K> Clone for OwnedEntry<K> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value,
        }
    }
}
struct OwnedPage<K> {
    cells: [Option<OwnedEntry<K>>; CELLS],
    occupied: u16,
}
impl<K> OwnedPage<K> {
    fn empty() -> Self {
        Self {
            cells: std::array::from_fn(|_| None),
            occupied: 0,
        }
    }
}
impl<K> Clone for OwnedPage<K> {
    fn clone(&self) -> Self {
        Self {
            cells: self.cells.clone(),
            occupied: self.occupied,
        }
    }
}
enum OwnedUndo<K> {
    Scalar(usize, ScalarEdit),
    Row(usize, Option<OwnedEntry<K>>),
}
impl<K> OwnedUndo<K> {
    fn position(&self, wide: &[Position], arena: &arena::Arena) -> Position {
        match self {
            Self::Scalar(slot, _) => arena.position(*slot),
            Self::Row(packed, _) => InverseFrame::decode(wide, *packed),
        }
    }
    fn rebase(&mut self, offset: usize) {
        if let Self::Row(p, _) = self
            && *p & WIDE_POSITION != 0
        {
            let index = (*p & !WIDE_POSITION)
                .checked_add(offset)
                .expect("inverse address index");
            assert!(index < WIDE_POSITION, "inverse address index");
            *p = WIDE_POSITION | index;
        }
    }
}
struct OwnedFrame<K> {
    edits: Vec<OwnedUndo<K>>,
    wide: Vec<Position>,
}
impl<K> Default for OwnedFrame<K> {
    fn default() -> Self {
        Self {
            edits: Vec::new(),
            wide: Vec::new(),
        }
    }
}
impl<K> OwnedFrame<K> {
    fn encode(&mut self, pos: Position) -> usize {
        if pos.page.0.iter().all(|&p| p <= POSITION_MASK) {
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
}
pub(super) struct Owned<K> {
    pages: PageDirectory<OwnedPage<K>>,
    map: Mapper<K>,
    len: usize,
    frames: Vec<OwnedFrame<K>>,
    spare: Vec<OwnedFrame<K>>,
    empty: Vec<PageKey>,
    keys: pool::Pool<K>,
    values: arena::Arena,
}
impl<K> Clone for Owned<K> {
    fn clone(&self) -> Self {
        Self {
            pages: self.pages.clone(),
            map: self.map,
            len: self.len,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
            keys: self.keys.empty_clone(),
            values: self.values.clone_live(
                self.pages
                    .values()
                    .flat_map(|p| p.cells.iter().flatten().map(|e| e.value)),
            ),
        }
    }
}
impl<K> Owned<K> {
    pub(super) fn new(map: Mapper<K>) -> Self {
        Self {
            pages: PageDirectory::new(),
            map,
            len: 0,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
            keys: pool::Pool::new(),
            values: arena::Arena::default(),
        }
    }
    fn entry(&self, pos: Position) -> Option<&OwnedEntry<K>> {
        self.pages.get(&pos.page)?.cells[pos.cell].as_ref()
    }
    fn set(&mut self, pos: Position, next: Option<OwnedEntry<K>>) -> Option<OwnedEntry<K>> {
        let page = if next.is_some() {
            self.pages
                .entry(pos.page)
                .or_insert_with(|| Box::new(OwnedPage::empty()))
        } else {
            self.pages.get_mut(&pos.page)?
        };
        let present = next.is_some();
        let old = std::mem::replace(&mut page.cells[pos.cell], next);
        if present {
            page.occupied |= 1 << pos.cell;
        } else {
            page.occupied &= !(1 << pos.cell);
        }
        match (old.is_some(), present) {
            (false, true) => self.len += 1,
            (true, false) => self.len -= 1,
            _ => {}
        }
        if page.occupied == 0 {
            self.empty.push(pos.page);
        }
        old
    }
    fn retire(&mut self, entry: Option<OwnedEntry<K>>) {
        if let Some(OwnedEntry { key, value }) = entry {
            self.keys.retire(key);
            self.values.retire(value);
        }
    }
    fn record_set(&mut self, pos: Position, next: Option<OwnedEntry<K>>) {
        let packed = self.frames.last_mut().map(|frame| {
            frame.edits.reserve(1);
            frame.encode(pos)
        });
        let old = self.set(pos, next);
        if let Some(frame) = self.frames.last_mut() {
            frame.edits.push(OwnedUndo::Row(packed.unwrap(), old));
        } else {
            self.retire(old);
            self.cleanup();
        }
    }
    fn cleanup(&mut self) {
        if !self.frames.is_empty() {
            return;
        }
        self.empty.sort_unstable();
        self.empty.dedup();
        for key in self.empty.drain(..) {
            if self.pages.get(&key).is_some_and(|page| page.occupied == 0) {
                self.pages.remove(&key);
            }
        }
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &IntCollector)> {
        self.pages
            .values()
            .flat_map(|page| page.cells.iter().flatten())
            .map(|e| (e.key.as_ref(), self.values.get(e.value)))
    }
    pub(super) fn get_key_value(&self, key: &K) -> Option<(&K, &IntCollector)> {
        self.entry((self.map)(key))
            .map(|e| (e.key.as_ref(), self.values.get(e.value)))
    }
    pub(super) fn get(&self, key: &K) -> Option<&IntCollector> {
        self.get_key_value(key).map(|(_, v)| v)
    }
    pub(super) fn insert(&mut self, key: K, value: IntCollector) {
        let pos = (self.map)(&key);
        let canonical = self
            .entry(pos)
            .map(|e| e.key.clone())
            .unwrap_or_else(|| self.keys.take(key));
        let value = self.values.insert(pos, value);
        self.record_set(
            pos,
            Some(OwnedEntry {
                key: canonical,
                value,
            }),
        );
    }
    pub(super) fn remove(&mut self, key: &K) -> bool {
        let pos = (self.map)(key);
        if self.entry(pos).is_none() {
            return false;
        }
        self.record_set(pos, None);
        true
    }
    pub(super) fn clear(&mut self) {
        if self.frames.is_empty() {
            self.pages.clear();
            self.values = arena::Arena::default();
            self.empty.clear();
            self.len = 0;
        } else {
            let positions = self.positions();
            for pos in positions {
                self.record_set(pos, None);
            }
        }
    }
    fn positions(&self) -> Vec<Position> {
        self.pages
            .iter()
            .flat_map(|(page, rows)| {
                rows.cells
                    .iter()
                    .enumerate()
                    .filter_map(move |(cell, row)| row.as_ref().map(|_| Position { page, cell }))
            })
            .collect()
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "owned group journal order");
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            let offset = parent.wide.len();
            parent.wide.append(&mut frame.wide);
            for mut edit in frame.edits.drain(..) {
                edit.rebase(offset);
                parent.edits.push(edit);
            }
        } else {
            for edit in frame.edits.drain(..) {
                if let OwnedUndo::Row(_, old) = edit {
                    self.retire(old);
                }
            }
        }
        frame.wide.clear();
        self.spare.push(frame);
        self.cleanup();
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "owned group journal order");
        let mut frame = self.frames.pop().unwrap();
        for edit in frame.edits.drain(..).rev() {
            match edit {
                OwnedUndo::Row(packed, old) => {
                    let pos = InverseFrame::decode(&frame.wide, packed);
                    let retired = self.set(pos, old);
                    self.retire(retired);
                }
                OwnedUndo::Scalar(slot, edit) => {
                    let collector = self.values.get_mut(slot);
                    match edit {
                        ScalarEdit::Insert(v) => {
                            assert!(collector.remove(v), "inverse group occurrence")
                        }
                        ScalarEdit::Remove(v) => collector.insert(v),
                        ScalarEdit::Replace(a, b) => {
                            assert!(collector.replace_captured(b, a), "inverse group occurrence")
                        }
                    }
                }
            }
        }
        frame.wide.clear();
        self.spare.push(frame);
        self.cleanup();
    }
    pub(super) fn capture(&self, token: usize) -> RowsCapture<K, IntCollector> {
        assert!(token < self.frames.len(), "active owned group journal");
        let mut positions: Vec<_> = self.frames[token..]
            .iter()
            .flat_map(|f| f.edits.iter().map(|e| e.position(&f.wide, &self.values)))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        RowsCapture::Delta(DeltaRows {
            entries: positions
                .into_iter()
                .map(|pos| {
                    (
                        pos,
                        self.entry(pos).map(|e| Entry {
                            key: e.key.clone(),
                            value: Arc::new(self.values.get(e.value).clone()),
                        }),
                    )
                })
                .collect(),
        })
    }
    pub(super) fn install_capture(&mut self, capture: RowsCapture<K, IntCollector>) {
        match capture {
            RowsCapture::Delta(delta) => {
                for (pos, entry) in delta.entries {
                    let entry = entry.map(|e| OwnedEntry {
                        key: e.key,
                        value: self.values.insert(pos, Self::value_from_shared(e.value)),
                    });
                    self.record_set(pos, entry);
                }
            }
            RowsCapture::Full(rows) => self.install_shared(rows),
        }
    }
    fn value_from_shared(value: Arc<IntCollector>) -> IntCollector {
        Arc::try_unwrap(value).unwrap_or_else(|v| v.as_ref().clone())
    }
    pub(super) fn install(&mut self, snapshot: Self) {
        if self.frames.is_empty() {
            *self = snapshot;
            return;
        }
        let mut positions = self.positions();
        positions.extend(snapshot.positions());
        positions.sort_unstable();
        positions.dedup();
        for pos in positions {
            let entry = snapshot.entry(pos).map(|e| OwnedEntry {
                key: e.key.clone(),
                value: self
                    .values
                    .insert(pos, snapshot.values.get(e.value).clone()),
            });
            self.record_set(pos, entry);
        }
    }
    pub(super) fn install_shared(&mut self, rows: PrimitiveRows<K, IntCollector>) {
        let snapshot = rows
            .iter()
            .map(|(k, v)| ((self.map)(k), v.clone()))
            .collect::<Vec<_>>();
        // Shared-mode snapshots arise only at the empty activation boundary.
        assert!(
            snapshot.is_empty(),
            "owned groups require owned nonempty full snapshot"
        );
        self.clear();
    }
    #[cfg(test)]
    pub(super) fn shared_entries(&self) -> Vec<(Arc<K>, Arc<IntCollector>)> {
        self.positions()
            .into_iter()
            .map(|p| {
                let row = self.entry(p).unwrap();
                (
                    row.key.clone(),
                    Arc::new(self.values.get(row.value).clone()),
                )
            })
            .collect()
    }
    #[cfg(test)]
    pub(super) fn journal_snapshot(&self, token: usize) -> Self {
        assert!(token < self.frames.len(), "active owned journal");
        let mut saved = self.clone();
        // Historical row tokens may refer to values absent from the current page set.
        saved.values = self.values.clone();
        for frame in self.frames[token..].iter().rev() {
            for edit in frame.edits.iter().rev() {
                match edit {
                    OwnedUndo::Row(packed, old) => {
                        let pos = InverseFrame::decode(&frame.wide, *packed);
                        saved.set(pos, old.clone());
                    }
                    OwnedUndo::Scalar(slot, edit) => {
                        let c = saved.values.get_mut(*slot);
                        match *edit {
                            ScalarEdit::Insert(v) => assert!(c.remove(v)),
                            ScalarEdit::Remove(v) => c.insert(v),
                            ScalarEdit::Replace(a, b) => assert!(c.replace_captured(b, a)),
                        }
                    }
                }
            }
        }
        saved.values.retain_live(
            saved
                .pages
                .values()
                .flat_map(|p| p.cells.iter().flatten().map(|e| e.value)),
        );
        saved.cleanup();
        saved
    }
    #[inline(always)]
    fn scalar<P: LookupPolicy<K>>(&mut self, key: &K, edit: ScalarEdit) -> Option<IntSummary> {
        let pos = P::locate(self.map, key);
        let row = self.pages.get_mut(&pos.page)?.cells[pos.cell].as_mut()?;
        let slot = row.value;
        let count = self.values.get(slot).summary().count;
        if count == 0 || (count == 1 && !matches!(edit, ScalarEdit::Insert(_))) {
            return None;
        }
        if matches!(edit, ScalarEdit::Insert(_)) {
            count.checked_add(1).expect("collector count overflow");
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.edits.reserve(1);
        }
        let collector = self.values.get_mut(slot);
        match edit {
            ScalarEdit::Insert(v) => collector.insert(v),
            ScalarEdit::Remove(v) => assert!(collector.remove(v), "input occurrence"),
            ScalarEdit::Replace(a, b) => {
                assert!(collector.replace_captured(a, b), "input occurrence")
            }
        }
        let summary = collector.summary();
        if let Some(frame) = self.frames.last_mut() {
            frame.edits.push(OwnedUndo::Scalar(slot, edit));
        }
        Some(summary)
    }
}
impl<K: Ord + Clone> Owned<K> {
    #[inline(always)]
    pub(super) fn apply<P: LookupPolicy<K>>(
        &mut self,
        old: Option<&(K, i64)>,
        new: Option<&(K, i64)>,
        new_key: Option<&K>,
        same: bool,
    ) -> (Option<IntSummary>, Option<IntSummary>) {
        if same {
            let (key, old) = old.unwrap();
            let new = new.unwrap().1;
            if *old != new
                && let Some(summary) = self.scalar::<P>(key, ScalarEdit::Replace(*old, new))
            {
                return (Some(summary), None);
            }
            let mut collector = self.get(key).expect("input group").clone();
            if collector.summary().count == 1 {
                assert!(collector.remove(*old), "input occurrence");
                self.remove(key);
                collector.insert(new);
                let summary = collector.summary();
                self.insert(new_key.unwrap().clone(), collector);
                return (Some(summary), None);
            }
            if *old != new {
                assert!(collector.replace_captured(*old, new), "input occurrence");
                let summary = collector.summary();
                self.insert(key.clone(), collector);
                return (Some(summary), None);
            }
            return (Some(collector.summary()), None);
        }
        let before = old.and_then(|(key, value)| {
            if let Some(summary) = self.scalar::<P>(key, ScalarEdit::Remove(*value)) {
                return Some(summary);
            }
            let mut collector = self.get(key).expect("input group").clone();
            assert!(collector.remove(*value), "input occurrence");
            let summary = collector.summary();
            if summary.count == 0 {
                self.remove(key);
                None
            } else {
                self.insert(key.clone(), collector);
                Some(summary)
            }
        });
        let after = new.map(|(key, value)| {
            if let Some(summary) = self.scalar::<P>(key, ScalarEdit::Insert(*value)) {
                return summary;
            }
            let mut collector = self.get(key).cloned().unwrap_or_default();
            collector.insert(*value);
            let summary = collector.summary();
            self.insert(key.clone(), collector);
            summary
        });
        (before, after)
    }
}
