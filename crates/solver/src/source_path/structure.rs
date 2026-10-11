//! Snapshot-dependent structure inspection over actual compiled source paths.
use super::{CompiledSourcePath, SourceMemberRole, SourceParent, SourcePathError};
use std::any::Any;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowDirection {
    Previous,
    Next,
}
/// Structural declaration shape, rather than proof of an acyclic runtime graph.
/// Directional links may still form loops; use shadow_consistency for that query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShadowStructure {
    Empty,
    Directional(ShadowDirection),
    /// One target entity type with zero or one directional-parent kind, plus
    /// groups/indirect references. The actual runtime graph may branch or cycle.
    SingleType {
        direction: Option<ShadowDirection>,
    },
    General,
}
/// Borrow a root snapshot and all its compiled declarative supplier sources.
/// An entity without declarative suppliers supplies an empty paths slice.
/// Omitted sources cannot contribute to inspection; provide the complete binding.
#[derive(Clone, Copy)]
pub struct ShadowSourceEntity<'a, 'r, 's> {
    pub entity: &'a dyn Any,
    pub paths: &'a [CompiledSourcePath<'r, 's>],
}
impl ShadowStructure {
    /// Inspect without invoking suppliers, terminal getters or cloning entities.
    /// Source links are declarations: null directional links still contribute.
    /// Group-only declarations contribute only when their group prefix resolves
    /// to members in this snapshot, even if later directional links are null.
    /// Entities with no declarative sources do not enlarge the target type set.
    /// Multiple target types or directional-parent kinds require General.
    ///
    /// The result belongs to this snapshot. Reinspect after groups or represented
    /// entity types change. Existing dependency resolution/evaluation is unchanged.
    pub fn inspect<'a, 'r, 's>(
        entities: impl IntoIterator<Item = ShadowSourceEntity<'a, 'r, 's>>,
    ) -> Result<Self, SourcePathError>
    where
        'r: 'a,
        's: 'r,
    {
        let mut target_type = None;
        let mut multiple_types = false;
        let mut previous = false;
        let mut next = false;
        let mut non_directional = false;
        for binding in entities {
            for path in binding.paths {
                // Validate every binding, including ones after a General result.
                if binding.entity.type_id() != path.root.id() {
                    return Err(SourcePathError {
                        path: path.path.clone(),
                        owner: path.root,
                        member: String::new(),
                        reason: "wrong traversal root type".into(),
                    });
                }
                match target_type {
                    None => target_type = Some(path.root),
                    Some(first) if first != path.root => multiple_types = true,
                    _ => (),
                }
                if let Some(group) = path.group {
                    // Only resolve the group prefix; parent links after it may be null.
                    if path
                        .walk(binding.entity, path.root, 0..group + 1)?
                        .is_empty()
                    {
                        continue;
                    }
                    non_directional = true;
                } else if matches!(
                    path.parent,
                    SourceParent::Indirect | SourceParent::ListElement
                ) {
                    non_directional = true;
                }
                for member in &path.steps {
                    previous |= member.role == SourceMemberRole::Previous;
                    next |= member.role == SourceMemberRole::Next;
                }
            }
        }
        if target_type.is_none() {
            return Ok(Self::Empty);
        }
        if multiple_types || (previous && next) {
            return Ok(Self::General);
        }
        let direction = if previous {
            Some(ShadowDirection::Previous)
        } else if next {
            Some(ShadowDirection::Next)
        } else {
            None
        };
        Ok(match (direction, non_directional) {
            (Some(direction), false) => Self::Directional(direction),
            _ => Self::SingleType { direction },
        })
    }
}
