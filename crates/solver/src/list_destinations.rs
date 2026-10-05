//! Insertion positions for custom list-move providers.
use crate::{
    AdditiveScore, DatasetEnumerationOptions, EnumeratedDataset, ListAssignment, ListError,
    ListPosition, ListProblem, ListStreamScoringError, ListStreamSources, StreamGraphBuilder,
};

/// A live list-position view. Its optional None member means unassignment.
pub type ListDestinationDataset = EnumeratedDataset<(usize, usize), ListPosition>;

/// Enumerate every insertion slot once, in entity/index order, optionally preceded
/// by None for unassignment. Whole-entity pins exclude all slots; prefix pins allow
/// insertion at or after the prefix, including an empty movable suffix.
///
/// Positions refer to the snapshot before removing source values. A move provider
/// must adjust same-list destinations after removal and check the receiving range
/// for its particular values. Ranges do not remove slots from this value-independent
/// enumeration. Explicit unassignment requires an optional-list problem. Validation
/// happens before the iterator is returned.
pub fn list_destinations<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    include_unassigned: bool,
) -> Result<impl Iterator<Item = Option<ListPosition>> + 'a, ListError> {
    problem.validate(assignment)?;
    if include_unassigned && !problem.allows_unassigned() {
        return Err(ListError::UnassignmentNotAllowed);
    }
    let positions = problem
        .entities()
        .iter()
        .enumerate()
        .filter(|(_, entity)| !entity.is_pinned())
        .flat_map(move |(entity, metadata)| {
            (metadata.pinned_prefix()..=assignment.lists()[entity].len())
                .map(move |index| Some(ListPosition { entity, index }))
        });
    Ok(include_unassigned
        .then_some(None)
        .into_iter()
        .chain(positions))
}

/// Capture insertion slots from the same problem/sources used by a list or mixed
/// stream score director. Touched entity rows update their positions; trials,
/// rollback and owned snapshots use the ordinary graph journal. None, when enabled,
/// is a permanent dataset-view member even if every entity is pinned.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])])?;
/// let mut b = StreamGraphBuilder::<SimpleScore>::new();
/// let sources = ListStreamSources::new(&mut b);
/// let positions = list_destination_dataset(&mut b, &sources, &p, false)?;
/// let d = ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment())?;
/// assert_eq!(positions.len(d.graph())?, 2);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn list_destination_dataset<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'_, S>,
    sources: &ListStreamSources<V>,
    problem: &ListProblem<V>,
    include_unassigned: bool,
) -> Result<ListDestinationDataset, ListStreamScoringError> {
    if include_unassigned && !problem.allows_unassigned() {
        return Err(ListError::UnassignmentNotAllowed.into());
    }
    let pins: Vec<_> = problem
        .entities()
        .iter()
        .map(|e| (e.is_pinned(), e.pinned_prefix()))
        .collect();
    let positions = builder.flat_map(sources.entities.port(), move |(entity, list)| {
        let (pinned, prefix) = pins[*entity];
        let entity = *entity;
        (!pinned)
            .then_some(prefix..=list.len())
            .into_iter()
            .flatten()
            .map(move |index| ListPosition { entity, index })
    })?;
    Ok(EnumeratedDataset::capture(
        builder,
        &positions,
        DatasetEnumerationOptions {
            include_null: include_unassigned,
            exclude_pinned: false,
        },
        |_| false,
    )?)
}
