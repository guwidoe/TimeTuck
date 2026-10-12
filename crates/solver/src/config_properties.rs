//! Explicit typed properties for solver/phase factories, without reflection.
use std::{collections::BTreeMap, error::Error, fmt, str::FromStr, sync::Arc};

/// Retain a property only when both observations contain the same value.
pub fn merge_config_property<T: Eq>(left: Option<T>, right: Option<T>) -> Option<T> {
    match (left, right) {
        (Some(a), Some(b)) if a == b => Some(a),
        _ => None,
    }
}
/// Missing observations inherit the other value; conflicting values disappear.
pub fn meld_config_property<T: Eq>(left: Option<T>, right: Option<T>) -> Option<T> {
    match (left, right) {
        (Some(a), Some(b)) => merge_config_property(Some(a), Some(b)),
        (a, None) | (None, a) => a,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigDivisionError {
    Zero,
    Overflow,
}
impl fmt::Display for ConfigDivisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Zero => "division by zero",
            Self::Overflow => "integer division overflows",
        })
    }
}
impl Error for ConfigDivisionError {}
/// Ceiling of signed integer division, without floating conversion or an
/// overflowing numerator adjustment. MIN/-1 returns an explicit native error.
pub fn ceil_divide_config(numerator: i64, denominator: i64) -> Result<i64, ConfigDivisionError> {
    if denominator == 0 {
        return Err(ConfigDivisionError::Zero);
    }
    let q = numerator
        .checked_div(denominator)
        .ok_or(ConfigDivisionError::Overflow)?;
    let r = numerator
        .checked_rem(denominator)
        .ok_or(ConfigDivisionError::Overflow)?;
    Ok(q + i64::from(r != 0 && (r > 0) == (denominator > 0)))
}
/// Read at most four supplied entries. None/empty remains an empty description.
pub fn abbreviate_config<T: fmt::Display>(values: Option<impl IntoIterator<Item = T>>) -> String {
    let Some(values) = values else {
        return String::new();
    };
    let mut it = values.into_iter();
    let mut out = it
        .by_ref()
        .take(3)
        .map(|v| v.to_string())
        .collect::<Vec<_>>();
    if it.next().is_some() {
        out.push("...".into());
    }
    out.join(", ")
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigPropertyError {
    DuplicateRegistration(String),
    DuplicateInput {
        owner: String,
        collection: String,
        property: String,
    },
    Unknown {
        owner: String,
        collection: String,
        property: String,
    },
    Parse {
        owner: String,
        collection: String,
        property: String,
        reason: String,
    },
}
impl fmt::Display for ConfigPropertyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration property: {self:?}")
    }
}
impl Error for ConfigPropertyError {}
type Apply<T> = Box<dyn FnOnce(&mut T)>;
type Prepare<T> = dyn Fn(&str) -> Result<Apply<T>, String>;
/// Register actual typed setter callbacks. Input parsing uses FromStr; Rust
/// strict parsing replaces reflective Java bean coercion. Keys/values are all
/// validated before any setter executes, so unknown/duplicate/malformed inputs
/// leave the target untouched. Omitted fields retain their existing values.
/// A setter panic can leave prior setter effects; application callbacks cannot
/// be rolled back automatically. Neither target nor parsed values need Clone/Eq.
pub struct ConfigPropertyRegistry<T> {
    setters: BTreeMap<String, Box<Prepare<T>>>,
}
impl<T> Default for ConfigPropertyRegistry<T> {
    fn default() -> Self {
        Self {
            setters: BTreeMap::new(),
        }
    }
}
impl<T: 'static> ConfigPropertyRegistry<T> {
    pub fn register<V: FromStr + 'static>(
        &mut self,
        name: impl Into<String>,
        setter: impl Fn(&mut T, V) + 'static,
    ) -> Result<&mut Self, ConfigPropertyError>
    where
        V::Err: fmt::Display,
    {
        let name = name.into();
        if self.setters.contains_key(&name) {
            return Err(ConfigPropertyError::DuplicateRegistration(name));
        }
        let setter = Arc::new(setter);
        self.setters.insert(
            name,
            Box::new(move |text| {
                let value = text.parse::<V>().map_err(|e| e.to_string())?;
                let setter = setter.clone();
                Ok(Box::new(move |target| setter(target, value)))
            }),
        );
        Ok(self)
    }
    pub fn apply<'a>(
        &self,
        target: &mut T,
        owner: &str,
        properties: impl IntoIterator<Item = (&'a str, &'a str)>,
        collection: &str,
    ) -> Result<(), ConfigPropertyError> {
        let mut seen = std::collections::BTreeSet::new();
        let mut prepared = Vec::new();
        for (property, value) in properties {
            if !seen.insert(property) {
                return Err(ConfigPropertyError::DuplicateInput {
                    owner: owner.into(),
                    collection: collection.into(),
                    property: property.into(),
                });
            }
            let parse = self
                .setters
                .get(property)
                .ok_or_else(|| ConfigPropertyError::Unknown {
                    owner: owner.into(),
                    collection: collection.into(),
                    property: property.into(),
                })?;
            let apply = parse(value).map_err(|reason| ConfigPropertyError::Parse {
                owner: owner.into(),
                collection: collection.into(),
                property: property.into(),
                reason,
            })?;
            prepared.push(apply);
        }
        for apply in prepared {
            apply(target);
        }
        Ok(())
    }
}
