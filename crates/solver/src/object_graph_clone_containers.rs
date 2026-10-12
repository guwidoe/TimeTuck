//! Container factories for ordered references and mutable identity-keyed graphs.
use crate::*;
use std::{
    any::Any,
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
};

/// An owning allocation-identity key. Equal payloads in different allocations
/// remain different keys; mutating the payload cannot change the key's hash.
pub struct ObjectIdentity<T>(Arc<T>);
impl<T> ObjectIdentity<T> {
    pub fn new(object: Arc<T>) -> Self {
        Self(object)
    }
    pub fn as_arc(&self) -> &Arc<T> {
        &self.0
    }
    pub fn into_arc(self) -> Arc<T> {
        self.0
    }
}
impl<T> Clone for ObjectIdentity<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> PartialEq for ObjectIdentity<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for ObjectIdentity<T> {}
impl<T> Hash for ObjectIdentity<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (Arc::as_ptr(&self.0) as usize).hash(state);
    }
}

impl<'a> ObjectGraphCloneRegistry<'a> {
    /// Copy a non-null list, preserving order and repeated allocation aliases.
    pub fn register_list<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<Vec<Arc<T>>>>(
            |_| Ok(Mutex::new(Vec::new())),
            move |s, c| {
                for item in s.lock().unwrap().iter() {
                    c.field(item, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let items = s
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|v| c.field(v, policy))
                    .collect::<Result<_, _>>()?;
                *t.lock().unwrap() = items;
                Ok(())
            },
        )
    }
    /// Copy array storage of any length, retaining order and element aliases.
    /// The boxed slice is installed only after all element references resolve.
    pub fn register_slice<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<Box<[Arc<T>]>>>(
            |_| Ok(Mutex::new(Vec::new().into_boxed_slice())),
            move |s, c| {
                for item in s.lock().unwrap().iter() {
                    c.field(item, policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let items = s
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|v| c.field(v, policy))
                    .collect::<Result<Vec<_>, _>>()?;
                *t.lock().unwrap() = items.into_boxed_slice();
                Ok(())
            },
        )
    }
    /// Copy a set with allocation-identity membership; no payload Eq/Hash needed.
    pub fn register_identity_set<T: Any + Send + Sync>(
        &mut self,
        policy: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<HashSet<ObjectIdentity<T>>>>(
            |_| Ok(Mutex::new(HashSet::new())),
            move |s, c| {
                for item in s.lock().unwrap().iter() {
                    c.field(item.as_arc(), policy)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let mut out = HashSet::new();
                for item in s.lock().unwrap().iter() {
                    if !out.insert(ObjectIdentity::new(c.field(item.as_arc(), policy)?)) {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone identity set members collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
    /// Copy identity-keyed map storage with independent key and value policies.
    /// Iteration order is unspecified; keys retain their owning allocations.
    pub fn register_identity_map<K: Any + Send + Sync, V: Any + Send + Sync>(
        &mut self,
        keys: ObjectCloneFieldPolicy,
        values: ObjectCloneFieldPolicy,
    ) -> Result<&mut Self, ObjectGraphCloneError> {
        self.register::<Mutex<HashMap<ObjectIdentity<K>, Arc<V>>>>(
            |_| Ok(Mutex::new(HashMap::new())),
            move |s, c| {
                for (k, v) in s.lock().unwrap().iter() {
                    c.field(k.as_arc(), keys)?;
                    c.field(v, values)?;
                }
                Ok(())
            },
            move |s, t, c| {
                let mut out = HashMap::new();
                for (k, v) in s.lock().unwrap().iter() {
                    let k = ObjectIdentity::new(c.field(k.as_arc(), keys)?);
                    let v = c.field(v, values)?;
                    if out.insert(k, v).is_some() {
                        return Err(ObjectGraphCloneError::Callback(
                            "clone identity map keys collided".into(),
                        ));
                    }
                }
                *t.lock().unwrap() = out;
                Ok(())
            },
        )
    }
}
