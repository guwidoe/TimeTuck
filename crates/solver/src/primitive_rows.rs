//! Private ordered state with an explicit primitive-key storage binding.
//! Generic/borrowed keys retain SnapshotMap. Primitive pages use exact numeric
//! prefixes and sixteen final-coordinate cells, including usize::MAX. User
//! key/value owners are shared, never cloned by snapshots or journal restores.
pub(crate) mod consumed_scores;
mod directory;
pub(crate) mod group_collectors;
pub(crate) mod membership;
#[cfg(test)]
mod page_box_tests;
pub(crate) mod pool;
use crate::SnapshotMap;
use directory::Directory;
use pool::Pools;
#[cfg(test)]
use std::collections::BTreeMap;
use std::{
    any::{Any, TypeId},
    fmt,
    ops::Index,
    sync::{Arc, OnceLock},
};

const CELLS: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PageKey([usize; 4]);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Position {
    page: PageKey,
    cell: usize,
}
fn position(parts: [usize; 4]) -> Position {
    Position {
        page: PageKey([parts[0], parts[1], parts[2], parts[3] / CELLS]),
        cell: parts[3] % CELLS,
    }
}
type Mapper<K> = fn(&K) -> Position;

// Lookup policy is a compile-time parameter of private graph apply helpers,
// never stored in a row/node. Borrowed public operators retain Stored lookup.
pub(crate) trait LookupPolicy<K> {
    fn locate(map: fn(&K) -> Position, key: &K) -> Position;
}
pub(crate) struct Stored;
pub(crate) struct Exact;
impl<K> LookupPolicy<K> for Stored {
    #[inline]
    fn locate(map: fn(&K) -> Position, key: &K) -> Position {
        map(key)
    }
}
impl<K: 'static> LookupPolicy<K> for Exact {
    #[inline]
    fn locate(_map: fn(&K) -> Position, key: &K) -> Position {
        mapper::<K>().expect("certified primitive group key")(key)
    }
}
struct Entry<K, V> {
    key: Arc<K>,
    value: Arc<V>,
}
impl<K, V> Clone for Entry<K, V> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
        }
    }
}
struct Page<K, V> {
    cells: [Option<Entry<K, V>>; CELLS],
    occupied: u16,
}
impl<K, V> Page<K, V> {
    fn empty() -> Self {
        Self {
            cells: std::array::from_fn(|_| None),
            occupied: 0,
        }
    }
}
impl<K, V> Clone for Page<K, V> {
    fn clone(&self) -> Self {
        Self {
            cells: self.cells.clone(),
            occupied: self.occupied,
        }
    }
}
enum Undo<K, V> {
    Value(Position, Arc<V>),
    Entry(Position, Option<Entry<K, V>>),
}
type Frame<K, V> = Vec<Undo<K, V>>;
struct Paged<K, V> {
    pages: Directory<K, V>,
    map: Mapper<K>,
    len: usize,
    frames: Vec<Frame<K, V>>,
    spare: Vec<Frame<K, V>>,
    empty: Vec<PageKey>,
    cache: OnceLock<SnapshotMap<K, V>>,
    pools: Box<Pools<K, V>>,
}
impl<K, V> Clone for Paged<K, V> {
    fn clone(&self) -> Self {
        Self {
            pages: self.pages.clone(),
            map: self.map,
            len: self.len,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
            cache: OnceLock::new(),
            pools: Box::new(self.pools.empty_clone()),
        }
    }
}
enum Storage<K, V> {
    Tree(SnapshotMap<K, V>),
    Paged(Paged<K, V>),
}
pub(crate) struct PrimitiveRows<K, V> {
    storage: Storage<K, V>,
}

/// One inspected entry, exclusively borrowed until publication or abandonment.
/// Preparing acquires no row owners and performs no user Clone/Drop operations.
pub(crate) struct PreparedChange<'a, K, V> {
    inner: PreparedKind<'a, K, V>,
}
enum PreparedKind<'a, K, V> {
    Tree(crate::snapshot_map::PreparedChange<'a, K, V>),
    Paged(PreparedPage<'a, K, V>),
}
// Keep the located directory entry through fallible output construction. All
// other mutable fields are borrowed separately; no page or row owner changes
// until commit, and commit never repeats the directory lookup.
struct PreparedPage<'a, K, V> {
    entry: directory::Entry<'a, K, V>,
    pos: Position,
    found: bool,
    len: &'a mut usize,
    frames: &'a mut Vec<Frame<K, V>>,
    empty: &'a mut Vec<PageKey>,
    cache: &'a mut OnceLock<SnapshotMap<K, V>>,
    pools: &'a mut Pools<K, V>,
}
impl<K, V> PreparedPage<'_, K, V> {
    fn insert(self, key: K, value: V) {
        self.cache.take();
        if self.found {
            drop(key);
            let value = self.pools.value(value);
            let page = self
                .entry
                .or_insert_with(|| unreachable!("prepared occupied page"));
            let old = std::mem::replace(
                &mut page.as_mut().cells[self.pos.cell]
                    .as_mut()
                    .expect("prepared occupied cell")
                    .value,
                value,
            );
            let undo = Undo::Value(self.pos, old);
            match self.frames.last_mut() {
                Some(frame) => frame.push(undo),
                None => self.pools.retire_undo(undo),
            }
        } else {
            let row = Entry {
                key: self.pools.key(key),
                value: self.pools.value(value),
            };
            let page = self.entry.or_insert_with(|| Box::new(Page::empty()));
            let page = page.as_mut();
            page.cells[self.pos.cell] = Some(row);
            page.occupied |= 1 << self.pos.cell;
            *self.len += 1;
            if let Some(frame) = self.frames.last_mut() {
                frame.push(Undo::Entry(self.pos, None));
            }
        }
    }
    fn remove(self) -> bool {
        if !self.found {
            return false;
        }
        self.cache.take();
        let mut entry = self.entry;
        let page = entry.get_mut().expect("prepared occupied page").as_mut();
        let old = page.cells[self.pos.cell].take();
        page.occupied &= !(1 << self.pos.cell);
        *self.len -= 1;
        let empty = page.occupied == 0;
        let undo = Undo::Entry(self.pos, old);
        match self.frames.last_mut() {
            Some(frame) => frame.push(undo),
            None => self.pools.retire_undo(undo),
        }
        if empty {
            if self.frames.is_empty() {
                // Outside a journal the entry can be removed directly. There
                // are no previously deferred empty pages after an outer finish.
                entry.remove();
            } else {
                self.empty.push(self.pos.page);
            }
        }
        true
    }
}
impl<K, V> PreparedChange<'_, K, V> {
    pub(crate) fn insert(self, key: K, value: V) {
        match self.inner {
            PreparedKind::Tree(prepared) => prepared.insert(key, value),
            PreparedKind::Paged(prepared) => prepared.insert(key, value),
        }
    }
    pub(crate) fn remove(self) {
        match self.inner {
            PreparedKind::Tree(prepared) => prepared.remove(),
            PreparedKind::Paged(prepared) => {
                prepared.remove();
            }
        }
    }
}

/// Current candidate state captured relative to an active journal's entry state.
/// A Delta must be installed over that same entry state. It owns only final
/// touched-cell entries; it never owns a page or the primitive page directory.
pub(crate) enum RowsCapture<K, V> {
    Full(PrimitiveRows<K, V>),
    Delta(DeltaRows<K, V>),
}
pub(crate) struct DeltaRows<K, V> {
    entries: Vec<(Position, Option<Entry<K, V>>)>,
}
impl<K, V> Clone for RowsCapture<K, V> {
    fn clone(&self) -> Self {
        match self {
            Self::Full(rows) => Self::Full(rows.clone()),
            Self::Delta(rows) => Self::Delta(DeltaRows {
                entries: rows.entries.clone(),
            }),
        }
    }
}
impl<K, V> Clone for PrimitiveRows<K, V> {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Tree(rows) => Storage::Tree(rows.clone()),
                Storage::Paged(rows) => Storage::Paged(rows.clone()),
            },
        }
    }
}

impl<K, V> Paged<K, V> {
    fn new(map: Mapper<K>) -> Self {
        Self {
            pages: Directory::new(),
            map,
            len: 0,
            frames: Vec::new(),
            spare: Vec::new(),
            empty: Vec::new(),
            cache: OnceLock::new(),
            pools: Box::new(Pools::new()),
        }
    }
    // Keep directory routing and cell inspection in the caller's numeric read
    // path, avoiding a separate large Position argument/call on every lookup.
    #[inline(always)]
    fn entry(&self, pos: Position) -> Option<&Entry<K, V>> {
        self.pages.get(&pos.page)?.cells[pos.cell].as_ref()
    }
    fn replace_value(&mut self, pos: Position, value: Arc<V>) -> Arc<V> {
        let page = self
            .pages
            .get_mut(&pos.page)
            .expect("primitive replacement page")
            .as_mut();
        std::mem::replace(
            &mut page.cells[pos.cell]
                .as_mut()
                .expect("primitive replacement cell")
                .value,
            value,
        )
    }
    fn set(&mut self, pos: Position, value: Option<Entry<K, V>>) -> Option<Entry<K, V>> {
        let page = if value.is_none() {
            self.pages.get_mut(&pos.page)?
        } else {
            self.pages
                .entry(pos.page)
                .or_insert_with(|| Box::new(Page::empty()))
        };
        let page = page.as_mut();
        let next = value.is_some();
        let old = std::mem::replace(&mut page.cells[pos.cell], value);
        if next {
            page.occupied |= 1 << pos.cell;
        } else {
            page.occupied &= !(1 << pos.cell);
        }
        match (old.is_some(), next) {
            (false, true) => self.len += 1,
            (true, false) => self.len -= 1,
            _ => {}
        }
        if page.occupied == 0 {
            self.empty.push(pos.page);
        }
        old
    }
    fn prepare<R>(
        &mut self,
        pos: Position,
        inspect: impl FnOnce(Option<&V>) -> R,
    ) -> (PreparedPage<'_, K, V>, R) {
        let entry = self.pages.entry(pos.page);
        let row = entry.get().and_then(|page| page.cells[pos.cell].as_ref());
        let found = row.is_some();
        let observed = inspect(row.map(|row| row.value.as_ref()));
        (
            PreparedPage {
                entry,
                pos,
                found,
                len: &mut self.len,
                frames: &mut self.frames,
                empty: &mut self.empty,
                cache: &mut self.cache,
                pools: &mut self.pools,
            },
            observed,
        )
    }
    fn record(&mut self, undo: Undo<K, V>) {
        if let Some(frame) = self.frames.last_mut() {
            frame.push(undo);
        } else {
            self.pools.retire_undo(undo);
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
    fn apply_undo(&mut self, undo: Undo<K, V>) {
        match undo {
            Undo::Value(pos, value) => {
                let retired = self.replace_value(pos, value);
                self.pools.retire_value(retired);
            }
            Undo::Entry(pos, value) => {
                let retired = self.set(pos, value);
                self.pools.retire_entry(retired);
            }
        }
    }
    fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "primitive row journal order");
        self.cache.take();
        let mut frame = self.frames.pop().unwrap();
        for undo in frame.drain(..).rev() {
            self.apply_undo(undo);
        }
        self.spare.push(frame);
        self.cleanup();
    }
    fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "primitive row journal order");
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut frame);
            } else {
                parent.append(&mut frame);
            }
        }
        for undo in frame.drain(..) {
            self.pools.retire_undo(undo);
        }
        self.spare.push(frame);
        self.cleanup();
    }
    fn snapshot(&self, token: usize) -> Self {
        assert!(token < self.frames.len(), "active primitive journal token");
        let mut saved = self.clone();
        for frame in self.frames[token..].iter().rev() {
            for undo in frame.iter().rev() {
                saved.apply_undo(match undo {
                    Undo::Value(pos, value) => Undo::Value(*pos, value.clone()),
                    Undo::Entry(pos, value) => Undo::Entry(*pos, value.clone()),
                });
            }
        }
        saved.cleanup();
        saved
    }
    fn install(&mut self, saved: Self) {
        self.cache.take();
        if self.frames.is_empty() {
            self.pages = saved.pages;
            self.len = saved.len;
            self.empty.clear();
            self.pages.retain(|_, page| page.occupied != 0);
            return;
        }
        let mut keys: Vec<_> = self.pages.keys().chain(saved.pages.keys()).collect();
        keys.sort_unstable();
        keys.dedup();
        for key in keys {
            for cell in 0..CELLS {
                let pos = Position { page: key, cell };
                let old = self.entry(pos);
                let new = saved.entry(pos);
                if old.is_none() && new.is_none() {
                    continue;
                }
                if old.zip(new).is_some_and(|(a, b)| {
                    Arc::ptr_eq(&a.key, &b.key) && Arc::ptr_eq(&a.value, &b.value)
                }) {
                    continue;
                }
                let next = new.cloned();
                let old = self.set(pos, next);
                self.record(Undo::Entry(pos, old));
            }
        }
    }
}

impl<K, V> PrimitiveRows<K, V> {
    pub(crate) fn new() -> Self {
        Self {
            storage: Storage::Tree(SnapshotMap::new()),
        }
    }
    // Transfer existing canonical owners before a private operator binds pages.
    pub(crate) fn from_snapshot_map(rows: SnapshotMap<K, V>) -> Self {
        Self {
            storage: Storage::Tree(rows),
        }
    }
    pub(crate) fn is_primitive(&self) -> bool {
        matches!(self.storage, Storage::Paged(_))
    }
    pub(crate) fn len(&self) -> usize {
        match &self.storage {
            Storage::Tree(rows) => rows.len(),
            Storage::Paged(rows) => rows.len,
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[cfg(test)]
    pub(crate) fn is_empty_for_tests(&self) -> bool {
        self.is_empty()
    }
    pub(crate) fn iter(&self) -> Iter<'_, K, V> {
        let inner = match &self.storage {
            Storage::Tree(rows) => IterKind::Tree(rows.iter()),
            Storage::Paged(rows) => IterKind::Paged {
                pages: rows.pages.values(),
                cells: [].iter(),
            },
        };
        Iter {
            inner,
            remaining: self.len(),
        }
    }
    #[cfg(test)]
    pub(crate) fn keys(&self) -> impl ExactSizeIterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }
    pub(crate) fn values(&self) -> impl ExactSizeIterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }
    /// Clone only shared ownership handles, in exact key order.
    pub(crate) fn shared_entries(&self) -> Vec<(Arc<K>, Arc<V>)> {
        match &self.storage {
            Storage::Tree(rows) => rows.shared_entries(),
            Storage::Paged(rows) => rows
                .pages
                .values()
                .flat_map(|page| page.cells.iter())
                .filter_map(Option::as_ref)
                .map(|entry| (entry.key.clone(), entry.value.clone()))
                .collect(),
        }
    }
    pub(crate) fn materialized(&self) -> &SnapshotMap<K, V> {
        match &self.storage {
            Storage::Tree(rows) => rows,
            Storage::Paged(rows) => rows.cache.get_or_init(|| {
                let entries = rows
                    .pages
                    .values()
                    .flat_map(|page| page.cells.iter())
                    .filter_map(Option::as_ref)
                    .map(|entry| (entry.key.clone(), entry.value.clone()))
                    .collect();
                SnapshotMap::from_shared_sorted(entries)
            }),
        }
    }
    pub(crate) fn begin_journal(&mut self) -> usize {
        match &mut self.storage {
            Storage::Tree(rows) => rows.begin_journal(),
            Storage::Paged(rows) => rows.begin(),
        }
    }
    pub(crate) fn commit_journal(&mut self, token: usize) {
        match &mut self.storage {
            Storage::Tree(rows) => rows.commit_journal(token),
            Storage::Paged(rows) => rows.commit(token),
        }
    }
    pub(crate) fn rollback_journal(&mut self, token: usize) {
        match &mut self.storage {
            Storage::Tree(rows) => rows.rollback_journal(token),
            Storage::Paged(rows) => rows.rollback(token),
        }
    }
    pub(crate) fn journal_snapshot(&self, token: usize) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Tree(rows) => Storage::Tree(rows.journal_snapshot(token)),
                Storage::Paged(rows) => Storage::Paged(rows.snapshot(token)),
            },
        }
    }
    /// Capture the current state of every cell touched since `token` began.
    /// Nested committed edits are included; nested rolled-back edits are absent.
    /// The caller restores that journal's entry state before installing a Delta.
    pub(crate) fn capture_current(&self, token: usize) -> RowsCapture<K, V> {
        let Storage::Paged(rows) = &self.storage else {
            return RowsCapture::Full(self.clone());
        };
        assert!(
            token < rows.frames.len(),
            "active primitive capture journal token"
        );
        // Deduplicate before acquiring owners, directly in the final buffer.
        // Sorting/discarding placeholders cannot clone or drop user payloads.
        let mut entries: Vec<_> = rows.frames[token..]
            .iter()
            .flat_map(|frame| {
                frame.iter().map(|undo| {
                    let pos = match undo {
                        Undo::Value(pos, _) | Undo::Entry(pos, _) => *pos,
                    };
                    (pos, None)
                })
            })
            .collect();
        entries.sort_unstable_by_key(|(pos, _)| *pos);
        entries.dedup_by_key(|(pos, _)| *pos);
        for (pos, entry) in &mut entries {
            *entry = rows.entry(*pos).cloned();
        }
        RowsCapture::Delta(DeltaRows { entries })
    }
    /// Install a candidate over its restored journal entry state. Primitive
    /// captures restore full captured representatives, including remove/reinsert
    /// identities, and preserve any enclosing journal for later rollback.
    pub(crate) fn install_capture(&mut self, capture: RowsCapture<K, V>) {
        let delta = match capture {
            RowsCapture::Full(rows) => {
                self.install_snapshot(rows);
                return;
            }
            RowsCapture::Delta(delta) => delta,
        };
        let Storage::Paged(rows) = &mut self.storage else {
            panic!("primitive capture requires primitive row storage");
        };
        rows.cache.take();
        for (pos, next) in delta.entries {
            let old = rows.entry(pos);
            if old.is_none() && next.is_none() {
                continue;
            }
            if old.zip(next.as_ref()).is_some_and(|(a, b)| {
                Arc::ptr_eq(&a.key, &b.key) && Arc::ptr_eq(&a.value, &b.value)
            }) {
                continue;
            }
            let old = rows.set(pos, next);
            rows.record(Undo::Entry(pos, old));
        }
        rows.cleanup();
    }
    pub(crate) fn install_snapshot(&mut self, saved: Self) {
        match (&mut self.storage, saved.storage) {
            (Storage::Tree(rows), Storage::Tree(saved)) => rows.install_snapshot(saved),
            (Storage::Paged(rows), Storage::Paged(saved)) => rows.install(saved),
            (Storage::Tree(rows), Storage::Paged(saved)) => {
                let saved = Self {
                    storage: Storage::Paged(saved),
                };
                rows.install_snapshot(saved.materialized().clone());
            }
            (Storage::Paged(rows), Storage::Tree(saved)) => {
                let mut next = Paged::new(rows.map);
                for (key, value) in saved.shared_entries() {
                    next.set((next.map)(&key), Some(Entry { key, value }));
                }
                rows.install(next);
            }
        }
    }
    pub(crate) fn clear(&mut self) {
        match &mut self.storage {
            Storage::Tree(rows) => rows.clear(),
            Storage::Paged(rows) => {
                rows.cache.take();
                if rows.frames.is_empty() {
                    rows.pages.clear();
                    rows.len = 0;
                    rows.empty.clear();
                    return;
                }
                let keys: Vec<_> = rows.pages.keys().collect();
                for page in keys {
                    for cell in 0..CELLS {
                        let pos = Position { page, cell };
                        if rows.entry(pos).is_some() {
                            let old = rows.set(pos, None);
                            rows.record(Undo::Entry(pos, old));
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn journal_transaction(&mut self) -> Transaction<'_, K, V> {
        let token = self.begin_journal();
        Transaction {
            rows: self,
            token: Some(token),
        }
    }
}
impl<K: Ord, V> PrimitiveRows<K, V> {
    pub(crate) fn get(&self, key: &K) -> Option<&V> {
        self.get_key_value(key).map(|(_, value)| value)
    }
    pub(crate) fn get_key_value(&self, key: &K) -> Option<(&K, &V)> {
        match &self.storage {
            Storage::Tree(rows) => rows.get_key_value(key),
            Storage::Paged(rows) => rows
                .entry((rows.map)(key))
                .map(|entry| (entry.key.as_ref(), entry.value.as_ref())),
        }
    }
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }
    pub(crate) fn prepare_change<R>(
        &mut self,
        key: &K,
        inspect: impl FnOnce(Option<&V>) -> R,
    ) -> (PreparedChange<'_, K, V>, R) {
        self.prepare_change_with_policy::<Stored, R>(key, inspect)
    }
    #[inline]
    pub(crate) fn prepare_change_with_policy<P: LookupPolicy<K>, R>(
        &mut self,
        key: &K,
        inspect: impl FnOnce(Option<&V>) -> R,
    ) -> (PreparedChange<'_, K, V>, R) {
        match &mut self.storage {
            Storage::Tree(rows) => {
                let (prepared, observed) = rows.prepare_change(key, inspect);
                (
                    PreparedChange {
                        inner: PreparedKind::Tree(prepared),
                    },
                    observed,
                )
            }
            Storage::Paged(rows) => {
                let pos = P::locate(rows.map, key);
                let (prepared, observed) = rows.prepare(pos, inspect);
                (
                    PreparedChange {
                        inner: PreparedKind::Paged(prepared),
                    },
                    observed,
                )
            }
        }
    }
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.insert_with_policy::<Stored>(key, value);
    }
    #[inline]
    pub(crate) fn insert_with_policy<P: LookupPolicy<K>>(&mut self, key: K, value: V) {
        match &mut self.storage {
            Storage::Tree(rows) => rows.insert(key, value),
            Storage::Paged(rows) => {
                let pos = P::locate(rows.map, &key);
                rows.prepare(pos, |_| ()).0.insert(key, value);
            }
        }
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        match &mut self.storage {
            Storage::Tree(rows) => rows.remove(key),
            Storage::Paged(rows) => {
                let pos = (rows.map)(key);
                rows.prepare(pos, |_| ()).0.remove()
            }
        }
    }
}
// Only this concrete library-owned collector can opt into a reset-on-retire
// policy. Arbitrary Drop payloads remain entirely outside allocation reuse.
impl<K> PrimitiveRows<K, crate::IntCollector> {
    pub(crate) fn enable_int_collector_pool(&mut self) -> bool {
        let Storage::Paged(rows) = &mut self.storage else {
            return false;
        };
        rows.pools
            .enable_value_reset(|collector| *collector = crate::IntCollector::default());
        true
    }
}
impl<K: 'static, V> PrimitiveRows<K, V> {
    pub(crate) fn bind_primitive(&mut self) -> bool {
        if self.is_primitive() {
            return true;
        }
        let Some(map) = mapper::<K>() else {
            return false;
        };
        let Storage::Tree(old) = &self.storage else {
            unreachable!()
        };
        let mut rows = Paged::new(map);
        for (key, value) in old.shared_entries() {
            rows.set(map(&key), Some(Entry { key, value }));
        }
        self.storage = Storage::Paged(rows);
        true
    }
}
fn mapper<K: 'static>() -> Option<Mapper<K>> {
    fn cast<K: 'static, T: 'static>(key: &K) -> &T {
        (key as &dyn Any)
            .downcast_ref()
            .expect("checked primitive key type")
    }
    let kind = TypeId::of::<K>();
    if kind == TypeId::of::<usize>() {
        Some(|key| position([0, 0, 0, *cast::<K, usize>(key)]))
    } else if kind == TypeId::of::<(usize, usize)>() {
        Some(|key| {
            let &(a, b) = cast::<K, (usize, usize)>(key);
            position([0, 0, a, b])
        })
    } else if kind == TypeId::of::<(usize, usize, usize)>() {
        Some(|key| {
            let &(a, b, c) = cast::<K, (usize, usize, usize)>(key);
            position([0, a, b, c])
        })
    } else if kind == TypeId::of::<((usize, usize), usize)>() {
        Some(|key| {
            let &((a, b), c) = cast::<K, ((usize, usize), usize)>(key);
            position([0, a, b, c])
        })
    } else if kind == TypeId::of::<(usize, (usize, usize))>() {
        Some(|key| {
            let &(a, (b, c)) = cast::<K, (usize, (usize, usize))>(key);
            position([0, a, b, c])
        })
    } else if kind == TypeId::of::<((usize, usize), (usize, usize))>() {
        Some(|key| {
            let &((a, b), (c, d)) = cast::<K, ((usize, usize), (usize, usize))>(key);
            position([a, b, c, d])
        })
    } else if kind == TypeId::of::<(usize, usize, usize, usize)>() {
        Some(|key| {
            let &(a, b, c, d) = cast::<K, (usize, usize, usize, usize)>(key);
            position([a, b, c, d])
        })
    } else {
        None
    }
}

pub(crate) struct Iter<'a, K, V> {
    inner: IterKind<'a, K, V>,
    remaining: usize,
}
enum IterKind<'a, K, V> {
    Tree(crate::snapshot_map::Iter<'a, K, V>),
    Paged {
        pages: directory::Values<'a, K, V>,
        cells: std::slice::Iter<'a, Option<Entry<K, V>>>,
    },
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let row = match &mut self.inner {
            IterKind::Tree(rows) => rows.next(),
            IterKind::Paged { pages, cells } => loop {
                if let Some(row) = cells.find_map(Option::as_ref) {
                    break Some((row.key.as_ref(), row.value.as_ref()));
                }
                *cells = pages.next()?.cells.iter();
            },
        };
        if row.is_some() {
            self.remaining -= 1;
        }
        row
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}
impl<K, V> std::iter::FusedIterator for Iter<'_, K, V> {}
impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for PrimitiveRows<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
impl<K: PartialEq, V: PartialEq> PartialEq for PrimitiveRows<K, V> {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl<K: Eq, V: Eq> Eq for PrimitiveRows<K, V> {}
impl<K: Ord, V> Index<&K> for PrimitiveRows<K, V> {
    type Output = V;
    fn index(&self, key: &K) -> &V {
        self.get(key).expect("missing primitive row key")
    }
}
pub(crate) struct Transaction<'a, K, V> {
    rows: &'a mut PrimitiveRows<K, V>,
    token: Option<usize>,
}
impl<K: Ord, V> Transaction<'_, K, V> {
    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.rows.insert(key, value);
    }
    pub(crate) fn remove(&mut self, key: &K) -> bool {
        self.rows.remove(key)
    }
    pub(crate) fn commit(mut self) {
        self.rows.commit_journal(self.token.take().unwrap());
    }
}
impl<K, V> Drop for Transaction<'_, K, V> {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            self.rows.rollback_journal(token);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    fn collect<K: Ord + Clone, V: Clone>(rows: &PrimitiveRows<K, V>) -> BTreeMap<K, V> {
        rows.iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    #[test]
    fn primitive_domains_preserve_order_and_full_unsigned_boundaries() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        for key in [usize::MAX, 16, 0, usize::MAX - 1, 15, 17] {
            rows.insert(key, key);
        }
        assert_eq!(rows.iter().len(), 6);
        assert_eq!(
            rows.values().copied().collect::<Vec<_>>(),
            [0, 15, 16, 17, usize::MAX - 1, usize::MAX]
        );
        assert_eq!(rows.get(&usize::MAX), Some(&usize::MAX));
        let mut pairs = PrimitiveRows::new();
        assert!(pairs.bind_primitive());
        for key in [
            (usize::MAX, usize::MAX),
            (0, usize::MAX),
            (1, 0),
            (0, 15),
            (0, 16),
        ] {
            pairs.insert(key, key);
        }
        assert_eq!(
            pairs.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
            [
                (0, 15),
                (0, 16),
                (0, usize::MAX),
                (1, 0),
                (usize::MAX, usize::MAX)
            ]
        );
        let mut nested = PrimitiveRows::new();
        assert!(nested.bind_primitive());
        nested.insert(((usize::MAX, 0), (usize::MAX, usize::MAX)), 1);
        nested.insert(((0, usize::MAX), (0, 0)), 2);
        assert_eq!(nested.values().copied().collect::<Vec<_>>(), [2, 1]);
        let mut flat = PrimitiveRows::<(usize, usize, usize, usize), ()>::new();
        assert!(flat.bind_primitive());
        flat.insert((usize::MAX, 0, usize::MAX, usize::MAX), ());
        assert_eq!(flat.len(), 1);
    }

    #[test]
    fn snapshots_and_materialization_retain_actual_nonclone_owners_and_key_representatives() {
        struct Value(i64);
        let mut rows = PrimitiveRows::new();
        rows.insert((usize::MAX, 15usize), Value(7));
        let old = rows.clone();
        let (key_ptr, value_ptr) = rows
            .get_key_value(&(usize::MAX, 15))
            .map(|(k, v)| (k as *const _, v as *const _))
            .unwrap();
        assert!(rows.bind_primitive());
        let (key, value) = rows.get_key_value(&(usize::MAX, 15)).unwrap();
        assert_eq!((key as *const _, value as *const _), (key_ptr, value_ptr));
        assert_eq!(value.0, 7);
        let saved = rows.clone();
        let materialized = rows.materialized().clone();
        assert!(std::ptr::eq(&materialized[&(usize::MAX, 15)], value));
        rows.insert((usize::MAX, 15), Value(8));
        assert_eq!(
            rows.get_key_value(&(usize::MAX, 15)).unwrap().0 as *const _,
            key_ptr
        );
        rows.remove(&(usize::MAX, 15));
        rows.insert((0, 0), Value(9));
        rows.install_snapshot(saved);
        assert_eq!(
            rows.get_key_value(&(usize::MAX, 15)).unwrap().1 as *const _,
            value_ptr
        );
        assert_eq!(materialized[&(usize::MAX, 15)].0, 7);
        assert_eq!(old.get(&(usize::MAX, 15)).unwrap().0, 7);
    }

    #[test]
    fn nested_journals_saved_installs_diagnostics_and_transactions_restore_complete_rows() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        for key in [0usize, 15, 16, usize::MAX] {
            rows.insert(key, key);
        }
        let original = collect(&rows);
        let outer = rows.begin_journal();
        rows.insert(0, 10);
        rows.remove(&15);
        let before_inner = collect(&rows);
        let full = rows.clone();
        let inner = rows.begin_journal();
        rows.insert(0, 20);
        rows.insert(17, 17);
        rows.remove(&usize::MAX);
        assert_eq!(collect(&rows.journal_snapshot(inner)), before_inner);
        assert_eq!(collect(&rows.journal_snapshot(outer)), original);
        rows.commit_journal(inner);
        rows.install_snapshot(full);
        assert_eq!(collect(&rows), before_inner);
        rows.clear();
        assert!(rows.is_empty_for_tests());
        rows.rollback_journal(outer);
        assert_eq!(collect(&rows), original);
        {
            let mut edit = rows.journal_transaction();
            edit.remove(&0);
            edit.insert(31, 1);
        }
        assert_eq!(collect(&rows), original);
        {
            let mut edit = rows.journal_transaction();
            edit.remove(&0);
            edit.insert(31, 1);
            edit.commit();
        }
        assert_eq!(rows.get(&31), Some(&1));
        assert!(!rows.contains_key(&0));
    }

    #[test]
    fn empty_page_cleanup_bounds_history_after_direct_and_nested_mutations() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        for key in 0usize..200 {
            rows.insert(key * CELLS, key);
            rows.remove(&(key * CELLS));
        }
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert!(paged.pages.is_empty());
        for key in 0usize..200 {
            let outer = rows.begin_journal();
            rows.insert((key + 200) * CELLS, key);
            let inner = rows.begin_journal();
            rows.remove(&((key + 200) * CELLS));
            rows.commit_journal(inner);
            rows.commit_journal(outer);
        }
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert!(paged.pages.is_empty());
        assert!(paged.empty.is_empty());
        let outer = rows.begin_journal();
        rows.insert(usize::MAX, 1);
        rows.rollback_journal(outer);
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert!(paged.pages.is_empty());
    }

    #[test]
    fn payloads_release_after_their_actual_snapshots_and_journals_finish() {
        struct Value(Rc<Cell<usize>>);
        impl Drop for Value {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(0usize, Value(drops.clone()));
        let held = rows.clone();
        let outer = rows.begin_journal();
        rows.remove(&0);
        assert_eq!(drops.get(), 0);
        rows.rollback_journal(outer);
        rows.remove(&0);
        assert_eq!(drops.get(), 0);
        drop(held);
        assert_eq!(drops.get(), 1);
        rows.insert(usize::MAX, Value(drops.clone()));
        rows.remove(&usize::MAX);
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn generic_and_borrowed_keys_keep_the_tree_path() {
        let text = String::from("borrowed");
        let mut rows = PrimitiveRows::new();
        rows.insert(text.as_str(), 1);
        assert_eq!(rows.get(&"borrowed"), Some(&1));
        assert!(!rows.is_primitive());
        let mut strings = PrimitiveRows::new();
        strings.insert(String::from("a"), 1);
        assert!(!strings.bind_primitive());
        let saved = strings.clone();
        let token = strings.begin_journal();
        strings.insert(String::from("b"), 2);
        strings.rollback_journal(token);
        assert_eq!(strings, saved);
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use std::{cell::Cell, cmp::Ordering, rc::Rc};

    fn contents(rows: &PrimitiveRows<usize, i64>) -> BTreeMap<usize, i64> {
        rows.iter().map(|(key, value)| (*key, *value)).collect()
    }

    #[test]
    fn capture_deduplicates_nested_touches_without_cloning_pages_and_retains_final_key_owners() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        for key in 0..128usize {
            rows.insert(key, key as i64);
        }
        rows.insert(usize::MAX, -1);
        let baseline = rows.clone();
        let original_key = rows.get_key_value(&0).unwrap().0 as *const usize;
        let candidate = rows.begin_journal();
        rows.insert(15, 1015);
        rows.insert(15, 2015);
        rows.remove(&0);
        rows.insert(0, 1000);
        rows.remove(&usize::MAX);
        rows.insert(usize::MAX - 1, 9);
        let inner = rows.begin_journal();
        rows.insert(16, 1016);
        rows.commit_journal(inner);
        let discarded = rows.begin_journal();
        rows.insert(31, 1031);
        rows.rollback_journal(discarded);
        let final_key = rows.get_key_value(&0).unwrap().0 as *const usize;
        assert_ne!(original_key, final_key);
        let expected = contents(&rows);
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        let owners: Vec<_> = paged
            .pages
            .values()
            .map(|page| page.as_ref() as *const _)
            .collect();
        let capture = rows.capture_current(candidate);
        let RowsCapture::Delta(delta) = &capture else {
            panic!("expected sparse capture")
        };
        assert_eq!(delta.entries.len(), 5);
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert_eq!(
            paged
                .pages
                .values()
                .map(|page| page.as_ref() as *const _)
                .collect::<Vec<_>>(),
            owners
        );
        rows.rollback_journal(candidate);
        assert_eq!(contents(&rows), contents(&baseline));
        assert_eq!(
            rows.get_key_value(&0).unwrap().0 as *const usize,
            original_key
        );
        for value in [20, 30, 40] {
            let other = rows.begin_journal();
            rows.insert(15, value);
            rows.remove(&16);
            rows.insert(31, value);
            rows.rollback_journal(other);
        }
        // A materialized view is invalidated by installing the candidate.
        let before = rows.materialized().clone();
        rows.install_capture(capture.clone());
        assert_eq!(contents(&rows), expected);
        assert_eq!(rows.get_key_value(&0).unwrap().0 as *const usize, final_key);
        assert_eq!(before[&0], 0);
        assert_eq!(rows.materialized()[&0], 1000);
        assert!(!rows.contains_key(&usize::MAX));
        rows.install_snapshot(baseline);
        rows.install_capture(capture);
        assert_eq!(contents(&rows), expected);
    }

    #[test]
    fn capture_inside_nested_journals_installs_over_the_same_base_and_preserves_outer_undo() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        for (key, value) in [(1usize, 1), (2, 2), (100, 100)] {
            rows.insert(key, value);
        }
        let baseline = contents(&rows);
        let outer = rows.begin_journal();
        rows.insert(100, 110);
        let candidate = rows.begin_journal();
        rows.insert(1, 10);
        rows.remove(&2);
        let inner = rows.begin_journal();
        rows.insert(3, 30);
        rows.commit_journal(inner);
        let final_state = contents(&rows);
        let captured = rows.capture_current(candidate);
        assert_eq!(
            contents(&rows.journal_snapshot(candidate)),
            BTreeMap::from([(1, 1), (2, 2), (100, 110)])
        );
        rows.rollback_journal(candidate);
        rows.install_capture(captured.clone());
        assert_eq!(contents(&rows), final_state);
        assert_eq!(contents(&rows.journal_snapshot(outer)), baseline);
        rows.rollback_journal(outer);
        assert_eq!(contents(&rows), baseline);
        let outer = rows.begin_journal();
        rows.insert(100, 110);
        rows.install_capture(captured);
        rows.commit_journal(outer);
        assert_eq!(contents(&rows), final_state);
    }

    #[test]
    fn capture_absences_clear_empty_pages_and_noop_captures_hold_no_unrelated_payloads() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(usize::MAX, 1i64);
        rows.insert(16usize, 2);
        let candidate = rows.begin_journal();
        rows.remove(&usize::MAX);
        rows.insert(32, 3);
        rows.remove(&32);
        let capture = rows.capture_current(candidate);
        let RowsCapture::Delta(delta) = &capture else {
            unreachable!()
        };
        assert_eq!(delta.entries.len(), 2);
        assert!(delta.entries.iter().all(|(_, entry)| entry.is_none()));
        rows.rollback_journal(candidate);
        rows.install_capture(capture);
        assert_eq!(contents(&rows), BTreeMap::from([(16, 2)]));
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert_eq!(paged.pages.len(), 1);
        assert!(paged.empty.is_empty());
        let candidate = rows.begin_journal();
        let capture = rows.capture_current(candidate);
        let RowsCapture::Delta(delta) = &capture else {
            unreachable!()
        };
        assert!(delta.entries.is_empty());
        rows.rollback_journal(candidate);
        rows.install_capture(capture);
        assert_eq!(contents(&rows), BTreeMap::from([(16, 2)]));
    }

    #[test]
    fn capture_shares_nonclone_payloads_and_nested_pair_maximum_key_representatives() {
        struct NoClone(i64);
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        let key = ((usize::MAX, usize::MAX), (usize::MAX, usize::MAX));
        rows.insert(key, NoClone(1));
        let candidate = rows.begin_journal();
        rows.remove(&key);
        rows.insert(key, NoClone(2));
        let (final_key, final_value) = rows.get_key_value(&key).unwrap();
        let pointers = (final_key as *const _, final_value as *const _);
        let captured = rows.capture_current(candidate);
        rows.rollback_journal(candidate);
        assert_eq!(rows.get(&key).unwrap().0, 1);
        rows.install_capture(captured);
        let (installed_key, installed_value) = rows.get_key_value(&key).unwrap();
        assert_eq!(
            (installed_key as *const _, installed_value as *const _),
            pointers
        );
        assert_eq!(installed_value.0, 2);
    }

    #[test]
    fn custom_tree_capture_is_full_and_performs_no_user_clone_or_ordering() {
        struct Key(usize, Rc<Cell<usize>>);
        impl PartialEq for Key {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl Eq for Key {}
        impl PartialOrd for Key {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for Key {
            fn cmp(&self, other: &Self) -> Ordering {
                self.1.set(self.1.get() + 1);
                self.0.cmp(&other.0)
            }
        }
        struct NoClone(usize);
        let calls = Rc::new(Cell::new(0));
        let mut rows = PrimitiveRows::new();
        rows.insert(Key(1, calls.clone()), NoClone(1));
        assert!(!rows.bind_primitive());
        let candidate = rows.begin_journal();
        rows.insert(Key(2, calls.clone()), NoClone(2));
        let before = calls.get();
        let capture = rows.capture_current(candidate);
        assert!(matches!(&capture, RowsCapture::Full(_)));
        assert_eq!(calls.get(), before);
        rows.rollback_journal(candidate);
        assert_eq!(calls.get(), before);
        let outer = rows.begin_journal();
        rows.install_capture(capture);
        assert_eq!(calls.get(), before);
        assert_eq!(
            rows.iter()
                .map(|(key, value)| (key.0, value.0))
                .collect::<Vec<_>>(),
            [(1, 1), (2, 2)]
        );
        rows.rollback_journal(outer);
        assert_eq!(calls.get(), before);
        assert_eq!(rows.len(), 1);
    }
}

#[cfg(test)]
mod prepared_tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn primitive_prepared_changes_preserve_preflight_atomicity_representatives_and_delta_capture() {
        struct NoClone(i64);
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(usize::MAX, NoClone(1));
        let key_owner = rows.get_key_value(&usize::MAX).unwrap().0 as *const usize;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = rows.prepare_change::<()>(&usize::MAX, |_| panic!("preflight failure"));
            }))
            .is_err()
        );
        assert_eq!(rows.get(&usize::MAX).unwrap().0, 1);
        {
            let (_prepared, value) = rows.prepare_change(&usize::MAX, |row| row.unwrap().0);
            assert_eq!(value, 1);
        }
        let token = rows.begin_journal();
        let (prepared, old) = rows.prepare_change(&usize::MAX, |row| row.unwrap().0);
        assert_eq!(old, 1);
        prepared.insert(usize::MAX, NoClone(2));
        assert_eq!(
            rows.get_key_value(&usize::MAX).unwrap().0 as *const usize,
            key_owner
        );
        rows.prepare_change(&15, |_| ()).0.insert(15, NoClone(3));
        let saved = rows.capture_current(token);
        rows.prepare_change(&usize::MAX, |_| ()).0.remove();
        rows.rollback_journal(token);
        assert_eq!(rows.len(), 1);
        rows.install_capture(saved);
        assert_eq!(rows.get(&usize::MAX).unwrap().0, 2);
        assert_eq!(rows.get(&15).unwrap().0, 3);
        assert_eq!(
            rows.get_key_value(&usize::MAX).unwrap().0 as *const usize,
            key_owner
        );
    }

    #[test]
    fn borrowed_tree_prepared_changes_keep_default_lifetime_support() {
        struct NoClone;
        let name = String::from("borrowed");
        let mut rows = PrimitiveRows::new();
        rows.prepare_change(&name.as_str(), |row| assert!(row.is_none()))
            .0
            .insert(name.as_str(), NoClone);
        let owner = rows.get_key_value(&name.as_str()).unwrap().0 as *const &str;
        rows.prepare_change(&name.as_str(), |row| assert!(row.is_some()))
            .0
            .insert(name.as_str(), NoClone);
        assert_eq!(
            rows.get_key_value(&name.as_str()).unwrap().0 as *const &str,
            owner
        );
        rows.prepare_change(&name.as_str(), |_| ()).0.remove();
        assert!(rows.is_empty_for_tests());
        assert!(!rows.is_primitive());
    }
}

#[cfg(test)]
mod entry_cursor_tests {
    use super::*;
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    #[test]
    fn held_directory_entries_do_not_publish_or_acquire_owners_before_commit() {
        struct Value(i64);
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(15usize, Value(1));
        rows.insert(14usize, Value(2));
        let held = rows.clone();
        let owner = rows.get_key_value(&15).unwrap().1 as *const Value;
        for key in [15usize, 13, usize::MAX] {
            let Storage::Paged(paged) = &rows.storage else {
                unreachable!()
            };
            let pages: Vec<_> = paged
                .pages
                .iter()
                .map(|(key, page)| {
                    (
                        key,
                        page.as_ref() as *const _,
                        page.cells
                            .iter()
                            .map(|entry| {
                                entry.as_ref().map(|entry| {
                                    (
                                        Arc::strong_count(&entry.key),
                                        Arc::strong_count(&entry.value),
                                    )
                                })
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let (_prepared, _) = rows.prepare_change(&key, |_| ());
                    // Model a payload Clone failure after a successful inspection.
                    panic!("output construction failed");
                }))
                .is_err()
            );
            assert_eq!(rows.len(), 2);
            assert_eq!(rows.get(&15).unwrap() as *const Value, owner);
            let Storage::Paged(paged) = &rows.storage else {
                unreachable!()
            };
            assert_eq!(
                paged
                    .pages
                    .iter()
                    .map(|(key, page)| (
                        key,
                        page.as_ref() as *const _,
                        page.cells
                            .iter()
                            .map(|entry| entry.as_ref().map(|entry| (
                                Arc::strong_count(&entry.key),
                                Arc::strong_count(&entry.value)
                            )))
                            .collect::<Vec<_>>()
                    ))
                    .collect::<Vec<_>>(),
                pages
            );
        }
        assert_eq!(held.get(&15).unwrap().0, 1);
        rows.prepare_change(&15, |_| ()).0.insert(15, Value(3));
        assert_eq!(rows.get(&15).unwrap().0, 3);
        assert_eq!(held.get(&15).unwrap().0, 1);
        rows.prepare_change(&13, |_| ()).0.insert(13, Value(4));
        rows.prepare_change(&usize::MAX, |_| ())
            .0
            .insert(usize::MAX, Value(5));
        assert_eq!(
            rows.iter()
                .map(|(key, value)| (*key, value.0))
                .collect::<Vec<_>>(),
            [(13, 4), (14, 2), (15, 3), (usize::MAX, 5)]
        );
    }

    #[test]
    fn occupied_cell_mutations_keep_payload_drop_order_and_direct_empty_cleanup() {
        struct Value(&'static str, Rc<RefCell<Vec<&'static str>>>);
        impl Drop for Value {
            fn drop(&mut self) {
                self.1.borrow_mut().push(self.0);
            }
        }
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(usize::MAX, Value("old", trace.clone()));
        let key_owner = rows.get_key_value(&usize::MAX).unwrap().0 as *const usize;
        rows.prepare_change(&usize::MAX, |_| ())
            .0
            .insert(usize::MAX, Value("new", trace.clone()));
        assert_eq!(&*trace.borrow(), &["old"]);
        assert_eq!(
            rows.get_key_value(&usize::MAX).unwrap().0 as *const usize,
            key_owner
        );
        rows.prepare_change(&usize::MAX, |_| ()).0.remove();
        assert_eq!(&*trace.borrow(), &["old", "new"]);
        let Storage::Paged(paged) = &rows.storage else {
            unreachable!()
        };
        assert!(paged.pages.is_empty());
        assert!(paged.empty.is_empty());
        rows.prepare_change(&usize::MAX, |_| ()).0.remove();
        assert_eq!(&*trace.borrow(), &["old", "new"]);
    }

    #[test]
    fn held_entry_updates_restore_nested_representatives_and_captured_absences() {
        let mut rows = PrimitiveRows::new();
        assert!(rows.bind_primitive());
        rows.insert(usize::MAX, 1i64);
        rows.insert(15usize, 2);
        let original_key = rows.get_key_value(&usize::MAX).unwrap().0 as *const usize;
        let outer = rows.begin_journal();
        rows.prepare_change(&usize::MAX, |_| ()).0.remove();
        rows.prepare_change(&usize::MAX, |_| ())
            .0
            .insert(usize::MAX, 10);
        let new_key = rows.get_key_value(&usize::MAX).unwrap().0 as *const usize;
        assert_ne!(new_key, original_key);
        let inner = rows.begin_journal();
        rows.prepare_change(&15, |_| ()).0.remove();
        rows.prepare_change(&16, |_| ()).0.insert(16, 20);
        rows.commit_journal(inner);
        let capture = rows.capture_current(outer);
        rows.rollback_journal(outer);
        assert_eq!(
            rows.get_key_value(&usize::MAX).unwrap().0 as *const usize,
            original_key
        );
        assert_eq!(rows.get(&15), Some(&2));
        let outer = rows.begin_journal();
        rows.install_capture(capture);
        assert_eq!(
            rows.get_key_value(&usize::MAX).unwrap().0 as *const usize,
            new_key
        );
        assert!(!rows.contains_key(&15));
        assert_eq!(rows.get(&16), Some(&20));
        rows.rollback_journal(outer);
        assert_eq!(rows.get(&usize::MAX), Some(&1));
        assert_eq!(rows.get(&15), Some(&2));
        assert!(!rows.contains_key(&16));
    }
}
