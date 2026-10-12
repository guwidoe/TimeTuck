//! Original XML bounds, normalized namespaces, explicit binding and runtime behavior.
#[allow(dead_code)]
#[path = "fixtures/xml_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, error::Error, rc::Rc};
use timetuck::*;
#[test]
fn parser_rejects_external_entities_foreign_nodes_and_unknown_attributes() {
    for xml in [
        "<!DOCTYPE solver [<!ENTITY secret SYSTEM 'file:///not-read'>]><solver><solutionClass>&secret;</solutionClass></solver>",
        "<solver xmlns='foreign'/>",
        "<solver><solutionClass unknown='x'>A</solutionClass></solver>",
        "<solver><solutionClass>A</solutionClass>mixed</solver>",
    ] {
        assert!(SolverXmlConfiguration::parse(xml).is_err());
    }
}
#[test]
fn byte_node_and_depth_budgets_reject_before_factory_execution() {
    for limits in [
        SolverXmlLimits {
            bytes: 3,
            ..Default::default()
        },
        SolverXmlLimits {
            nodes: 2,
            ..Default::default()
        },
        SolverXmlLimits {
            depth: 0,
            ..Default::default()
        },
    ] {
        let e = SolverXmlConfiguration::parse_with_limits(SIMPLE, "bounded", limits).unwrap_err();
        assert!(e.to_string().contains("bounded"));
        assert!(e.source().is_some());
    }
}
#[test]
fn namespace_normalization_changes_only_the_root_declaration() {
    let input = "<?xml version='1.0'?><!-- literal xmlns=\"https://timefold.ai/xsd/solver\" --><solver  xmlns = 'https://timefold.ai/xsd/solver'><solutionClass><![CDATA[owned<&>]]></solutionClass></solver>";
    let c = SolverXmlConfiguration::parse(input).unwrap();
    let output = c.to_xml();
    assert!(output.contains("<!-- literal xmlns=\"https://timefold.ai/xsd/solver\" -->"));
    assert!(!output.contains("xmlns = '"));
    assert_eq!(
        SolverXmlConfiguration::parse(&output)
            .unwrap()
            .solution_key(),
        Some("owned<&>")
    );
}
#[test]
fn mutations_escape_values_and_inheritance_preserves_child_fields() {
    let parent = SolverXmlConfiguration::parse(SIMPLE).unwrap();
    let child =
        SolverXmlConfiguration::parse("<solver><solutionClass>Child</solutionClass></solver>")
            .unwrap()
            .with_preview_feature("A&B")
            .inherit(&parent);
    assert_eq!(child.solution_key(), Some("Child"));
    assert_eq!(child.entity_keys(), parent.entity_keys());
    let xml = child.to_xml();
    assert!(xml.contains("A&amp;B"));
    let read = SolverXmlConfiguration::parse(&xml).unwrap();
    assert_eq!(read.preview_features().unwrap(), vec!["A&B"]);
    assert_eq!(
        parent.solution_key(),
        Some("ai.timefold.solver.core.testdomain.TestdataSolution")
    );
}
#[test]
fn complete_documents_round_trip_and_preserve_recursive_reference_keys() {
    for text in [SIMPLE, DIVERTED, INVALID, NAMESPACE, COMPLETE, ENUMS] {
        let parsed = SolverXmlConfiguration::parse(text);
        if text == INVALID {
            assert!(parsed.is_err());
            continue;
        }
        let c = parsed.unwrap();
        let reread = SolverXmlConfiguration::parse(&c.to_xml()).unwrap();
        assert_eq!(reread.root(), c.root());
        assert_eq!(reread.referenced_components(), c.referenced_components());
    }
}
#[test]
fn typed_registration_preflight_and_builder_errors_preserve_real_causes() {
    let c = SolverXmlConfiguration::parse(SIMPLE).unwrap();
    let key = XmlFactorySignature::from_configuration(&c).unwrap();
    let calls = Rc::new(Cell::new(0));
    let mut r = Registry::default();
    r.register(key.clone(), {
        let calls = calls.clone();
        move |_| {
            calls.set(calls.get() + 1);
            Err(SolverFactoryError::EmptyPhases)
        }
    })
    .unwrap();
    assert!(matches!(
        r.register(key, |_| Err(SolverFactoryError::SharedCancellationToken)),
        Err(XmlFactoryError::Duplicate(_))
    ));
    let e = r.prepare(&c).err().unwrap();
    assert_eq!(calls.get(), 1);
    assert!(e.source().unwrap().is::<SolverFactoryError>());
    let unknown =
        SolverXmlConfiguration::parse(&SIMPLE.replace("TestdataSolution", "OtherSolution"))
            .unwrap();
    assert!(matches!(
        r.prepare(&unknown),
        Err(XmlFactoryError::Unregistered(_))
    ));
    assert_eq!(calls.get(), 1);
    actual_solver(registry().prepare(&c).unwrap());
}
