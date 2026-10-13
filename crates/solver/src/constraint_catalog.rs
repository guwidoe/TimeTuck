//! Optional validated declaration identities bound to actual stream terminals.
use crate::{
    AdditiveScore, GraphError, StreamConstraint, StreamGraph, StreamGraphBuilder, StreamPort,
};
use std::{collections::BTreeMap, error::Error, fmt, rc::Rc};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidConstraintName {
    pub name: Option<String>,
}
impl fmt::Display for InvalidConstraintName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid catalog constraint name: {:?}", self.name)
    }
}
impl Error for InvalidConstraintName {}
/// Validate this optional catalog's native names without rewriting them. Names
/// contain Unicode letters/numbers, spaces, `_`, `-`, apostrophes, periods and
/// parentheses, with at least one letter/number. Empty/missing names and reserved
/// `null`/`nil` (ignoring case/outer whitespace) reject. Legacy graph registration
/// retains its existing name policy. This does not claim arbitrary Java regex parity.
pub fn validate_catalog_constraint_name(name: Option<&str>) -> Result<(), InvalidConstraintName> {
    let valid = name.is_some_and(|n| {
        let text = n.trim();
        !matches!(text.to_ascii_lowercase().as_str(), "null" | "nil")
            && n.chars().any(char::is_alphanumeric)
            && n.chars().all(|c| {
                c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '\'' | '.' | '(' | ')')
            })
    });
    if valid {
        Ok(())
    } else {
        Err(InvalidConstraintName {
            name: name.map(str::to_owned),
        })
    }
}
/// Caller-created stable declaration identity. Declared weight stays unchanged
/// when a graph applies runtime overrides; graph weights remain authoritative.
#[derive(Debug)]
pub struct DeclaredConstraint<S> {
    name: String,
    weight: S,
}
impl<S> DeclaredConstraint<S> {
    pub fn new(name: impl Into<String>, weight: S) -> Result<Self, InvalidConstraintName> {
        let name = name.into();
        Self::from_optional_name(Some(&name), weight)
    }
    /// Construct a declaration from an optional external name without publishing
    /// anything to a graph. Missing names receive the same typed validation error.
    pub fn from_optional_name(
        name: Option<&str>,
        weight: S,
    ) -> Result<Self, InvalidConstraintName> {
        validate_catalog_constraint_name(name)?;
        Ok(Self {
            name: name.expect("validated name").to_owned(),
            weight,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn declared_weight(&self) -> &S {
        &self.weight
    }
}
pub struct ConstraintCatalog<S> {
    declarations: BTreeMap<String, Rc<DeclaredConstraint<S>>>,
}
impl<S> ConstraintCatalog<S> {
    pub fn get(&self, name: &str) -> Option<&Rc<DeclaredConstraint<S>>> {
        self.declarations.get(name)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Rc<DeclaredConstraint<S>>> {
        self.declarations.values()
    }
    pub fn len(&self) -> usize {
        self.declarations.len()
    }
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}
/// Retain both the exact caller declaration and the real graph terminal handle,
/// allowing ordinary match/explanation queries against the registered constraint.
pub struct CatalogConstraint<I, T, S> {
    pub declaration: Rc<DeclaredConstraint<S>>,
    pub terminal: StreamConstraint<I, T>,
}
pub struct CatalogGraphBuilder<'a, S: AdditiveScore + 'static> {
    builder: StreamGraphBuilder<'a, S>,
    catalog: ConstraintCatalog<S>,
}
impl<'a, S: AdditiveScore + 'static> Default for CatalogGraphBuilder<'a, S> {
    fn default() -> Self {
        Self::new()
    }
}
impl<'a, S: AdditiveScore + 'static> CatalogGraphBuilder<'a, S> {
    pub fn new() -> Self {
        Self {
            builder: StreamGraphBuilder::new(),
            catalog: ConstraintCatalog {
                declarations: BTreeMap::new(),
            },
        }
    }
    /// Build ordinary native topology. Direct legacy registrations remain usable but
    /// do not acquire declaration identities unless registered through this wrapper.
    pub fn builder(&mut self) -> &mut StreamGraphBuilder<'a, S> {
        &mut self.builder
    }
    pub fn catalog(&self) -> &ConstraintCatalog<S> {
        &self.catalog
    }
    /// Name/duplicate/port/weight errors publish neither catalog entry nor terminal.
    /// Catalog insertion happens only after successful native graph registration.
    pub fn register_weighted<I: Ord + Clone + 'static, T: Clone + 'static>(
        &mut self,
        declaration: Rc<DeclaredConstraint<S>>,
        input: &StreamPort<I, T>,
        factor: impl Fn(&T) -> i64 + 'a,
    ) -> Result<CatalogConstraint<I, T, S>, GraphError> {
        let terminal = self.builder.score_weighted(
            declaration.name(),
            input,
            declaration.weight.clone(),
            factor,
        )?;
        self.catalog
            .declarations
            .insert(declaration.name.clone(), declaration.clone());
        Ok(CatalogConstraint {
            declaration,
            terminal,
        })
    }
    pub fn build(self) -> CatalogGraph<'a, S> {
        CatalogGraph {
            graph: self.builder.build(),
            catalog: self.catalog,
        }
    }
}
pub struct CatalogGraph<'a, S: AdditiveScore + 'static> {
    graph: StreamGraph<'a, S>,
    catalog: ConstraintCatalog<S>,
}
impl<'a, S: AdditiveScore + 'static> CatalogGraph<'a, S> {
    pub fn graph(&self) -> &StreamGraph<'a, S> {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut StreamGraph<'a, S> {
        &mut self.graph
    }
    pub fn catalog(&self) -> &ConstraintCatalog<S> {
        &self.catalog
    }
}
