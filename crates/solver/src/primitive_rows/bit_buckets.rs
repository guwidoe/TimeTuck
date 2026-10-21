//! Private primitive join buckets: one inverse for each unshared bitmap edit.
//! Ordinary PrimitiveRows and public/custom map ownership remain unchanged.
//! Dirty observers receive value copies, retaining the real canonical key Arcs.
use super::*;

pub(crate) trait BitValue: Clone {
    fn count(&self) -> usize;
    fn bit(&self, id: usize) -> bool;
    fn assign(&mut self, id: usize, present: bool);
}
#[derive(Clone, Copy)]
struct Inverse {
    pos: Position,
    normal: usize,
    owner: usize,
    id: usize,
    present: bool,
}
pub(crate) struct BitBuckets<K, V: BitValue> {
    rows: PrimitiveRows<K, V>,
    inverses: Vec<Vec<Inverse>>,
    spare: Vec<Vec<Inverse>>,
}
impl<K, V: BitValue> BitBuckets<K, V> {
    pub(crate) fn new(rows: PrimitiveRows<K, V>) -> Self {
        assert!(rows.is_primitive());
        Self {
            rows,
            inverses: Vec::new(),
            spare: Vec::new(),
        }
    }
    #[cfg(test)]
    pub(crate) fn active_inverse_count(&self) -> usize {
        self.inverses.iter().map(Vec::len).sum()
    }
    fn paged(&self) -> &Paged<K, V> {
        let Storage::Paged(rows) = &self.rows.storage else {
            unreachable!("primitive bitmap buckets")
        };
        rows
    }
    fn paged_mut(&mut self) -> &mut Paged<K, V> {
        let Storage::Paged(rows) = &mut self.rows.storage else {
            unreachable!("primitive bitmap buckets")
        };
        rows
    }
    fn dirty(&self) -> Vec<(Position, usize)> {
        let mut owners: Vec<_> = self
            .inverses
            .iter()
            .flatten()
            .map(|edit| (edit.pos, edit.owner))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }
    fn observed(&self, pos: Position, dirty: &[(Position, usize)]) -> Option<Entry<K, V>> {
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
    pub(crate) fn shared_entries(&self) -> Vec<(Arc<K>, Arc<V>)> {
        let dirty = self.dirty();
        self.paged()
            .pages
            .iter()
            .flat_map(|(page, rows)| {
                rows.cells
                    .iter()
                    .enumerate()
                    .filter_map(move |(cell, row)| row.as_ref().map(|_| Position { page, cell }))
            })
            .map(|pos| {
                let entry = self.observed(pos, &dirty).unwrap();
                (entry.key, entry.value)
            })
            .collect()
    }
    pub(crate) fn materialized(&self) -> &SnapshotMap<K, V> {
        self.paged()
            .cache
            .get_or_init(|| SnapshotMap::from_shared_sorted(self.shared_entries()))
    }
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
    }
    pub(crate) fn begin_journal(&mut self) -> usize {
        let token = self.rows.begin_journal();
        assert_eq!(token, self.inverses.len(), "bitmap inverse journal order");
        self.inverses.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(crate) fn commit_journal(&mut self, token: usize) {
        assert_eq!(
            self.inverses.len(),
            token + 1,
            "bitmap inverse journal order"
        );
        let offset = token
            .checked_sub(1)
            .map_or(0, |parent| self.paged().frames[parent].len());
        let mut frame = self.inverses.pop().unwrap();
        if let Some(parent) = self.inverses.last_mut() {
            for mut inverse in frame.drain(..) {
                inverse.normal += offset;
                parent.push(inverse);
            }
        }
        frame.clear();
        self.spare.push(frame);
        self.rows.commit_journal(token);
    }
    fn undo(rows: &mut Paged<K, V>, inverse: Inverse) {
        let row = rows
            .pages
            .get_mut(&inverse.pos.page)
            .expect("inverse bitmap page")
            .cells[inverse.pos.cell]
            .as_mut()
            .expect("inverse bitmap cell");
        Arc::make_mut(&mut row.value).assign(inverse.id, inverse.present);
    }
    pub(crate) fn rollback_journal(&mut self, token: usize) {
        assert_eq!(
            self.inverses.len(),
            token + 1,
            "bitmap inverse journal order"
        );
        let mut frame = self.inverses.pop().unwrap();
        let rows = self.paged_mut();
        rows.cache.take();
        for inverse in frame.drain(..).rev() {
            while rows.frames[token].len() > inverse.normal {
                let undo = rows.frames[token].pop().unwrap();
                rows.apply_undo(undo);
            }
            Self::undo(rows, inverse);
        }
        self.spare.push(frame);
        self.rows.rollback_journal(token);
    }
    pub(crate) fn capture_current(&self, token: usize) -> RowsCapture<K, V> {
        assert!(token < self.inverses.len(), "active bitmap inverse token");
        let mut positions: Vec<_> = self.paged().frames[token..]
            .iter()
            .flatten()
            .map(|undo| match undo {
                Undo::Value(pos, _) | Undo::Entry(pos, _) => *pos,
            })
            .chain(self.inverses[token..].iter().flatten().map(|edit| edit.pos))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        let dirty = self.dirty();
        RowsCapture::Delta(DeltaRows {
            entries: positions
                .into_iter()
                .map(|pos| (pos, self.observed(pos, &dirty)))
                .collect(),
        })
    }
    pub(crate) fn install_capture(&mut self, capture: RowsCapture<K, V>) {
        self.rows.install_capture(capture);
    }
    pub(crate) fn install_snapshot(&mut self, saved: Self) {
        self.rows.install_snapshot(saved.rows);
    }
    pub(crate) fn install_rows_snapshot(&mut self, saved: PrimitiveRows<K, V>) {
        self.rows.install_snapshot(saved);
    }
}
impl<K, V: BitValue> Clone for BitBuckets<K, V> {
    fn clone(&self) -> Self {
        let mut rows = self.rows.clone();
        let dirty = self.dirty();
        let Storage::Paged(copy) = &mut rows.storage else {
            unreachable!()
        };
        let mut positions: Vec<_> = dirty.iter().map(|(pos, _)| *pos).collect();
        positions.dedup();
        for pos in positions {
            if let Some(entry) = self.observed(pos, &dirty) {
                drop(copy.replace_value(pos, entry.value));
            }
        }
        Self {
            rows,
            inverses: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<K: Ord, V: BitValue> BitBuckets<K, V> {
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.rows.get(key)
    }
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.rows.insert(key, value);
    }
    pub(crate) fn remove(&mut self, key: &K) {
        self.rows.remove(key);
    }
    pub(crate) fn try_edit(&mut self, key: &K, id: usize, present: bool) -> bool {
        if self.inverses.is_empty() {
            return false;
        }
        let Storage::Paged(rows) = &mut self.rows.storage else {
            unreachable!()
        };
        let pos = (rows.map)(key);
        let Some(row) = rows
            .pages
            .get_mut(&pos.page)
            .and_then(|page| page.cells[pos.cell].as_mut())
        else {
            return false;
        };
        let old = row.value.bit(id);
        // Preserve ordinary key/value retirement and canonical rebirth for empty buckets.
        if !present && old && row.value.count() == 1 {
            return false;
        }
        let Some(value) = Arc::get_mut(&mut row.value) else {
            return false;
        };
        let owner = value as *mut V as usize;
        let frame = self.inverses.last_mut().unwrap();
        frame.reserve(1);
        rows.cache.take();
        value.assign(id, present);
        frame.push(Inverse {
            pos,
            normal: rows.frames.last().unwrap().len(),
            owner,
            id,
            present: old,
        });
        true
    }
}
