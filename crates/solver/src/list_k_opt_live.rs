//! Explicit live-position binding for native physical route reconnections.
//! This does not infer the Community virtual-seam or partition policy.
use crate::{
    ListAssignment, ListKOptError, ListKOptLocalCyclePlan, ListMove, ListPosition, ListProblem,
};

/// Retains cut endpoint identities and row shape, while allowing interiors to
/// change between selection and execution. Current payloads occupy the prepared
/// physical ports. Target row lengths remain an explicit caller policy.
#[derive(Clone, Debug)]
pub struct ListKOptLivePlan<V> {
    cuts: Vec<(ListPosition, ListPosition)>,
    partners: Vec<(usize, usize)>,
    endpoints: Vec<(ListPosition, V)>,
    shape: Vec<(usize, usize, usize)>,
}
impl<V: Eq + Clone> ListKOptLivePlan<V> {
    pub fn prepare(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        cuts: &[(ListPosition, ListPosition)],
        partners: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        let physical = ListKOptLocalCyclePlan::prepare(problem, before, cuts, partners)?;
        Ok(Self {
            cuts: cuts.to_vec(),
            partners: partners.to_vec(),
            endpoints: cuts
                .iter()
                .flat_map(|&(a, b)| [a, b])
                .map(|p| (p, before.lists()[p.entity][p.index].clone()))
                .collect(),
            shape: physical
                .selected_entities()
                .iter()
                .map(|&e| {
                    (
                        e,
                        before.lists()[e].len(),
                        problem.entities()[e].pinned_prefix(),
                    )
                })
                .collect(),
        })
    }
    /// Reject moved endpoints, changed row lengths/pins and invalid assignments
    /// before producing any edits. Interior membership is read from `current`.
    pub fn movement(
        &self,
        problem: &ListProblem<V>,
        current: &ListAssignment<V>,
        first: ListPosition,
        reverse: bool,
        lengths: &[usize],
    ) -> Result<ListMove<V>, ListKOptError> {
        problem.validate(current)?;
        if self.shape.iter().any(|&(e, n, p)| {
            problem
                .entities()
                .get(e)
                .is_none_or(|meta| meta.pinned_prefix() != p || current.lists()[e].len() != n)
        }) || self
            .endpoints
            .iter()
            .any(|(p, v)| current.lists()[p.entity][p.index] != *v)
        {
            return Err(ListKOptError::Partition);
        }
        ListKOptLocalCyclePlan::prepare(problem, current, &self.cuts, &self.partners)?
            .movement(problem, current, first, reverse, lengths)
    }
}
