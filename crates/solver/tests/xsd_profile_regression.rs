use timetuck::*;
const SPEC: &str = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:t="urn:records" targetNamespace="urn:records" elementFormDefault="qualified"><xs:element name="record" type="t:derived"/><xs:simpleType name="mode"><xs:restriction base="xs:string"><xs:enumeration value="READY"/><xs:enumeration value="DONE"/></xs:restriction></xs:simpleType><xs:complexType name="base" abstract="true"><xs:sequence><xs:element name="number" type="xs:int"/></xs:sequence><xs:attribute name="enabled" type="xs:boolean" use="required"/></xs:complexType><xs:complexType name="derived"><xs:complexContent><xs:extension base="t:base"><xs:sequence><xs:element name="mode" type="t:mode"/><xs:choice minOccurs="1" maxOccurs="2"><xs:element name="amount" type="xs:double"/><xs:element name="count" type="xs:long"/></xs:choice></xs:sequence></xs:extension></xs:complexContent></xs:complexType></xs:schema>"#;
#[test]
fn typed_extension_choice_attributes_and_lexical_boundaries() {
    let schema = XsdProfile::compile(SPEC).unwrap();
    for body in [
        "<amount>1.25e2</amount>",
        "<count>9223372036854775807</count><amount>INF</amount>",
        "<amount>-INF</amount><amount>NaN</amount>",
    ] {
        schema.validate(&format!("<record xmlns='urn:records' enabled='1'><number>+2147483647</number><mode>READY</mode>{body}</record>"),XsdValidationLimits::default()).unwrap();
    }
    for xml in [
        "<record xmlns='urn:records' enabled='true'><number>2147483648</number><mode>READY</mode><count>1</count></record>",
        "<record xmlns='urn:records' enabled='TRUE'><number>1</number><mode>READY</mode><count>1</count></record>",
        "<record xmlns='urn:records'><number>1</number><mode>READY</mode><count>1</count></record>",
        "<record xmlns='urn:records' enabled='false'><mode>READY</mode><number>1</number><count>1</count></record>",
        "<record xmlns='urn:records' enabled='false'><number>1</number><mode>BAD</mode><count>1</count></record>",
        "<record xmlns='urn:records' enabled='false'><number>1</number><mode>READY</mode></record>",
        "<record xmlns='urn:records' enabled='false'><number>1</number><mode>READY</mode><count>1</count><count>2</count><count>3</count></record>",
        "<record xmlns='urn:records' enabled='false'><number>1</number><mode>READY</mode><amount>inf</amount></record>",
        "<record xmlns='urn:records' enabled='false'><number>1</number><mode>READY</mode><count>9223372036854775808</count></record>",
    ] {
        assert!(
            schema
                .validate(xml, XsdValidationLimits::default())
                .is_err(),
            "accepted {xml}"
        );
    }
}
#[test]
fn real_public_schema_rejects_mutated_fixture() {
    let schema = XsdProfile::merge(
        include_str!("fixtures/xsd_profile/public-benchmark.xsd"),
        &[include_str!("fixtures/xsd_profile/public-solver.xsd")],
        XsdMergePolicy::OverlayTypes,
    )
    .unwrap();
    let valid = include_str!("fixtures/xsd_profile/validBenchmarkConfig.xml");
    for xml in [
        valid.replace(
            "<minutesSpentLimit>5</minutesSpentLimit>",
            "<minutesSpentLimit>five</minutesSpentLimit>",
        ),
        valid.replace("BEST_SCORE", "NOT_A_STATISTIC"),
        valid.replace(
            "<name>Entity Tabu Search</name>",
            "<name>A</name><name>B</name>",
        ),
        valid.replace(
            "<solver>\n      <solutionClass>",
            "<solver><unknown/><solutionClass>",
        ),
        valid.replace("https://timefold.ai/xsd/benchmark", "urn:wrong"),
    ] {
        assert!(
            schema
                .validate(&xml, XsdValidationLimits::default())
                .is_err()
        );
    }
}
#[test]
fn merge_overlay_retargets_real_type_references_and_survives_file_roundtrip() {
    let overlay = SPEC
        .replace("urn:records", "urn:new-version")
        .replace("type=\"xs:int\"", "type=\"xs:boolean\"");
    assert!(XsdProfile::merge(SPEC, &[&overlay], XsdMergePolicy::RejectConflicts).is_err());
    let merged = XsdProfile::merge(SPEC, &[&overlay], XsdMergePolicy::OverlayTypes).unwrap();
    assert_eq!(merged.namespace(), "urn:records");
    let reloaded = XsdProfile::compile(&merged.to_xml()).unwrap();
    let xml = "<record xmlns='urn:records' enabled='true'><number>false</number><mode>DONE</mode><count>2</count></record>";
    reloaded
        .validate(xml, XsdValidationLimits::default())
        .unwrap();
    assert!(
        XsdProfile::compile(SPEC)
            .unwrap()
            .validate(xml, XsdValidationLimits::default())
            .is_err()
    );
}
#[test]
fn unsupported_constructs_unresolved_refs_cycles_and_limits_fail_explicitly() {
    for (spec, diagnostic) in [
        (
            SPEC.replace("type=\"t:derived\"", "type=\"t:missing\""),
            "Unresolved type",
        ),
        (
            SPEC.replace(
                "<xs:enumeration value=\"READY\"/>",
                "<xs:pattern value=\".*\"/>",
            ),
            "Unsupported XSD facet",
        ),
        (
            SPEC.replace(
                "<xs:sequence><xs:element name=\"number\" type=\"xs:int\"/></xs:sequence>",
                "<xs:any/>",
            ),
            "Unsupported XSD content",
        ),
        (
            SPEC.replace("name=\"number\" type=\"xs:int\"", "ref=\"t:missing\""),
            "Unsupported XSD attribute",
        ),
        (
            SPEC.replace("base=\"t:base\"", "base=\"t:derived\""),
            "Cyclic schema",
        ),
        (
            SPEC.replace(
                "name=\"base\" abstract=\"true\"",
                "name=\"base\" abstract=\"true\" final=\"extension\"",
            ),
            "non-final complex",
        ),
    ] {
        assert!(
            XsdProfile::compile(&spec)
                .unwrap_err()
                .0
                .contains(diagnostic)
        );
    }
    let nested = format!(
        "<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema' targetNamespace='urn:deep' elementFormDefault='qualified'><xs:complexType name='deep'>{}<xs:element name='leaf' type='xs:string'/>{}</xs:complexType></xs:schema>",
        "<xs:sequence>".repeat(140),
        "</xs:sequence>".repeat(140)
    );
    assert!(
        XsdProfile::compile(&nested)
            .unwrap_err()
            .0
            .contains("Schema declaration depth budget exceeded")
    );
    let mut inherited = "<xs:schema xmlns:xs='http://www.w3.org/2001/XMLSchema' xmlns:t='urn:deep' targetNamespace='urn:deep' elementFormDefault='qualified'><xs:complexType name='t0'/>".to_string();
    for i in 1..=140 {
        inherited.push_str(&format!("<xs:complexType name='t{i}'><xs:complexContent><xs:extension base='t:t{}'/></xs:complexContent></xs:complexType>", i-1));
    }
    inherited.push_str("</xs:schema>");
    assert!(
        XsdProfile::compile(&inherited)
            .unwrap_err()
            .0
            .contains("Schema inheritance depth budget exceeded")
    );
    let schema = XsdProfile::compile(SPEC).unwrap();
    let xml = "<record xmlns='urn:records' enabled='true'><number>1</number><mode>READY</mode><count>1</count></record>";
    for limits in [
        XsdValidationLimits {
            max_bytes: 1,
            ..XsdValidationLimits::default()
        },
        XsdValidationLimits {
            max_nodes: 1,
            ..XsdValidationLimits::default()
        },
        XsdValidationLimits {
            max_steps: 1,
            ..XsdValidationLimits::default()
        },
        XsdValidationLimits {
            max_depth: 0,
            ..XsdValidationLimits::default()
        },
    ] {
        assert!(
            schema
                .validate(xml, limits)
                .unwrap_err()
                .0
                .contains("budget exceeded")
        );
    }
}

#[test]
fn inline_complex_content_validates_actual_nested_types_and_attributes() {
    let spec = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:t="urn:inline" targetNamespace="urn:inline" elementFormDefault="qualified"><xs:element name="root" type="t:container"/><xs:complexType name="container"><xs:sequence><xs:element name="nested"><xs:complexType><xs:sequence><xs:element name="value" type="xs:int" minOccurs="1" maxOccurs="2"/></xs:sequence><xs:attribute name="flag" type="xs:boolean" use="required"/></xs:complexType></xs:element></xs:sequence></xs:complexType></xs:schema>"#;
    let complex_attribute = spec.replace(
        "name=\"flag\" type=\"xs:boolean\"",
        "name=\"flag\" type=\"t:container\"",
    );
    assert!(
        XsdProfile::compile(&complex_attribute)
            .unwrap_err()
            .0
            .contains("Attribute requires simple type")
    );
    let schema = XsdProfile::compile(spec).unwrap();
    let schema = XsdProfile::compile(&schema.to_xml()).unwrap();
    schema.validate("<root xmlns='urn:inline'><nested flag='false'><value>3</value><value>-7</value></nested></root>", XsdValidationLimits::default()).unwrap();
    for bad in [
        "<root xmlns='urn:inline'><nested flag='true'/></root>",
        "<root xmlns='urn:inline'><nested><value>2</value></nested></root>",
        "<root xmlns='urn:inline'><nested flag='true'><value>bad</value></nested></root>",
        "<root xmlns='urn:inline'><nested flag='true'><wrong>2</wrong></nested></root>",
    ] {
        assert!(
            schema
                .validate(bad, XsdValidationLimits::default())
                .is_err()
        );
    }
}
