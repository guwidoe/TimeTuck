//! Products retain only one current item per child and restart exhausted suffixes.

/// Lazy Cartesian product, with the last child varying fastest. Factories must
/// return fresh, independent iterators over the same sequence on every call.
/// Children need not be Clone and their sequences are never materialized.
/// An empty child either empties the product or is omitted, according to
/// `ignore_empty`. Zero children or all-omitted children yield no items.
/// An infinite suffix starves earlier children; bound consumption explicitly.
///
/// ```
/// use timetuck::MoveProduct;
/// let choices = [vec![1, 2], vec![8, 9]];
/// let product = MoveProduct::new(2, false, |i| choices[i].iter().copied());
/// assert_eq!(product.collect::<Vec<_>>(),
///            vec![vec![1,8], vec![1,9], vec![2,8], vec![2,9]]);
/// ```
pub struct MoveProduct<F, I: Iterator> {
    factory: F,
    child_count: usize,
    ignore_empty: bool,
    initialized: bool,
    done: bool,
    indices: Vec<usize>,
    iterators: Vec<I>,
    current: Vec<I::Item>,
}
impl<F, I> MoveProduct<F, I>
where
    F: FnMut(usize) -> I,
    I: Iterator,
{
    /// Does not invoke the factory or poll any child until `next` is called.
    pub fn new(child_count: usize, ignore_empty: bool, factory: F) -> Self {
        Self {
            factory,
            child_count,
            ignore_empty,
            initialized: false,
            done: false,
            indices: Vec::new(),
            iterators: Vec::new(),
            current: Vec::new(),
        }
    }
}
impl<F, I> Iterator for MoveProduct<F, I>
where
    F: FnMut(usize) -> I,
    I: Iterator,
    I::Item: Clone,
{
    type Item = Vec<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if !self.initialized {
            self.initialized = true;
            for index in 0..self.child_count {
                let mut child = (self.factory)(index);
                if let Some(value) = child.next() {
                    self.indices.push(index);
                    self.iterators.push(child);
                    self.current.push(value);
                } else if !self.ignore_empty {
                    self.done = true;
                    return None;
                }
            }
            if self.current.is_empty() {
                self.done = true;
                return None;
            }
            return Some(self.current.clone());
        }
        for index in (0..self.iterators.len()).rev() {
            if let Some(value) = self.iterators[index].next() {
                self.current[index] = value;
                for suffix in index + 1..self.iterators.len() {
                    self.iterators[suffix] = (self.factory)(self.indices[suffix]);
                    let Some(value) = self.iterators[suffix].next() else {
                        // A factory violating the stable-sequence contract must
                        // still terminate rather than returning stale items.
                        self.done = true;
                        return None;
                    };
                    self.current[suffix] = value;
                }
                return Some(self.current.clone());
            }
        }
        self.done = true;
        None
    }
}
impl<F, I> std::iter::FusedIterator for MoveProduct<F, I>
where
    F: FnMut(usize) -> I,
    I: Iterator,
    I::Item: Clone,
{
}
