//! Explicit field policies and reusable mutable-container clone factories.
use crate::*;
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
/// Policy belongs to the field, not globally to the referenced allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjectCloneFieldPolicy {
    /// Keep the original handle even if another field copies that object.
    Shared,
    /// Require a registered factory and resolve a copied allocation.
    Required,
    /// Copy when a type factory exists; otherwise share an immutable fact.
    Registered,
}
impl<'a> ObjectGraphDiscovery<'_, 'a> {
    pub fn field<T: Any + Send + Sync>(
        &mut self,
        source: &Arc<T>,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<(), ObjectGraphCloneError> {
        match policy {
            ObjectCloneFieldPolicy::Shared => Ok(()),
            ObjectCloneFieldPolicy::Required => self.required(source),
            ObjectCloneFieldPolicy::Registered => {
                if self.has_factory::<T>() {
                    self.required(source)
                } else {
                    Ok(())
                }
            }
        }
    }
    pub fn optional_field<T: Any + Send + Sync>(
        &mut self,
        source: &Option<Arc<T>>,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<(), ObjectGraphCloneError> {
        if let Some(s) = source {
            self.field(s, policy)?;
        }
        Ok(())
    }
}
impl ObjectGraphCloneView {
    pub fn field<T: Any + Send + Sync>(
        &self,
        source: &Arc<T>,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<Arc<T>, ObjectGraphCloneError> {
        match policy {
            ObjectCloneFieldPolicy::Shared => Ok(source.clone()),
            ObjectCloneFieldPolicy::Required => self.required(source),
            ObjectCloneFieldPolicy::Registered => {
                if self.has_factory::<T>() {
                    self.required(source)
                } else {
                    Ok(source.clone())
                }
            }
        }
    }
    pub fn optional_field<T: Any + Send + Sync>(
        &self,
        source: &Option<Arc<T>>,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<Option<Arc<T>>, ObjectGraphCloneError> {
        source.as_ref().map(|s| self.field(s, policy)).transpose()
    }
}
impl<'a> ObjectGraphCloneRegistry<'a> {
    /// Copy list storage while applying a field policy to every nullable element.
    pub fn register_nullable_list<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<Vec<Option<Arc<T>>>>>(
            |_| Ok(Mutex::new(Vec::new())),
            move |s, c| {
                for v in s.lock().unwrap().iter() {
                    c.optional_field(v, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                *t.lock().unwrap() = s
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|v| c.optional_field(v, policy))
                    .collect::<Result<_, _>>()?;
                Ok(())
            },
        )
    }
    /// Copy ordered map storage and independently choose key/value policies.
    /// Null keys and values remain null. Rebased keys must remain distinct.
    pub fn register_nullable_map<K: Any + Send + Sync + Ord, V: Any + Send + Sync>(
        &mut self,
        keys: ObjectCloneFieldPolicy,
        values: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<BTreeMap<Option<Arc<K>>, Option<Arc<V>>>>>(
            |_| Ok(Mutex::new(BTreeMap::new())),
            move |s, c| {
                for (k, v) in s.lock().unwrap().iter() {
                    c.optional_field(k, keys)?;
                    c.optional_field(v, values)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let mut out = BTreeMap::new();
                for (k, v) in s.lock().unwrap().iter() {
                    let k = c.optional_field(k, keys)?;
                    let v = c.optional_field(v, values)?;
                    if out.insert(k, v).is_some() {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone map keys collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
    /// Copy ordered set storage; rebased identities must remain distinct.
    pub fn register_set<T: Any + Send + Sync + Ord>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<BTreeSet<Arc<T>>>>(
            |_| Ok(Mutex::new(BTreeSet::new())),
            move |s, c| {
                for v in s.lock().unwrap().iter() {
                    c.field(v, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let mut out = BTreeSet::new();
                for v in s.lock().unwrap().iter() {
                    if !out.insert(c.field(v, policy)?) {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone set values collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
}
