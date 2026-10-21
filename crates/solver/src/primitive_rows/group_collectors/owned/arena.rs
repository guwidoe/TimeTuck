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
#[derive(Clone, Default)]
pub(super) struct Arena {
    values: Vec<Option<Value>>,
    free: Vec<usize>,
}
impl Arena {
    pub(super) fn clone_live(&self, live: impl Iterator<Item = usize>) -> Self {
        let mut marks = vec![false; self.values.len()];
        for index in live {
            marks[index] = true;
        }
        let values = self
            .values
            .iter()
            .enumerate()
            .map(|(i, value)| if marks[i] { value.clone() } else { None })
            .collect::<Vec<_>>();
        let free = values
            .iter()
            .enumerate()
            .filter_map(|(i, value)| value.is_none().then_some(i))
            .collect();
        Self { values, free }
    }
    #[cfg(test)]
    pub(super) fn retain_live(&mut self, live: impl Iterator<Item = usize>) {
        let mut marks = vec![false; self.values.len()];
        for index in live {
            marks[index] = true;
        }
        self.free.clear();
        for (index, value) in self.values.iter_mut().enumerate() {
            if !marks[index] {
                *value = None;
            }
            if value.is_none() {
                self.free.push(index);
            }
        }
    }

    pub(super) fn insert(&mut self, pos: Position, collector: IntCollector) -> usize {
        let value = Value {
            address: Address::new(pos),
            collector,
        };
        match self.free.pop() {
            Some(index) => {
                assert!(self.values[index].is_none(), "free collector slot");
                self.values[index] = Some(value);
                index
            }
            None => {
                let index = self.values.len();
                self.values.push(Some(value));
                index
            }
        }
    }
    pub(super) fn retire(&mut self, index: usize) {
        let value = self.values[index].take().expect("owned collector slot");
        drop(value);
        self.free.push(index);
    }
    #[inline(always)]
    pub(super) fn get(&self, index: usize) -> &IntCollector {
        &self.values[index]
            .as_ref()
            .expect("owned collector slot")
            .collector
    }
    #[inline(always)]
    pub(super) fn get_mut(&mut self, index: usize) -> &mut IntCollector {
        &mut self.values[index]
            .as_mut()
            .expect("owned collector slot")
            .collector
    }
    pub(super) fn position(&self, index: usize) -> Position {
        self.values[index]
            .as_ref()
            .expect("owned collector slot")
            .address
            .position()
    }
}
