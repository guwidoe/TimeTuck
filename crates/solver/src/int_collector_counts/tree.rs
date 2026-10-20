//! Private persistent frequency tree with primitive keys and counts inline.
//!
//! Unlike generic snapshot maps, these nodes need no separate ownership of
//! keys/values: neither primitive has callbacks or observable address identity.
use std::{cmp::Ordering, sync::Arc};

type Link = Option<Arc<Node>>;
#[derive(Clone, Debug)]
struct Node {
    key: i64,
    count: usize,
    left: Link,
    right: Link,
    height: usize,
    size: usize,
}
#[derive(Clone, Debug, Default)]
pub(crate) struct CountsTree {
    root: Link,
}
fn height(root: &Link) -> usize {
    root.as_ref().map_or(0, |n| n.height)
}
fn size(root: &Link) -> usize {
    root.as_ref().map_or(0, |n| n.size)
}
fn refresh(node: &mut Node) {
    node.height = 1 + height(&node.left).max(height(&node.right));
    node.size = 1 + size(&node.left) + size(&node.right);
}
fn rotate_left(root: &mut Link) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).right.take().expect("right pivot");
    let middle = Arc::make_mut(&mut pivot).left.take();
    let node = Arc::make_mut(&mut old);
    node.right = middle;
    refresh(node);
    let node = Arc::make_mut(&mut pivot);
    node.left = Some(old);
    refresh(node);
    *root = Some(pivot);
}
fn rotate_right(root: &mut Link) {
    let mut old = root.take().expect("rotation root");
    let mut pivot = Arc::make_mut(&mut old).left.take().expect("left pivot");
    let middle = Arc::make_mut(&mut pivot).right.take();
    let node = Arc::make_mut(&mut old);
    node.left = middle;
    refresh(node);
    let node = Arc::make_mut(&mut pivot);
    node.right = Some(old);
    refresh(node);
    *root = Some(pivot);
}
fn balance(root: &mut Link) {
    let Some(node) = root else { return };
    if height(&node.left) > height(&node.right) + 1 {
        let left = node.left.as_ref().unwrap();
        if height(&left.right) > height(&left.left) {
            rotate_left(&mut Arc::make_mut(node).left);
        }
        rotate_right(root);
    } else if height(&node.right) > height(&node.left) + 1 {
        let right = node.right.as_ref().unwrap();
        if height(&right.left) > height(&right.right) {
            rotate_right(&mut Arc::make_mut(node).right);
        }
        rotate_left(root);
    }
}
fn increment(root: &mut Link, key: i64) {
    let Some(owner) = root else {
        *root = Some(Arc::new(Node {
            key,
            count: 1,
            left: None,
            right: None,
            height: 1,
            size: 1,
        }));
        return;
    };
    let node = Arc::make_mut(owner);
    match key.cmp(&node.key) {
        Ordering::Less => increment(&mut node.left, key),
        Ordering::Greater => increment(&mut node.right, key),
        Ordering::Equal => {
            // The owning collector preflights its total occurrence count.
            node.count += 1;
            return;
        }
    }
    refresh(node);
    balance(root);
}
fn set_count(root: &mut Link, key: i64, count: usize) {
    let mut node = Arc::make_mut(root.as_mut().expect("present frequency"));
    loop {
        let child = match key.cmp(&node.key) {
            Ordering::Less => &mut node.left,
            Ordering::Greater => &mut node.right,
            Ordering::Equal => {
                node.count = count;
                return;
            }
        };
        node = Arc::make_mut(child.as_mut().expect("present frequency path"));
    }
}
// Return the physical node so a singleton rename can reuse its allocation.
fn detach_first(root: &mut Link) -> Arc<Node> {
    if root.as_ref().unwrap().left.is_some() {
        let node = Arc::make_mut(root.as_mut().unwrap());
        let detached = detach_first(&mut node.left);
        refresh(node);
        balance(root);
        detached
    } else {
        let mut detached = root.take().unwrap();
        let node = Arc::make_mut(&mut detached);
        *root = node.right.take();
        node.height = 1;
        node.size = 1;
        detached
    }
}
fn detach(root: &mut Link, key: i64) -> Arc<Node> {
    let direction = key.cmp(&root.as_ref().expect("present frequency").key);
    if direction != Ordering::Equal {
        let node = Arc::make_mut(root.as_mut().unwrap());
        let detached = detach(
            if direction == Ordering::Less {
                &mut node.left
            } else {
                &mut node.right
            },
            key,
        );
        refresh(node);
        balance(root);
        return detached;
    }
    let node = root.as_ref().unwrap();
    if node.left.is_none() || node.right.is_none() {
        let mut detached = root.take().unwrap();
        let node = Arc::make_mut(&mut detached);
        *root = if node.left.is_none() {
            node.right.take()
        } else {
            node.left.take()
        };
        node.height = 1;
        node.size = 1;
        return detached;
    }
    let node = Arc::make_mut(root.as_mut().unwrap());
    let mut detached = detach_first(&mut node.right);
    let removed = Arc::make_mut(&mut detached);
    std::mem::swap(&mut node.key, &mut removed.key);
    std::mem::swap(&mut node.count, &mut removed.count);
    refresh(node);
    balance(root);
    detached
}
fn put_node(root: &mut Link, detached: Arc<Node>) {
    let Some(owner) = root else {
        *root = Some(detached);
        return;
    };
    let node = Arc::make_mut(owner);
    let child = if detached.key < node.key {
        &mut node.left
    } else {
        &mut node.right
    };
    put_node(child, detached);
    refresh(node);
    balance(root);
}
impl CountsTree {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn len(&self) -> usize {
        size(&self.root)
    }
    pub(super) fn get(&self, key: i64) -> Option<usize> {
        let mut next = self.root.as_deref();
        while let Some(node) = next {
            match key.cmp(&node.key) {
                Ordering::Less => next = node.left.as_deref(),
                Ordering::Greater => next = node.right.as_deref(),
                Ordering::Equal => return Some(node.count),
            }
        }
        None
    }
    pub(super) fn increment(&mut self, key: i64) {
        increment(&mut self.root, key);
    }
    pub(super) fn insert_count(&mut self, key: i64, count: usize) {
        increment(&mut self.root, key);
        set_count(&mut self.root, key, count);
    }
    pub(super) fn remove(&mut self, key: i64) -> bool {
        let Some(count) = self.get(key) else {
            return false;
        };
        if count > 1 {
            set_count(&mut self.root, key, count - 1);
        } else {
            drop(detach(&mut self.root, key));
        }
        true
    }
    pub(super) fn retarget_singleton(&mut self, old: i64, new: i64) -> bool {
        if self.get(old) != Some(1) || self.get(new).is_some() {
            return false;
        }
        let mut detached = detach(&mut self.root, old);
        Arc::make_mut(&mut detached).key = new;
        put_node(&mut self.root, detached);
        true
    }
    pub(super) fn min(&self) -> Option<i64> {
        let mut node = self.root.as_deref()?;
        while let Some(left) = node.left.as_deref() {
            node = left;
        }
        Some(node.key)
    }
    pub(super) fn max(&self) -> Option<i64> {
        let mut node = self.root.as_deref()?;
        while let Some(right) = node.right.as_deref() {
            node = right;
        }
        Some(node.key)
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (i64, usize)> + '_ {
        let mut next = self.root.as_deref();
        let mut stack = Vec::new();
        std::iter::from_fn(move || {
            while let Some(node) = next {
                stack.push(node);
                next = node.left.as_deref();
            }
            let node = stack.pop()?;
            next = node.right.as_deref();
            Some((node.key, node.count))
        })
    }
}
impl PartialEq for CountsTree {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for CountsTree {}

#[cfg(test)]
mod tests;
