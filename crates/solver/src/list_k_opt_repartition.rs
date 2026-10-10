//! Explicit partitions over a reconnected combined cycle of movable suffixes.
use crate::list_k_opt::cycle_order;
use crate::list_k_opt_sampling::{draw_tour, feasible};
use crate::{
    ListAssignment, ListError, ListKOptError, ListKOptSampling, ListMove, ListPosition,
    ListProblem, SegmentedSlice,
};

fn members<'a, V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &'a ListAssignment<V>,
    entities: &[usize],
) -> Result<SegmentedSlice<'a, V>, ListKOptError> {
    let mut values = Vec::new();
    for (i, &e) in entities.iter().enumerate() {
        if entities[..i].contains(&e) {
            return Err(ListKOptError::Entities);
        }
        let meta = p.entities().get(e).ok_or(ListError::EntityIndex {
            entity: e,
            count: p.entities().len(),
        })?;
        if meta.is_pinned() {
            return Err(ListError::PinChanged { entity: e }.into());
        }
        values.push(&a.lists()[e][meta.pinned_prefix()..]);
    }
    if entities.is_empty() {
        return Err(ListKOptError::Entities);
    }
    Ok(SegmentedSlice::new(values))
}

// Construct current-index relocations; final-state range/pin validation is atomic.
// Source values are the assigned payloads, never registry replacements.
fn repartition<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    entities: &[usize],
    values: &SegmentedSlice<'_, V>,
    tour: &[usize],
    lengths: &[usize],
) -> Result<ListMove<V>, ListKOptError> {
    if lengths.len() != entities.len()
        || lengths.iter().try_fold(0usize, |n, &v| n.checked_add(v)) != Some(values.len())
    {
        return Err(ListKOptError::Partition);
    }
    let mut target = std::collections::BTreeMap::new();
    let mut offset = 0;
    for (&e, &length) in entities.iter().zip(lengths) {
        let prefix = &a.lists()[e][..p.entities()[e].pinned_prefix()];
        let row = prefix
            .iter()
            .chain(
                tour[offset..offset + length]
                    .iter()
                    .map(|&i| values.get(i).expect("tour index")),
            )
            .collect();
        target.insert(e, row);
        offset += length;
    }
    // The validated source and complete tour retain membership and registry
    // identity. Check borrowed final rows before spending work on relocations;
    // entity-order range diagnostics and global dependency checks are shared
    // with ordinary move validation. No assigned payload is cloned.
    p.validate_permuted_rows(a, target)?;
    // Track integer suffix identities while constructing current-index moves.
    let mut current = Vec::with_capacity(entities.len());
    let mut positions = Vec::with_capacity(values.len());
    for (row, &entity) in entities.iter().enumerate() {
        let length = a.lists()[entity].len() - p.entities()[entity].pinned_prefix();
        let start = positions.len();
        current.push((start..start + length).collect::<Vec<_>>());
        positions.extend((0..length).map(|index| (row, index)));
    }
    let mut moves = Vec::new();
    let mut offset = 0;
    for (row, (&entity, &length)) in entities.iter().zip(lengths).enumerate() {
        for (index, &value) in tour[offset..offset + length].iter().enumerate() {
            let (from_row, from_index) = positions[value];
            if (from_row, from_index) == (row, index) {
                continue;
            }
            current[from_row].remove(from_index);
            for (index, &shifted) in current[from_row].iter().enumerate().skip(from_index) {
                positions[shifted] = (from_row, index);
            }
            current[row].insert(index, value);
            for (index, &shifted) in current[row].iter().enumerate().skip(index) {
                positions[shifted] = (row, index);
            }
            moves.push(ListMove::Change {
                source: ListPosition {
                    entity: entities[from_row],
                    index: p.entities()[entities[from_row]].pinned_prefix() + from_index,
                },
                destination: ListPosition {
                    entity,
                    index: p.entities()[entity].pinned_prefix() + index,
                },
            });
        }
        offset += length;
    }
    debug_assert!(current.iter().flatten().copied().eq(tour.iter().copied()));
    Ok(ListMove::Composite(moves))
}
impl<V: Eq + Clone> ListProblem<V> {
    /// Reconnect the combined cycle of the selected movable suffixes, then divide
    /// it into explicit destination suffix lengths. `entities` defines both the
    /// concatenation and destination order; entries must be distinct and unpinned.
    /// Empty suffixes are allowed. Edges and `(first, successor)` use zero-based
    /// indices in that concatenation, including its single wrap edge. This ordered
    /// pair chooses the new starting identity and direction before partitioning.
    ///
    /// Lengths must sum to the original combined length. Prefixes, unselected lists,
    /// missing identities and assigned payloads are preserved. Ranges are checked
    /// at the final state. The ordinary composite is bound to this input snapshot
    /// and supports existing scoring and rollback. This explicit partition contract
    /// does not reproduce Java's implicit multi-entity boundary/orientation choices.
    ///
    /// ```
    /// use timetuck::*;
    /// let p=ListProblem::new((0..6).collect(),vec![ListEntity::new((0..3).collect()),ListEntity::new((3..6).collect())])?;
    /// let mut a=p.initial_assignment();
    /// let m=p.k_opt_repartition_move(&a,&[0,1],&[(1,2),(4,5)],&[(1,4),(2,5)],(0,1),&[3,3])?;
    /// { let trial=p.apply(&mut a,&m)?;
    ///   assert_eq!(trial.lists(), &[vec![0,1,4],vec![3,2,5]]); }
    /// assert_eq!(a,p.initial_assignment());
    /// # Ok::<(),Box<dyn std::error::Error>>(())
    /// ```
    pub fn k_opt_repartition_move(
        &self,
        before: &ListAssignment<V>,
        entities: &[usize],
        removed: &[(usize, usize)],
        added: &[(usize, usize)],
        orientation: (usize, usize),
        lengths: &[usize],
    ) -> Result<ListMove<V>, ListKOptError> {
        self.validate(before)?;
        let values = members(self, before, entities)?;
        let tour = cycle_order(
            0,
            values.len(),
            removed,
            added,
            orientation.0,
            orientation.1,
        )?;
        repartition(self, before, entities, &values, &tour, lengths)
    }
}

/// Bounded combined-cycle proposals over all unpinned entities, including empty
/// destinations. Choose k uniformly among feasible configured sizes, random cuts
/// and reconnections, a uniform cycle rotation, and a uniformly sampled weak composition of the value count
/// into suffix lengths. Reject retained cut edges, invalid receiving ranges or
/// dependency cycles.
/// Proposal counts are bounded; yields may repeat and final valid moves are not
/// uniformly distributed. No Java sampling or implicit partition parity is claimed.
///
/// Storage is linear in values plus entities. Range validation and current-index
/// relocation construction may be quadratic. An empty/impossible combined cycle
/// returns immediately, including with a maximal attempt budget. Zero attempts
/// still validates the input. Missing values remain missing.
pub fn sample_list_k_opt_repartitions<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptSampling,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    p.validate(a)?;
    Ok(ordered(p, a, config, 0..p.entities().len()))
}
pub(crate) fn ordered<'a, V: Eq + Clone>(
    p: &'a ListProblem<V>,
    a: &'a ListAssignment<V>,
    config: ListKOptSampling,
    order: impl Iterator<Item = usize>,
) -> impl Iterator<Item = ListMove<V>> + 'a {
    let entities: Vec<_> = order.filter(|&e| !p.entities()[e].is_pinned()).collect();
    let values = SegmentedSlice::new(
        entities
            .iter()
            .map(|&e| &a.lists()[e][p.entities()[e].pinned_prefix()..]),
    );
    let n = values.len();
    let sizes: Vec<_> = (config.sizes().minimum()..=config.sizes().maximum().min(n))
        .filter(|&k| feasible(n, k))
        .collect();
    let mut random = crate::random::SplitMix64::new(config.seed());
    (0..if sizes.is_empty() {
        0
    } else {
        config.attempts()
    })
        .filter_map(move |_| {
            let k = sizes[random.below(sizes.len() as u64) as usize];
            let mut tour = draw_tour(n, k, &mut random)?;
            tour.rotate_left(random.below(n as u64) as usize);
            let lengths = partition_lengths(n, entities.len(), &mut random);
            // Layout follows from a permutation/composition; model constraints
            // still filter receiving ranges and dependency cycles.
            match repartition(p, a, &entities, &values, &tour, &lengths) {
                Ok(m) => Some(m),
                Err(ListKOptError::List(ref error))
                    if matches!(
                        error.without_range_report(),
                        ListError::ValueOutsideEntityRange { .. }
                            | ListError::DependencyCycle { .. }
                    ) =>
                {
                    None
                }
                Err(e) => panic!("invalid generated repartition: {e}"),
            }
        })
}
// Uniform bars among stars: each weak composition corresponds to one subset.
fn partition_lengths(n: usize, count: usize, random: &mut crate::random::SplitMix64) -> Vec<usize> {
    let mut slots: Vec<_> = (0..n + count - 1).collect();
    for i in 0..count - 1 {
        let j = i + random.below((slots.len() - i) as u64) as usize;
        slots.swap(i, j);
    }
    slots.truncate(count - 1);
    slots.sort_unstable();
    let mut start = 0;
    let mut lengths = Vec::with_capacity(count);
    for bar in slots {
        lengths.push(bar - start);
        start = bar + 1;
    }
    lengths.push(n + count - 1 - start);
    lengths
}
