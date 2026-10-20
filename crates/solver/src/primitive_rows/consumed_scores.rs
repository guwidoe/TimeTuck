//! Copy-certified contribution rows for an ownership-consuming search only.
//! No public row view or escaped owner is permitted at this binding boundary.
//! Ordinary PrimitiveRows and all of their Arc ownership rules remain unchanged.
use super::{Mapper, PageKey, Position, mapper};
use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet},
};

const CELLS: usize = 16;
const HOT_ROWS: usize = 4096;
const HOT_COLUMNS: usize = 64;
type Entry<I, S> = (I, S);
type Cells<I, S> = [Option<Entry<I, S>>; CELLS];
type Undo<I, S> = Vec<(Position, Option<Entry<I, S>>)>;
struct Page<I, S> {
    cells: Cells<I, S>,
    occupied: u16,
}
impl<I, S> Page<I, S> {
    fn empty() -> Self {
        Self {
            cells: std::array::from_fn(|_| None),
            occupied: 0,
        }
    }
}
type Row<I, S> = Vec<Option<Box<Page<I, S>>>>;
pub(crate) struct Rows<I, S> {
    dense: Vec<Row<I, S>>,
    tree: BTreeMap<PageKey, Box<Page<I, S>>>,
    map: Mapper<I>,
    copy_id: fn(&I) -> I,
    copy_score: fn(&S) -> S,
    len: usize,
    frames: Vec<Undo<I, S>>,
    spare: Vec<Undo<I, S>>,
    empty: Vec<PageKey>,
}
pub(crate) struct Capture<I, S>(Undo<I, S>);
pub(crate) struct Prepared<'a, I, S> {
    rows: &'a mut Rows<I, S>,
    pos: Position,
}

pub(crate) fn builtin_score<S: 'static>() -> bool {
    let id = TypeId::of::<S>();
    id == TypeId::of::<crate::SimpleScore>()
        || id == TypeId::of::<crate::HardSoftScore>()
        || id == TypeId::of::<crate::HardMediumSoftScore>()
}
fn hot(PageKey([a, b, p, q]): PageKey) -> Option<(usize, usize)> {
    (a | b | (p & !(HOT_ROWS - 1)) | (q & !(HOT_COLUMNS - 1)) == 0).then_some((p, q))
}
impl<I: Clone + 'static, S: Clone + 'static> Rows<I, S> {
    pub(crate) fn new() -> Option<Self> {
        if !builtin_score::<S>() {
            return None;
        }
        Some(Self {
            dense: Vec::new(),
            tree: BTreeMap::new(),
            map: mapper::<I>()?,
            copy_id: Clone::clone,
            copy_score: Clone::clone,
            len: 0,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
        })
    }
}
impl<I, S> Rows<I, S> {
    fn copy(&self, entry: &Entry<I, S>) -> Entry<I, S> {
        ((self.copy_id)(&entry.0), (self.copy_score)(&entry.1))
    }
    fn page(&self, key: PageKey) -> Option<&Page<I, S>> {
        match hot(key) {
            Some((p, q)) => self.dense.get(p)?.get(q)?.as_deref(),
            None => self.tree.get(&key).map(Box::as_ref),
        }
    }
    fn page_mut(&mut self, key: PageKey, create: bool) -> Option<&mut Page<I, S>> {
        match hot(key) {
            Some((p, q)) => {
                if !create
                    && self
                        .dense
                        .get(p)
                        .and_then(|r| r.get(q))
                        .and_then(Option::as_ref)
                        .is_none()
                {
                    return None;
                }
                if self.dense.len() <= p {
                    self.dense.resize_with(p + 1, Vec::new);
                }
                let row = &mut self.dense[p];
                if row.len() <= q {
                    row.resize_with(q + 1, || None);
                }
                if create {
                    Some(
                        row[q]
                            .get_or_insert_with(|| Box::new(Page::empty()))
                            .as_mut(),
                    )
                } else {
                    row[q].as_deref_mut()
                }
            }
            None => {
                if create {
                    Some(
                        self.tree
                            .entry(key)
                            .or_insert_with(|| Box::new(Page::empty()))
                            .as_mut(),
                    )
                } else {
                    self.tree.get_mut(&key).map(Box::as_mut)
                }
            }
        }
    }
    fn get_at(&self, pos: Position) -> Option<&Entry<I, S>> {
        self.page(pos.page)?.cells[pos.cell].as_ref()
    }
    fn set(&mut self, pos: Position, value: Option<Entry<I, S>>) -> Option<Entry<I, S>> {
        let page = self.page_mut(pos.page, value.is_some())?;
        let old = std::mem::replace(&mut page.cells[pos.cell], value);
        if page.cells[pos.cell].is_some() {
            page.occupied |= 1 << pos.cell;
        } else {
            page.occupied &= !(1 << pos.cell);
        }
        let now_empty = page.occupied == 0;
        let present = page.cells[pos.cell].is_some();
        match (old.is_some(), present) {
            (false, true) => self.len += 1,
            (true, false) => self.len -= 1,
            _ => {}
        }
        if now_empty {
            self.empty.push(pos.page);
        }
        old
    }
    fn replace(&mut self, pos: Position, value: Option<Entry<I, S>>) {
        let old = self.set(pos, value);
        if let Some(frame) = self.frames.last_mut() {
            frame.push((pos, old));
        } else {
            self.prune();
        }
    }
    fn prune(&mut self) {
        for key in self.empty.drain(..) {
            match hot(key) {
                Some((p, q)) => {
                    if let Some(cell) = self.dense.get_mut(p).and_then(|r| r.get_mut(q))
                        && cell.as_ref().is_some_and(|p| p.occupied == 0)
                    {
                        *cell = None;
                    }
                }
                None => {
                    if self.tree.get(&key).is_some_and(|p| p.occupied == 0) {
                        self.tree.remove(&key);
                    }
                }
            }
        }
    }
    fn positions(&self) -> Vec<Position> {
        let mut positions = Vec::with_capacity(self.len);
        for (p, row) in self.dense.iter().enumerate() {
            for (q, page) in row.iter().enumerate() {
                if let Some(page) = page {
                    for cell in 0..CELLS {
                        if page.occupied & (1 << cell) != 0 {
                            positions.push(Position {
                                page: PageKey([0, 0, p, q]),
                                cell,
                            });
                        }
                    }
                }
            }
        }
        for (key, page) in &self.tree {
            for cell in 0..CELLS {
                if page.occupied & (1 << cell) != 0 {
                    positions.push(Position { page: *key, cell });
                }
            }
        }
        positions.sort_unstable();
        positions
    }
    pub(crate) fn len(&self) -> usize {
        self.len
    }
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&I, &S)> {
        self.positions().into_iter().map(|pos| {
            let row = self.get_at(pos).expect("live contribution");
            (&row.0, &row.1)
        })
    }
    pub(crate) fn prepare_change<R>(
        &mut self,
        id: &I,
        inspect: impl FnOnce(Option<&S>) -> R,
    ) -> (Prepared<'_, I, S>, R) {
        let pos = (self.map)(id);
        let old = inspect(self.get_at(pos).map(|row| &row.1));
        (Prepared { rows: self, pos }, old)
    }
    pub(crate) fn clear(&mut self) {
        for pos in self.positions() {
            self.replace(pos, None);
        }
    }
    pub(crate) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(crate) fn commit(&mut self, token: usize) {
        assert_eq!(
            self.frames.len(),
            token + 1,
            "consumed contribution journal order"
        );
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut frame);
            } else {
                parent.append(&mut frame);
            }
        }
        frame.clear();
        self.spare.push(frame);
        if self.frames.is_empty() {
            self.prune();
        }
    }
    pub(crate) fn rollback(&mut self, token: usize) {
        assert_eq!(
            self.frames.len(),
            token + 1,
            "consumed contribution journal order"
        );
        let mut frame = self.frames.pop().unwrap();
        while let Some((pos, old)) = frame.pop() {
            self.set(pos, old);
        }
        self.spare.push(frame);
        self.prune();
    }
    pub(crate) fn capture(&self, token: usize) -> Capture<I, S> {
        assert!(token < self.frames.len(), "consumed contribution journal");
        let touched: BTreeSet<_> = self.frames[token..]
            .iter()
            .flatten()
            .map(|(pos, _)| *pos)
            .collect();
        Capture(
            touched
                .into_iter()
                .map(|pos| (pos, self.get_at(pos).map(|v| self.copy(v))))
                .collect(),
        )
    }
    pub(crate) fn install_capture(&mut self, saved: Capture<I, S>) {
        for (pos, value) in saved.0 {
            self.replace(pos, value);
        }
    }
    pub(crate) fn install(&mut self, saved: Self) {
        let touched: BTreeSet<_> = self
            .positions()
            .into_iter()
            .chain(saved.positions())
            .collect();
        for pos in touched {
            self.replace(pos, saved.get_at(pos).map(|v| saved.copy(v)));
        }
    }
    pub(crate) fn journal_snapshot(&self, token: usize) -> Self {
        assert!(token < self.frames.len(), "consumed contribution journal");
        let mut snapshot = self.clone();
        for frame in self.frames[token..].iter().rev() {
            for (pos, old) in frame.iter().rev() {
                snapshot.set(*pos, old.as_ref().map(|v| self.copy(v)));
            }
        }
        snapshot.prune();
        snapshot
    }
}
impl<I, S> Clone for Rows<I, S> {
    fn clone(&self) -> Self {
        let mut snapshot = Self {
            dense: Vec::new(),
            tree: BTreeMap::new(),
            map: self.map,
            copy_id: self.copy_id,
            copy_score: self.copy_score,
            len: 0,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
        };
        for pos in self.positions() {
            snapshot.set(pos, Some(self.copy(self.get_at(pos).unwrap())));
        }
        snapshot
    }
}
impl<I, S> Prepared<'_, I, S> {
    pub(crate) fn insert(self, id: I, value: S) {
        self.rows.replace(self.pos, Some((id, value)));
    }
    pub(crate) fn remove(self) {
        self.rows.replace(self.pos, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    fn put(rows: &mut Rows<(usize, usize), SimpleScore>, id: (usize, usize), score: i64) {
        let (prepared, _) = rows.prepare_change(&id, |old| old.cloned());
        prepared.insert(id, SimpleScore::new(score));
    }
    fn remove(rows: &mut Rows<(usize, usize), SimpleScore>, id: (usize, usize)) {
        let (prepared, _) = rows.prepare_change(&id, |old| old.cloned());
        prepared.remove();
    }
    fn values(rows: &Rows<(usize, usize), SimpleScore>) -> Vec<((usize, usize), SimpleScore)> {
        rows.iter().map(|(id, s)| (*id, *s)).collect()
    }
    #[test]
    fn consuming_scores_nested_empty_page_reinsert_and_each_preimage_restore_all_routes() {
        let mut rows = Rows::new().unwrap();
        let ids = [
            (0usize, 0usize),
            (0, 15),
            (0, 16),
            (4095, 1023),
            (4096, 0),
            (usize::MAX, usize::MAX),
        ];
        for (i, id) in ids.into_iter().enumerate() {
            put(&mut rows, id, i as i64);
        }
        let base = values(&rows);
        let outer = rows.begin();
        for id in ids {
            remove(&mut rows, id);
        }
        assert_eq!(rows.len(), 0);
        let inner = rows.begin();
        for id in ids {
            put(&mut rows, id, 7);
            put(&mut rows, id, 8);
            remove(&mut rows, id);
            put(&mut rows, id, 9);
        }
        let wanted = values(&rows);
        let capture = rows.capture(outer);
        let full = rows.clone();
        assert_eq!(values(&rows.journal_snapshot(outer)), base);
        assert_eq!(rows.journal_snapshot(inner).len(), 0);
        rows.commit(inner);
        rows.rollback(outer);
        assert_eq!(values(&rows), base);
        let install = rows.begin();
        rows.install_capture(capture);
        assert_eq!(values(&rows), wanted);
        rows.clear();
        assert_eq!(rows.len(), 0);
        rows.install(full);
        assert_eq!(values(&rows), wanted);
        rows.rollback(install);
        assert_eq!(values(&rows), base);
        // Sparse dead candidate pages are released instead of accumulating.
        for page in 0..20usize {
            let token = rows.begin();
            put(&mut rows, (usize::MAX, page * 16), 7);
            rows.rollback(token);
            assert_eq!(values(&rows), base);
            assert_eq!(rows.tree.len(), 2);
        }
    }
    #[test]
    fn consuming_scores_saved_removed_and_added_ids_stay_independent_of_later_trials() {
        let mut rows = Rows::new().unwrap();
        put(&mut rows, (0, 0), 1);
        let token = rows.begin();
        remove(&mut rows, (0, 0));
        put(&mut rows, (usize::MAX, usize::MAX), 2);
        let capture = rows.capture(token);
        rows.rollback(token);
        let alternate = rows.begin();
        put(&mut rows, (0, 0), 99);
        rows.rollback(alternate);
        let install = rows.begin();
        rows.install_capture(capture);
        assert_eq!(
            values(&rows),
            vec![((usize::MAX, usize::MAX), SimpleScore::new(2))]
        );
        rows.rollback(install);
        assert_eq!(values(&rows), vec![((0, 0), SimpleScore::new(1))]);
    }
}
