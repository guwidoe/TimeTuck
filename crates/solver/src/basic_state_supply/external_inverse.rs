//! Checked import of nullable external inverse collections before publication.
use super::*;
impl<'s, E: Ord + Clone + 'static, V: Ord + Clone + 'static> BasicStateSupply<'s, E, V> {
    fn read_external_inverse<'r>(
        &self,
        mut read: impl FnMut(&V) -> Option<&'r [E]>,
    ) -> Result<BTreeMap<V, Vec<E>>, BasicSupplyError<E, V>>
    where
        E: 'r,
    {
        let binding = self.binding.ok_or(BasicSupplyError::Metadata(
            "external inverse checking requires an inverse shadow binding",
        ))?;
        let rows = self.state.snapshot();
        self.validate(&rows)?;
        let mut collections = BTreeMap::new();
        for value in &self.values {
            let members = read(value).ok_or_else(|| BasicSupplyError::NullInverseCollection {
                entity: rows
                    .iter()
                    .find_map(|(e, v)| (v.as_ref() == Some(value)).then(|| e.clone())),
                value: value.clone(),
                variable: self.source().name().into(),
                inverse: binding.name().into(),
            })?;
            let mut unique = BTreeSet::new();
            for entity in members {
                if !unique.insert(entity.clone()) {
                    return Err(BasicSupplyError::DuplicateEntity(entity.clone()));
                }
            }
            collections.insert(value.clone(), unique.into_iter().collect());
        }
        Ok(collections)
    }
    /// Validate actual nullable external collection fields without publication.
    /// Every value in the fixed universe is read exactly once. None is invalid;
    /// an empty slice is a valid empty inverse. Diagnostic identity comes from
    /// the live assignment and bound source/inverse metadata. Duplicate members
    /// and unknown assigned values fail too. Suitable for manager preflight.
    /// Readers must not mutate the registry; reads/clone panics preserve this supply.
    pub fn validate_external_inverse<'r>(
        &self,
        read: impl FnMut(&V) -> Option<&'r [E]>,
    ) -> Result<(), BasicSupplyError<E, V>>
    where
        E: 'r,
    {
        self.read_external_inverse(read).map(|_| ())
    }
    /// Import validated external inverses and return their actual before/after
    /// publication events. The entire import and ordinary refresh are staged:
    /// null/duplicate/unknown values, reader and Clone panics leave published
    /// collections and notification history unchanged. Apply returned events to
    /// external storage separately; this method does not invoke setters.
    /// Existing refresh_changed and non-null typed storage paths are unchanged.
    pub fn refresh_external_inverse<'r>(
        &mut self,
        read: impl FnMut(&V) -> Option<&'r [E]>,
    ) -> Result<BasicStateUpdate<'s, E, V>, BasicSupplyError<E, V>>
    where
        E: 'r,
    {
        let published = self.read_external_inverse(read)?;
        let mut staged = Self {
            state: self.state.clone(),
            values: self.values.clone(),
            binding: self.binding,
            published,
            observed: self.observed.clone(),
        };
        let update = staged.refresh_changed()?;
        self.published = staged.published;
        self.observed = staged.observed;
        Ok(update)
    }
}
