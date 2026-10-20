//! Reuse a removed frequency-one node for a different absent primitive key.
use super::*;

// Detach the physical leftmost node, preserving its key/value owners. The
// detached node is a singleton; its former right subtree stays in the tree.
fn detach_first<K, V>(root: &mut Link<K, V>) -> Arc<Node<K, V>> {
    if root.as_ref().expect("nonempty successor").left.is_some() {
        let node = Arc::make_mut(root.as_mut().unwrap());
        let detached = detach_first(&mut node.left);
        refresh(node);
        balance(root);
        detached
    } else {
        let mut detached = root.take().expect("nonempty successor");
        let node = Arc::make_mut(&mut detached);
        *root = node.right.take();
        node.height = 1;
        node.size = 1;
        detached
    }
}

fn detach<K, V>(root: &mut Link<K, V>, path: &[Ordering]) -> Arc<Node<K, V>> {
    if let Some((&direction, tail)) = path.split_first() {
        let node = Arc::make_mut(root.as_mut().expect("prepared removal path"));
        let detached = detach(
            if direction == Ordering::Less {
                &mut node.left
            } else {
                &mut node.right
            },
            tail,
        );
        refresh(node);
        balance(root);
        return detached;
    }
    let node = root.as_ref().expect("prepared removal");
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
    // The target keeps the successor's exact representative. The physical
    // successor node now owns the requested entry and can be retargeted.
    std::mem::swap(&mut node.key, &mut removed.key);
    std::mem::swap(&mut node.value, &mut removed.value);
    refresh(node);
    balance(root);
    detached
}

fn put_node<K, V>(root: &mut Link<K, V>, path: &[Ordering], detached: Arc<Node<K, V>>) {
    if let Some((&direction, tail)) = path.split_first() {
        let node = Arc::make_mut(root.as_mut().expect("prepared insertion path"));
        put_node(
            if direction == Ordering::Less {
                &mut node.left
            } else {
                &mut node.right
            },
            tail,
            detached,
        );
        refresh(node);
        balance(root);
    } else {
        debug_assert!(root.is_none());
        *root = Some(detached);
    }
}

impl SnapshotMap<i64, usize> {
    // Fully checked entry point: false leaves the exact root unchanged. The
    // post-deletion search is intentionally restricted to primitive comparisons.
    // No user callback, key/value destructor or checked arithmetic follows erase.
    pub(crate) fn retarget_singleton(&mut self, old: i64, new: i64) -> bool {
        let (path, count) = locate(&self.root, &old);
        if count.map(Arc::as_ref) != Some(&1) || self.contains_key(&new) {
            return false;
        }
        self.preserve_journal();
        let mut detached = detach(&mut self.root, path.as_slice());
        let (path, found) = locate(&self.root, &new);
        debug_assert!(found.is_none());
        *Arc::make_mut(&mut Arc::make_mut(&mut detached).key) = new;
        put_node(&mut self.root, path.as_slice(), detached);
        true
    }
}

#[cfg(test)]
mod tests;
