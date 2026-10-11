//! Bounded sampling of ordered list-value groups and eligible destinations.
use crate::{
    ListAssignment, ListDestinationError, ListMove, ListProblem, MixedAssignment, MixedMove,
    MixedProblem, Move, MoveProvider, SampleError, SampleRandom,
};
use std::{iter::FusedIterator, num::NonZeroUsize};

/// Per-step proposal budget and group-size bounds. Sizes are inclusive and are
/// clipped to movable registered values. A minimum above that count yields no
/// proposals. Attempts include rejected/no-op proposals, not just yielded moves.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatherSamplingConfig {
    minimum: usize,
    maximum: usize,
    pub attempts: u64,
    pub seed: u64,
    /// Add one unassignment destination; requires an optional-list model.
    pub unassignment: bool,
}
impl GatherSamplingConfig {
    pub fn new(
        minimum: usize,
        maximum: usize,
        attempts: u64,
        seed: u64,
    ) -> Result<Self, SampleError> {
        if minimum == 0 || minimum > maximum {
            return Err(SampleError::InvalidSizes { minimum, maximum });
        }
        Ok(Self {
            minimum,
            maximum,
            attempts,
            seed,
            unassignment: false,
        })
    }
    pub fn minimum(self) -> usize {
        self.minimum
    }
    pub fn maximum(self) -> usize {
        self.maximum
    }
}
fn draw(random: &mut SampleRandom, bound: usize) -> usize {
    if bound == 1 {
        0
    } else {
        random.index(NonZeroUsize::new(bound).expect("positive sample range"))
    }
}

/// Draw a group size uniformly, then distinct movable registry values in a
/// uniformly random order, then an eligible pre-removal insertion slot uniformly.
/// Optional unassignment is one additional slot. This distribution is NOT uniform
/// over final assignments; duplicate successors remain. Seeds reproduce native
/// draws, without promising Java random traces.
///
/// Full/prefix pins exclude source values and receiving slots. Range intersection
/// excludes incompatible receivers. Dependency failures and unchanged results
/// consume an attempt and yield nothing. Exhaustion can miss feasible moves.
/// Unassigned values can be gathered into a list, including on required partial
/// assignments. Unassignment is opt-in and requires an optional model.
///
/// Setup indexes receivers and movable identities without cloning values. Drawing
/// is lazy and keeps one permutation of eligible indices, not a factorial catalog.
/// Each proposal borrows its source and copies references only for touched rows.
/// Gather construction clones only the output values. Final pins and ranges are
/// checked on touched rows; dependency-enabled models still check the global graph.
/// Work is bounded by attempts; it is not constant time in problem size.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1,2,3], vec![ListEntity::new(vec![1,2,3])])?;
/// let a = p.initial_assignment();
/// let config = GatherSamplingConfig::new(2, 3, 20, 7)?;
/// assert!(sampled_list_gather_moves(&p, &a, config)?.count() <= 20);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn sampled_list_gather_moves<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    config: GatherSamplingConfig,
) -> Result<impl FusedIterator<Item = ListMove<V>> + 'a, ListDestinationError> {
    let (builder, destinations) =
        crate::list_gather::GatherBuilder::for_sampling(problem, assignment, config.unassignment)?;
    let locations = builder.locations();
    let eligible: Vec<_> = problem
        .values()
        .iter()
        .enumerate()
        .filter(|(id, _)| {
            locations[*id].is_none_or(|pos| {
                let owner = &problem.entities()[pos.entity];
                !owner.is_pinned() && pos.index >= owner.pinned_prefix()
            })
        })
        .collect();
    let maximum = config.maximum.min(eligible.len());
    let attempts = if maximum < config.minimum {
        0
    } else {
        config.attempts
    };
    let mut indices: Vec<_> = (0..eligible.len()).collect();
    let mut random = SampleRandom::new(config.seed);
    Ok((0..attempts)
        .filter_map(move |_| {
            let size = config.minimum + draw(&mut random, maximum - config.minimum + 1);
            // Partial Fisher-Yates, valid from any prior permutation. No rejected
            // duplicate draws and no reset/copy of the entire permutation per attempt.
            for i in 0..size {
                let selected = i + draw(&mut random, indices.len() - i);
                indices.swap(i, selected);
            }
            let borrowed: Vec<_> = indices[..size].iter().map(|&i| eligible[i].1).collect();
            let members: Vec<V> = borrowed.iter().map(|v| (*v).clone()).collect();
            let receiving = destinations
                .for_values(&members)
                .expect("registered members and bounded destinations");
            let destination = receiving.sample(&mut random)?;
            if let Some(position) = destination {
                if crate::list_gather_neighborhood::unchanged(assignment, &borrowed, position) {
                    return None;
                }
            } else if indices[..size]
                .iter()
                .all(|&i| builder.locations()[eligible[i].0].is_none())
            {
                return None;
            }
            builder.gather_move(&members, destination).ok()
        })
        .fuse())
}

/// Resample after each completed search step using seed + completed_steps
/// (wrapping). Works in list and mixed local search and custom phases. Mixed
/// candidates preserve basic variables; combine with other providers as needed.
/// Empty sampled scans end ordinary search, even if unseen feasible moves exist.
/// The caller must use valid assignments and enable unassignment only for an
/// optional-list model; incompatible inputs panic, as MoveProvider has no error
/// return. Use sampled_list_gather_moves directly for fallible setup.
#[derive(Clone, Copy, Debug)]
pub struct GatherSamplingProvider {
    pub config: GatherSamplingConfig,
}
impl GatherSamplingProvider {
    pub fn new(config: GatherSamplingConfig) -> Self {
        Self { config }
    }
    fn at_step(&self, step: u64) -> GatherSamplingConfig {
        GatherSamplingConfig {
            seed: self.config.seed.wrapping_add(step),
            ..self.config
        }
    }
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for GatherSamplingProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            sampled_list_gather_moves(p, a, self.at_step(step))
                .expect("valid gather sampling inputs"),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for GatherSamplingProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        p.validate(a).expect("valid mixed assignment");
        Box::new(
            sampled_list_gather_moves(p.lists(), a.lists(), self.at_step(step))
                .expect("valid gather sampling inputs")
                .map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                }),
        )
    }
}
