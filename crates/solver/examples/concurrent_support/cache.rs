//! Cached adaptation of Community concurrent test fixtures; Apache-2.0.
//! See compat/provenance.json. This is a second native evaluator for the public
//! comparison, independent of the full projection used by the scoring fixture.
use super::concurrent_support::Visit;
use timetuck::*;
type Key = (usize, u8);
#[derive(Clone)]
pub struct CachedSchedule {
    cache: DependencyCache<Key, i64>,
    previous: Vec<Visit>,
}
impl CachedSchedule {
    pub fn new(groups: &[Vec<usize>], count: usize) -> Self {
        let mut roots: Vec<_> = (0..count).collect();
        let mut seen = vec![false; count];
        for group in groups {
            assert!(!group.is_empty());
            for &v in group {
                assert!(v < count && !seen[v]);
                seen[v] = true;
                roots[v] = group[0];
            }
        }
        let mut topology =
            DependencyTopology::new((0..count).flat_map(|v| (0..3).map(move |p| (v, p)))).unwrap();
        let mut fixed = vec![];
        for v in 0..count {
            fixed.push(((v, 2), (v, 1)));
            if roots[v] == v {
                for (w, &root) in roots.iter().enumerate() {
                    if root == v {
                        fixed.push(((v, 1), (w, 0)));
                    }
                }
            } else {
                fixed.push(((v, 1), (roots[v], 1)));
            }
        }
        topology.update(&[], &fixed).unwrap();
        Self {
            cache: DependencyCache::new(topology, |_, _| None),
            previous: Self::metadata(&[], count),
        }
    }
    fn metadata(lists: &[Vec<usize>], count: usize) -> Vec<Visit> {
        let mut rows = vec![
            Visit {
                owner: None,
                index: None,
                previous: None,
                next: None,
                cascading: None,
                ready: None,
                start: None,
                finish: None,
                inconsistent: false
            };
            count
        ];
        for (e, list) in lists.iter().enumerate() {
            for (i, &v) in list.iter().enumerate() {
                assert!(rows[v].owner.is_none());
                rows[v].owner = Some(e);
                rows[v].index = Some(i);
                rows[v].previous = i.checked_sub(1).map(|j| list[j]);
                rows[v].next = list.get(i + 1).copied();
                rows[v].cascading = Some(i as i64 * 1440);
            }
        }
        rows
    }
    pub fn project(&mut self, lists: &[Vec<usize>]) -> (Vec<Visit>, usize) {
        let mut rows = Self::metadata(lists, self.previous.len());
        let mut removed = vec![];
        let mut added = vec![];
        let mut dirty = vec![];
        for (v, (old, new)) in self.previous.iter().zip(&rows).enumerate() {
            if old.previous != new.previous {
                if let Some(p) = old.previous {
                    removed.push(((v, 0), (p, 2)));
                }
                if let Some(p) = new.previous {
                    added.push(((v, 0), (p, 2)));
                }
            }
            if old.owner != new.owner || old.previous != new.previous {
                dirty.push((v, 0));
            }
        }
        let mut calls = 0;
        self.cache
            .update(&removed, &added, &dirty, |&(v, phase), ins| {
                calls += 1;
                match phase {
                    0 => {
                        if rows[v].owner.is_none() {
                            None
                        } else if rows[v].previous.is_none() {
                            Some(0)
                        } else {
                            ins[0].1.value.map(|t| t + 30)
                        }
                    }
                    1 => ins.iter().filter_map(|(_, v)| v.value).max(),
                    2 => ins[0].1.value.map(|t| t + 30),
                    _ => unreachable!(),
                }
            })
            .unwrap();
        for (v, row) in rows.iter_mut().enumerate() {
            row.ready = self.cache.values()[&(v, 0)].value;
            row.start = self.cache.values()[&(v, 1)].value;
            row.finish = self.cache.values()[&(v, 2)].value;
            row.inconsistent = (0..3).any(|p| self.cache.values()[&(v, p)].inconsistent);
        }
        self.previous = rows.clone();
        (rows, calls)
    }
}
