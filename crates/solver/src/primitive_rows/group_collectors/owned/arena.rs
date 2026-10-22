//! Value slots stay allocated while any live row or ordered undo owns their index.
use super::*;
#[derive(Clone)]
enum Address {
    Small(usize),
    Wide(Box<Position>),
}
impl Address {
    fn new(pos: Position) -> Self {
        if pos.page.0.iter().all(|&p| p <= POSITION_MASK) {
            let mut packed = 0;
            for part in pos.page.0 {
                packed = (packed << POSITION_BITS) | part;
            }
            Self::Small((packed << 4) | pos.cell)
        } else {
            Self::Wide(Box::new(pos))
        }
    }
    fn position(&self) -> Position {
        match self {
            Self::Small(packed) => InverseFrame::decode(&[], *packed),
            Self::Wide(pos) => **pos,
        }
    }
}
#[derive(Clone)]
struct Value {
    address: Address,
    collector: IntCollector,
}
#[derive(Clone)]
struct Counter {
    address: Address,
    count: usize,
}
#[derive(Default)]
struct Uniform {
    value: Option<i64>,
    values: Vec<Option<Counter>>,
    observed: std::sync::OnceLock<Vec<Option<IntCollector>>>,
}
impl Clone for Uniform {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            values: self.values.clone(),
            observed: std::sync::OnceLock::new(),
        }
    }
}
#[derive(Clone)]
enum Storage {
    Uniform(Uniform),
    General(Vec<Option<Value>>),
}
#[derive(Clone)]
pub(super) struct Arena {
    storage: Storage,
    free: Vec<usize>,
}
impl Default for Arena {
    fn default() -> Self {
        Self {
            storage: Storage::Uniform(Uniform::default()),
            free: Vec::new(),
        }
    }
}
impl Arena {
    fn len(&self) -> usize {
        match &self.storage {
            Storage::Uniform(u) => u.values.len(),
            Storage::General(v) => v.len(),
        }
    }
    pub(super) fn clone_live(&self, live: impl Iterator<Item = usize>) -> Self {
        let mut marks = vec![false; self.len()];
        for index in live {
            marks[index] = true;
        }
        let storage = match &self.storage {
            Storage::Uniform(u) => Storage::Uniform(Uniform {
                value: u.value,
                values: u
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if marks[i] { v.clone() } else { None })
                    .collect(),
                observed: std::sync::OnceLock::new(),
            }),
            Storage::General(v) => Storage::General(
                v.iter()
                    .enumerate()
                    .map(|(i, v)| if marks[i] { v.clone() } else { None })
                    .collect(),
            ),
        };
        let free = match &storage {
            Storage::Uniform(u) => u
                .values
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.is_none().then_some(i))
                .collect(),
            Storage::General(v) => v
                .iter()
                .enumerate()
                .filter_map(|(i, v)| v.is_none().then_some(i))
                .collect(),
        };
        Self { storage, free }
    }
    #[cfg(test)]
    pub(super) fn retain_live(&mut self, live: impl Iterator<Item = usize>) {
        let mut marks = vec![false; self.len()];
        for index in live {
            marks[index] = true;
        }
        self.free.clear();
        match &mut self.storage {
            Storage::Uniform(u) => {
                u.observed.take();
                for (i, v) in u.values.iter_mut().enumerate() {
                    if !marks[i] {
                        *v = None;
                    }
                    if v.is_none() {
                        self.free.push(i);
                    }
                }
            }
            Storage::General(values) => {
                for (i, v) in values.iter_mut().enumerate() {
                    if !marks[i] {
                        *v = None;
                    }
                    if v.is_none() {
                        self.free.push(i);
                    }
                }
            }
        }
    }
    // Reserve before moving any existing state. After this allocation succeeds,
    // conversion moves addresses/indices and constructs only primitive values.
    #[cold]
    #[inline(never)]
    fn promote(&mut self) {
        let Storage::Uniform(u) = &self.storage else {
            return;
        };
        let mut values = Vec::with_capacity(u.values.len());
        let Storage::Uniform(u) =
            std::mem::replace(&mut self.storage, Storage::General(Vec::new()))
        else {
            unreachable!()
        };
        let value = u.value.unwrap_or(0);
        for slot in u.values {
            values.push(slot.map(|v| Value {
                address: v.address,
                collector: IntCollector::from_single_state(value, v.count),
            }));
        }
        self.storage = Storage::General(values);
    }
    pub(super) fn insert(&mut self, pos: Position, collector: IntCollector) -> usize {
        let address = Address::new(pos);
        if let Storage::Uniform(u) = &mut self.storage {
            if let Some((value, count)) = collector.single_state()
                && (count == 0 || u.value.is_none_or(|v| v == value))
            {
                u.observed.take();
                if count != 0 && u.value.is_none() {
                    u.value = Some(value);
                }
                let counter = Counter { address, count };
                return match self.free.pop() {
                    Some(index) => {
                        assert!(u.values[index].is_none(), "free collector slot");
                        u.values[index] = Some(counter);
                        index
                    }
                    None => {
                        let index = u.values.len();
                        u.values.push(Some(counter));
                        index
                    }
                };
            }
        }
        self.promote();
        let Storage::General(values) = &mut self.storage else {
            unreachable!()
        };
        let value = Value { address, collector };
        match self.free.pop() {
            Some(index) => {
                assert!(values[index].is_none(), "free collector slot");
                values[index] = Some(value);
                index
            }
            None => {
                let index = values.len();
                values.push(Some(value));
                index
            }
        }
    }
    pub(super) fn retire(&mut self, index: usize) {
        match &mut self.storage {
            Storage::Uniform(u) => {
                u.observed.take();
                drop(u.values[index].take().expect("owned collector slot"));
            }
            Storage::General(v) => drop(v[index].take().expect("owned collector slot")),
        }
        self.free.push(index);
    }
    #[inline(always)]
    pub(super) fn count(&self, index: usize) -> usize {
        match &self.storage {
            Storage::Uniform(u) => {
                u.values[index]
                    .as_ref()
                    .expect("owned collector slot")
                    .count
            }
            Storage::General(v) => v[index]
                .as_ref()
                .expect("owned collector slot")
                .collector
                .occurrence_count(),
        }
    }
    // A foreign insertion requests promotion without changing logical state.
    // Missing captures panic before view invalidation/count publication,just as
    // the original primitive mutation. The caller owns original guard/journal order.
    #[inline(always)]
    pub(super) fn try_uniform_update(
        &mut self,
        index: usize,
        edit: ScalarEdit,
        missing: &'static str,
    ) -> Option<(i64, usize)> {
        let Storage::Uniform(u) = &mut self.storage else {
            return None;
        };
        let cell = u.values[index].as_mut().expect("owned collector slot");
        let count = cell.count;
        let (value, next) = match edit {
            ScalarEdit::Insert(value) => {
                if u.value.is_some_and(|v| v != value) {
                    return None;
                }
                (
                    value,
                    count.checked_add(1).expect("collector count overflow"),
                )
            }
            ScalarEdit::Remove(value) => {
                assert!(count != 0 && u.value == Some(value), "{missing}");
                (value, count - 1)
            }
            ScalarEdit::Replace(old, new) => {
                assert!(count != 0 && u.value == Some(old), "{missing}");
                if old != new {
                    return None;
                }
                (new, count)
            }
        };
        u.observed.take();
        if u.value.is_none() {
            u.value = Some(value);
        }
        cell.count = next;
        Some((value, next))
    }
    #[inline(always)]
    pub(super) fn get(&self, index: usize) -> &IntCollector {
        match &self.storage {
            Storage::Uniform(u) => u.observed.get_or_init(|| {
                let value = u.value.unwrap_or(0);
                u.values
                    .iter()
                    .map(|v| {
                        v.as_ref()
                            .map(|v| IntCollector::from_single_state(value, v.count))
                    })
                    .collect()
            })[index]
                .as_ref()
                .expect("owned collector slot"),
            Storage::General(v) => &v[index].as_ref().expect("owned collector slot").collector,
        }
    }
    #[inline(always)]
    pub(super) fn get_mut(&mut self, index: usize) -> &mut IntCollector {
        if matches!(self.storage, Storage::Uniform(_)) {
            self.promote();
        }
        let Storage::General(v) = &mut self.storage else {
            unreachable!()
        };
        &mut v[index].as_mut().expect("owned collector slot").collector
    }
    pub(super) fn position(&self, index: usize) -> Position {
        match &self.storage {
            Storage::Uniform(u) => u.values[index]
                .as_ref()
                .expect("owned collector slot")
                .address
                .position(),
            Storage::General(v) => v[index]
                .as_ref()
                .expect("owned collector slot")
                .address
                .position(),
        }
    }
}
#[cfg(test)]
mod uniform_tests;
