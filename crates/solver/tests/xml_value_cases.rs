// Complete adapted pinned Community JAXB TEST methods, Apache-2.0.
// Native explicit codecs replace JAXB/reflection; provenance records all criteria.
#[path = "fixtures/xml_values_fixture.rs"]
mod fixture;
use fixture::*;
use std::error::Error;
use timetuck::*;
#[test]
fn duration_unmarshall() {
    assert_eq!(
        "PT5M120S".parse::<XmlDuration>().unwrap(),
        XmlDuration::from_nanoseconds(420_000_000_000)
    );
}
#[test]
fn duration_null_or_empty_should_unmarshall_as_null() {
    assert_eq!(XmlDuration::nullable(None).unwrap(), None);
}
#[test]
fn duration_marshall() {
    assert_eq!(
        XmlDuration::from_nanoseconds(300_000_000_000).to_string(),
        "PT5M"
    );
}
fn date_time() -> XmlOffsetDateTime {
    XmlOffsetDateTime::new(
        CivilDateTime::new(
            CivilDate::new(2020, 1, 1).unwrap(),
            CivilTime::new(12, 0, 5, 100_000_000).unwrap(),
        ),
        7200,
    )
    .unwrap()
}
#[test]
fn offset_date_time_unmarshall() {
    assert_eq!(
        "2020-01-01T12:00:05.1+02:00"
            .parse::<XmlOffsetDateTime>()
            .unwrap(),
        date_time()
    );
}
#[test]
fn offset_date_time_null_or_empty_should_unmarshall_as_null() {
    assert_eq!(XmlOffsetDateTime::nullable(None).unwrap(), None);
}
#[test]
fn offset_date_time_marshall() {
    assert_eq!(date_time().to_string(), "2020-01-01T12:00:05.1+02:00");
}
#[test]
fn read_custom_properties() {
    let codec = XmlObjectCodec::new(
        "testBean",
        None,
        |n| read_xml_properties(n.children().find(|n| n.has_tag_name("customProperties"))),
        |v| SolverXmlElement {
            name: "testBean".into(),
            attributes: vec![],
            text: String::new(),
            children: write_xml_properties(v.as_ref()).into_iter().collect(),
        },
    );
    let xml = "<testBean>  <customProperties>    <property xmlns=\"https://timefold.ai/xsd/solver\" name=\"firstKey\" value=\"firstValue\"/>    <property xmlns=\"https://timefold.ai/xsd/solver\" name=\"secondKey\" value=\"secondValue\"/>  </customProperties></testBean>";
    let values = codec.read(xml).unwrap().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values["firstKey"], "firstValue");
    assert_eq!(values["secondKey"], "secondValue");
}
#[test]
fn null_values() {
    assert_eq!(write_xml_properties(None), None);
    assert_eq!(read_xml_properties(None).unwrap(), None);
}
#[test]
fn read_write_simple_object() {
    let original = Dummy {
        id: 1,
        value: String::new(),
    };
    let io = codec();
    assert_eq!(io.read(&io.write(&original).unwrap()).unwrap(), original);
}
#[test]
fn read_throws_exception_on_invalid_xml() {
    let error: SolverXmlError = codec().read("<unknownRootElement/>").unwrap_err();
    assert!(matches!(error.cause, SolverXmlCause::Schema(_)));
}
#[test]
fn read_overriding_namespace_is_protected_from_xxe() {
    let malicious = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE dummyJaxbClass [  \n  <!ELEMENT dummyJaxbClass ANY >\n  <!ENTITY xxe SYSTEM \"file:///etc/passwd\" >]><dummyJaxbClass>&xxe;</dummyJaxbClass>";
    let error: SolverXmlError = codec().read_overriding_namespace(malicious).unwrap_err();
    // Native parser exception replaces SAXParseException; the actual parser
    // cause is retained and the source's doctype diagnostic remains observable.
    assert!(matches!(
        error.source().unwrap().downcast_ref::<roxmltree::Error>(),
        Some(roxmltree::Error::DtdDetected)
    ));
    assert!(error.to_string().contains("disallow-doctype-decl"));
}
