//! Graph-private accepted IDs. Exact numeric identities are implicit in one bit
//! per final-coordinate cell; no key or payload owners are retained here.
use super::{CELLS, Mapper, PageKey, Position, mapper};
use std::collections::{BTreeMap, btree_map};
const ROWS: usize = 4096;
const COLUMNS: usize = 64;
fn hot(PageKey([a, b, p, q]): PageKey) -> Option<(usize, usize)> {
    ((a | b | (p & !(ROWS - 1)) | (q & !(COLUMNS - 1))) == 0).then_some((p, q))
}
type Frame = Vec<(PageKey, u16)>;
pub(crate) struct Capture {
    entries: Frame,
}
pub(crate) struct Membership<I> {
    rows: Vec<Vec<u16>>,
    tree: BTreeMap<PageKey, u16>,
    map: Mapper<I>,
    len: usize,
    frames: Vec<Frame>,
    spare: Vec<Frame>,
}
impl<I> Clone for Membership<I> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            tree: self.tree.clone(),
            map: self.map,
            len: self.len,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
enum Entry<'a> {
    Dense {
        rows: &'a mut Vec<Vec<u16>>,
        p: usize,
        q: usize,
    },
    Tree(btree_map::Entry<'a, PageKey, u16>),
}
impl Entry<'_> {
    fn get(&self) -> u16 {
        match self {
            Self::Dense { rows, p, q } => rows
                .get(*p)
                .and_then(|row| row.get(*q))
                .copied()
                .unwrap_or(0),
            Self::Tree(btree_map::Entry::Occupied(entry)) => *entry.get(),
            Self::Tree(btree_map::Entry::Vacant(_)) => 0,
        }
    }
    fn set(self, bits: u16) {
        match self {
            Self::Tree(entry) => {
                if bits != 0 {
                    *entry.or_insert(0) = bits;
                } else if let btree_map::Entry::Occupied(entry) = entry {
                    entry.remove();
                }
            }
            Self::Dense { rows, p, q } => {
                if bits != 0 {
                    if rows.len() <= p {
                        rows.resize_with(p + 1, Vec::new);
                    }
                    let row = &mut rows[p];
                    if row.len() <= q {
                        row.resize(q + 1, 0);
                    }
                    row[q] = bits;
                } else if let Some(row) = rows.get_mut(p) {
                    if let Some(cell) = row.get_mut(q) {
                        *cell = 0;
                    }
                    while row.last() == Some(&0) {
                        row.pop();
                    }
                    while rows.last().is_some_and(Vec::is_empty) {
                        rows.pop();
                    }
                }
            }
        }
    }
}
pub(crate) struct Prepared<'a> {
    entry: Entry<'a>,
    pos: Position,
    old: u16,
    len: &'a mut usize,
    frames: &'a mut Vec<Frame>,
}
impl Prepared<'_> {
    pub(crate) fn set(self, accepted: bool) {
        let mask = 1 << self.pos.cell;
        let next = if accepted {
            self.old | mask
        } else {
            self.old & !mask
        };
        if next == self.old {
            return;
        }
        self.entry.set(next);
        if accepted {
            *self.len += 1;
        } else {
            *self.len -= 1;
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.push((self.pos.page, self.old));
        }
    }
}
impl<I: 'static> Membership<I> {
    pub(crate) fn new() -> Option<Self> {
        Some(Self {
            rows: Vec::new(),
            tree: BTreeMap::new(),
            map: mapper::<I>()?,
            len: 0,
            frames: Vec::new(),
            spare: Vec::new(),
        })
    }
}
impl<I> Membership<I> {
    fn entry(&mut self, key: PageKey) -> Entry<'_> {
        match hot(key) {
            Some((p, q)) => Entry::Dense {
                rows: &mut self.rows,
                p,
                q,
            },
            None => Entry::Tree(self.tree.entry(key)),
        }
    }
    fn page(&self, key: PageKey) -> u16 {
        match hot(key) {
            Some((p, q)) => self
                .rows
                .get(p)
                .and_then(|row| row.get(q))
                .copied()
                .unwrap_or(0),
            None => self.tree.get(&key).copied().unwrap_or(0),
        }
    }
    fn pages(&self) -> impl Iterator<Item = (PageKey, u16)> {
        self.rows
            .iter()
            .enumerate()
            .flat_map(|(p, row)| {
                row.iter().enumerate().filter_map(move |(q, &bits)| {
                    (bits != 0).then_some((PageKey([0, 0, p, q]), bits))
                })
            })
            .chain(self.tree.iter().map(|(key, bits)| (*key, *bits)))
    }
    pub(crate) fn prepare(&mut self, id: &I) -> (Prepared<'_>, bool) {
        let pos = (self.map)(id);
        let entry = match hot(pos.page) {
            Some((p, q)) => Entry::Dense {
                rows: &mut self.rows,
                p,
                q,
            },
            None => Entry::Tree(self.tree.entry(pos.page)),
        };
        let old = entry.get();
        let accepted = old & (1 << pos.cell) != 0;
        (
            Prepared {
                entry,
                pos,
                old,
                len: &mut self.len,
                frames: &mut self.frames,
            },
            accepted,
        )
    }
    fn set_page(&mut self, key: PageKey, next: u16, journal: bool) {
        let entry = self.entry(key);
        let old = entry.get();
        if old == next {
            return;
        }
        entry.set(next);
        self.len = self.len - old.count_ones() as usize + next.count_ones() as usize;
        if journal && let Some(frame) = self.frames.last_mut() {
            frame.push((key, old));
        }
    }
    pub(crate) fn clear(&mut self) {
        if self.frames.is_empty() {
            self.rows.clear();
            self.tree.clear();
            self.len = 0;
        } else {
            let keys: Vec<_> = self.pages().map(|(key, _)| key).collect();
            for key in keys {
                self.set_page(key, 0, true);
            }
        }
    }
    pub(crate) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(crate) fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "primitive row journal order");
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            parent.append(&mut frame);
        }
        frame.clear();
        self.spare.push(frame);
    }
    pub(crate) fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "primitive row journal order");
        let mut frame = self.frames.pop().unwrap();
        for (key, bits) in frame.drain(..).rev() {
            self.set_page(key, bits, false);
        }
        self.spare.push(frame);
    }
    pub(crate) fn capture(&self, token: usize) -> Capture {
        assert!(
            token < self.frames.len(),
            "active primitive capture journal token"
        );
        let mut entries: Frame = self.frames[token..]
            .iter()
            .flat_map(|frame| frame.iter().map(|(key, _)| (*key, 0)))
            .collect();
        entries.sort_unstable_by_key(|(key, _)| *key);
        entries.dedup_by_key(|(key, _)| *key);
        for (key, bits) in &mut entries {
            *bits = self.page(*key);
        }
        Capture { entries }
    }
    pub(crate) fn install_capture(&mut self, capture: Capture) {
        for (key, bits) in capture.entries {
            self.set_page(key, bits, true);
        }
    }
    pub(crate) fn install(&mut self, snapshot: Self) {
        if self.frames.is_empty() {
            *self = snapshot;
            return;
        }
        let mut keys: Vec<_> = self
            .pages()
            .chain(snapshot.pages())
            .map(|(key, _)| key)
            .collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            self.set_page(key, snapshot.page(key), true);
        }
    }
    fn positions(&self) -> impl Iterator<Item = [usize; 4]> {
        self.pages().flat_map(|(PageKey([a, b, c, q]), bits)| {
            (0..CELLS).filter_map(move |cell| {
                (bits & (1 << cell) != 0).then_some([a, b, c, q * CELLS + cell])
            })
        })
    }
    pub(crate) fn complete_fixed(&self, count: usize) -> bool {
        self.len == count
            && self
                .positions()
                .all(|[a, b, c, d]| a == 0 && b == 0 && c == 0 && d < count)
    }
    pub(crate) fn fits_pair(&self, left: usize, right: usize) -> bool {
        self.positions()
            .all(|[a, b, c, d]| a == 0 && b == 0 && c < left && d < right)
    }
}

#[cfg(test)]
mod tests;
