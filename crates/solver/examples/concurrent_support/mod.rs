//! Adapted Community concurrent domain/supplier/scoring fixtures, Apache-2.0.
//! See compat/provenance.json. Times are minute offsets from 2025-01-01 09:00.
use timetuck::*;
type Key = (usize, u8); // ready, shared start, finish
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Visit {
    pub owner: Option<usize>,
    pub index: Option<usize>,
    pub previous: Option<usize>,
    pub next: Option<usize>,
    pub cascading: Option<i64>,
    pub ready: Option<i64>,
    pub start: Option<i64>,
    pub finish: Option<i64>,
    pub inconsistent: bool,
}
#[derive(Clone)]
pub struct Schedule {
    plan: DependencyGraph<Key>,
    alignment: Vec<(Key, Key)>,
    count: usize,
    groups: Vec<usize>,
}
impl Schedule {
    pub fn new(groups: &[Vec<usize>], count: usize) -> Self {
        let mut seen = vec![false; count];
        let mut alignment = vec![];
        let mut roots: Vec<_> = (0..count).collect();
        for group in groups {
            assert!(!group.is_empty());
            for &v in group {
                assert!(v < count && !seen[v]);
                seen[v] = true;
                roots[v] = group[0];
                alignment.push(((v, 1), (group[0], 1)));
            }
        }
        let plan = DependencyGraph::new((0..count).flat_map(|v| {
            [
                ((v, 0), vec![]),
                ((v, 1), vec![(v, 0)]),
                ((v, 2), vec![(v, 1)]),
            ]
        }))
        .unwrap();
        Self {
            plan,
            alignment,
            count,
            groups: roots,
        }
    }
    pub fn project(&self, lists: &[Vec<usize>]) -> (Vec<Visit>, Vec<usize>) {
        let mut visits = vec![
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
            self.count
        ];
        let mut edges = vec![];
        for (e, list) in lists.iter().enumerate() {
            for (i, &v) in list.iter().enumerate() {
                let r = &mut visits[v];
                assert!(r.owner.is_none());
                r.owner = Some(e);
                r.index = Some(i);
                r.previous = i.checked_sub(1).map(|j| list[j]);
                r.next = list.get(i + 1).copied();
                r.cascading = Some(i as i64 * 1440);
                if let Some(p) = r.previous {
                    edges.push(((v, 0), (p, 2)));
                }
            }
        }
        let mut calls = vec![0; self.count];
        let values = self
            .plan
            .evaluate_aligned(&edges, &self.alignment, |members, inputs| {
                let (v, phase) = members[0];
                match phase {
                    0 => {
                        if visits[v].owner.is_none() {
                            None
                        } else if visits[v].previous.is_none() {
                            Some(0)
                        } else {
                            inputs[0].1.value.map(|t| t + 30)
                        }
                    }
                    1 => {
                        calls[v] += 1;
                        inputs.iter().filter_map(|(_, v)| v.value).max()
                    }
                    2 => inputs[0].1.value.map(|t| t + 30),
                    _ => unreachable!(),
                }
            })
            .unwrap();
        for (v, row) in visits.iter_mut().enumerate() {
            row.inconsistent = (0..3).any(|p| values[&(v, p)].inconsistent);
            // Entity inconsistency does not clear unrelated ready values.
            // A first visit can retain ready=0 while its shared start is cyclic.
            row.ready = values[&(v, 0)].value;
            row.start = values[&(v, 1)].value;
            row.finish = values[&(v, 2)].value;
        }
        (visits, calls)
    }
    // Independent closure/relaxation formulation of the fixture's expected
    // inconsistency and score. This does not call the dependency evaluator.
    pub fn reference_score(&self, lists: &[Vec<usize>]) -> HardSoftScore {
        let n = self.count;
        let mut assigned = vec![false; n];
        let mut links = vec![vec![false; n]; n];
        for list in lists {
            for (i, &v) in list.iter().enumerate() {
                assigned[v] = true;
                if i > 0 {
                    links[self.groups[v]][self.groups[list[i - 1]]] = true;
                }
            }
        }
        let mut reach = links.clone();
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        let bad: Vec<_> = (0..n)
            .map(|i| {
                (0..n).any(|k| reach[k][k] && (self.groups[i] == k || reach[self.groups[i]][k]))
            })
            .collect();
        let mut starts = vec![None; n];
        for _ in 0..n {
            for i in 0..n {
                if self.groups[i] != i || bad[i] {
                    continue;
                }
                let active = (0..n).any(|v| self.groups[v] == i && assigned[v]);
                if active && (0..n).filter(|&j| links[i][j]).all(|j| starts[j].is_some()) {
                    starts[i] = Some(
                        (0..n)
                            .filter(|&j| links[i][j])
                            .map(|j| starts[j].unwrap() + 60)
                            .max()
                            .unwrap_or(0),
                    );
                }
            }
        }
        HardSoftScore::new(
            -(bad.iter().filter(|&&v| v).count() as i64),
            -(0..n)
                .filter(|&v| assigned[v] && !bad[v])
                .map(|v| starts[self.groups[v]].unwrap() + 30)
                .sum::<i64>(),
        )
    }
    pub fn score(&self, lists: &[Vec<usize>]) -> HardSoftScore {
        let (visits, _) = self.project(lists);
        HardSoftScore::new(
            -(visits.iter().filter(|v| v.inconsistent).count() as i64),
            -visits
                .iter()
                .filter(|v| v.owner.is_some() && !v.inconsistent)
                .filter_map(|v| v.finish)
                .sum::<i64>(),
        )
    }
    pub fn graph(
        &self,
        owners: usize,
    ) -> (
        StreamGraph<'static, HardSoftScore>,
        ListStreamSources<usize>,
        StreamConstraint<(), Vec<Visit>>,
    ) {
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        let lists = b
            .collect_list(s.entities.port(), |row| row.clone())
            .unwrap();
        let fixture = self.clone();
        let projected = b
            .map(&lists, move |rows| {
                let mut lists = vec![vec![]; owners];
                for (e, list) in rows.iter() {
                    lists[*e] = list.clone();
                }
                // Sources arrive owner by owner; a transfer may temporarily
                // appear in both lists. Normalize this private intermediate
                // snapshot. Validated final assignments have unique ownership.
                let mut seen = std::collections::BTreeSet::new();
                for list in &mut lists {
                    list.retain(|v| seen.insert(*v));
                }
                let visits = fixture.project(&lists).0;
                let score = HardSoftScore::new(
                    -(visits.iter().filter(|v| v.inconsistent).count() as i64),
                    -visits
                        .iter()
                        .filter(|v| v.owner.is_some() && !v.inconsistent)
                        .filter_map(|v| v.finish)
                        .sum::<i64>(),
                );
                assert_eq!(score, fixture.reference_score(&lists));
                visits
            })
            .unwrap();
        let handle = b
            .score("concurrent schedule", &projected, |visits| {
                HardSoftScore::new(
                    -(visits.iter().filter(|v| v.inconsistent).count() as i64),
                    -visits
                        .iter()
                        .filter(|v| v.owner.is_some() && !v.inconsistent)
                        .filter_map(|v| v.finish)
                        .sum::<i64>(),
                )
            })
            .unwrap();
        (b.build(), s, handle)
    }
}
