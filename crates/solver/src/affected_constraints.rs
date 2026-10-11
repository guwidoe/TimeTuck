//! Pass-local union of sorted dependent-constraint ID slices.

pub(crate) struct AffectedConstraints<'a> {
    indices: Vec<usize>,
    seen: Option<Vec<u64>>,
    previous: &'a [usize],
    constraint_count: usize,
}

impl<'a> AffectedConstraints<'a> {
    #[inline]
    pub(crate) fn new(constraint_count: usize) -> Self {
        Self {
            indices: Vec::new(),
            seen: None,
            previous: &[],
            constraint_count,
        }
    }

    #[inline]
    pub(crate) fn extend(&mut self, ids: &'a [usize]) {
        // Consecutive writes/scopes often share their entire dependency slice.
        // Those IDs have already been collected; no membership table is needed.
        if ids.is_empty() || ids == self.previous {
            return;
        }
        let volume = self.indices.len().saturating_add(ids.len());
        if self.seen.is_none()
            && volume >= 256
            && volume >= self.constraint_count.div_ceil(8)
            && overlaps_sample(ids, self.previous)
        {
            // Activate only after actual duplicate IDs were observed. The
            // volume guard bounds the bitmap relative to collected input, so
            // sparse scopes in a huge network do not allocate a huge table.
            let mut seen = vec![0; self.constraint_count.div_ceil(64)];
            self.indices.retain(|&id| mark(&mut seen, id));
            self.seen = Some(seen);
        }
        self.previous = ids;
        if let Some(seen) = &mut self.seen {
            for &id in ids {
                if mark(seen, id) {
                    self.indices.push(id);
                }
            }
        } else {
            self.indices.extend_from_slice(ids);
        }
    }

    #[inline]
    pub(crate) fn into_sorted(mut self) -> Vec<usize> {
        self.indices.sort_unstable();
        if self.seen.is_none() {
            self.indices.dedup();
        }
        self.indices
    }
}

// Network registration keeps each dependency slice sorted and unique. Sampling
// is only an activation hint: an undetected overlap still uses exact sort/dedup.
#[inline]
fn overlaps_sample(left: &[usize], right: &[usize]) -> bool {
    if right.is_empty() || left[0] > right[right.len() - 1] || right[0] > left[left.len() - 1] {
        return false;
    }
    [left[0], left[left.len() / 2], left[left.len() - 1]]
        .iter()
        .any(|id| right.binary_search(id).is_ok())
}

#[inline]
fn mark(seen: &mut [u64], id: usize) -> bool {
    let word = &mut seen[id / 64];
    let bit = 1 << (id % 64);
    let fresh = *word & bit == 0;
    *word |= bit;
    fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_matches_sorted_dedup_across_word_boundaries_and_late_overlap() {
        for count in [0usize, 1, 63, 64, 65, 256, 4096] {
            for seed in 0..16 {
                let inputs: Vec<Vec<_>> = (0..40)
                    .map(|group| {
                        (0..count)
                            .filter(|id| {
                                (id * 17 + group * 31 + seed).is_multiple_of(group % 5 + 2)
                            })
                            .collect()
                    })
                    .collect();
                let mut actual = AffectedConstraints::new(count);
                let mut expected: Vec<usize> = Vec::new();
                for ids in &inputs {
                    actual.extend(ids);
                    actual.extend(ids);
                    actual.extend(&[]);
                    expected.extend(ids);
                }
                expected.sort_unstable();
                expected.dedup();
                assert_eq!(actual.into_sorted(), expected);
            }
        }
    }

    #[test]
    fn bitmap_requires_observed_overlap_and_enough_input() {
        let inputs: Vec<Vec<_>> = (0..16)
            .map(|group| (0..4096).filter(|id| id % 16 == group).collect())
            .collect();
        let mut direct = AffectedConstraints::new(4096);
        for ids in &inputs {
            direct.extend(ids);
        }
        assert!(
            direct.seen.is_none(),
            "interleaved disjoint inputs stay direct"
        );
        assert_eq!(direct.into_sorted(), (0..4096).collect::<Vec<_>>());

        let first: Vec<_> = (0..256).collect();
        let second: Vec<_> = (128..384).collect();
        let mut indexed = AffectedConstraints::new(4096);
        indexed.extend(&first);
        indexed.extend(&second);
        assert!(indexed.seen.is_some());
        assert_eq!(indexed.into_sorted(), (0..384).collect::<Vec<_>>());

        let mut sparse = AffectedConstraints::new(1_000_000);
        sparse.extend(&first);
        sparse.extend(&second);
        assert!(sparse.seen.is_none());
        assert_eq!(sparse.into_sorted(), (0..384).collect::<Vec<_>>());
    }
}
