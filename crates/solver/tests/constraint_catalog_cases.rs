// Complete pinned Community TEST adaptations; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::rc::Rc;
use timetuck::*;
#[test]
fn default_constraint_meta_model_test() {
    let mut b = CatalogGraphBuilder::<SimpleScore>::new();
    let source = b.builder().source::<usize, i32>();
    let declarations: Vec<_> = [1, 10, 100, 0]
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            Rc::new(
                DeclaredConstraint::new(format!("Test Constraint {}", i + 1), SimpleScore::new(w))
                    .unwrap(),
            )
        })
        .collect();
    for declaration in &declarations {
        b.register_weighted(declaration.clone(), source.port(), |_| 1)
            .unwrap();
    }
    let graph = b.build();
    for declaration in &declarations {
        assert!(Rc::ptr_eq(
            graph.catalog().get(declaration.name()).unwrap(),
            declaration
        ));
    }
    assert_eq!(graph.catalog().len(), 4);
}
#[test]
fn id_sanitized() {
    let rows = [
        (Some("name"), true),
        (Some("Name and spaces"), true),
        (Some("Name (or spaces)"), true),
        (Some("Name and numb3rs"), true),
        (Some("name_and_numb3r5"), true),
        (Some("name-and-numb3r5"), true),
        (Some("Let's allow a complete sentence."), true),
        (Some("name${something}name"), false),
        (Some("name$1name"), false),
        (Some("name%23name"), false),
        (Some("name/name"), false),
        (Some("name//name"), false),
        (Some("name\\/name"), false),
        (Some("name\\/\\/name"), false),
        (Some("null"), false),
        (Some("nil"), false),
        (None, false),
        (Some("-"), false),
        (Some("_"), false),
    ];
    assert_eq!(rows.len(), 19);
    for (name, correct) in rows {
        let mut b = CatalogGraphBuilder::<SimpleScore>::new();
        let source = b.builder().source::<usize, i32>();
        match DeclaredConstraint::from_optional_name(name, SimpleScore::new(1)) {
            Ok(declaration) => {
                assert!(correct, "{name:?}");
                let declaration = Rc::new(declaration);
                let result = b
                    .register_weighted(declaration.clone(), source.port(), |_| 1)
                    .unwrap();
                assert!(Rc::ptr_eq(&result.declaration, &declaration));
                assert_eq!(b.catalog().len(), 1);
            }
            Err(error) => {
                assert!(!correct, "{name:?}");
                assert_eq!(
                    error,
                    InvalidConstraintName {
                        name: name.map(str::to_owned)
                    }
                );
                assert!(b.catalog().is_empty());
            }
        }
    }
}
