//! Original scalar boundaries, typed object and actual configuration integration.
use std::{cell::Cell, collections::BTreeMap, rc::Rc, time::Duration};
use timetuck::*;
#[test]
fn duration_signed_fractional_and_wide_round_trips() {
    for n in [
        i128::MIN,
        i128::MIN + 1,
        -86_400_000_000_001,
        -500_000_000,
        -1,
        0,
        1,
        500_000_000,
        86_400_000_000_001,
        i128::MAX,
    ] {
        let value = XmlDuration::from_nanoseconds(n);
        assert_eq!(value.to_string().parse::<XmlDuration>().unwrap(), value);
    }
    assert_eq!(
        "P1DT2H3M4.000000005S"
            .parse::<XmlDuration>()
            .unwrap()
            .nanoseconds(),
        93_784_000_000_005
    );
    assert_eq!(
        "PT-0.5S".parse::<XmlDuration>().unwrap().nanoseconds(),
        -500_000_000
    );
    assert_eq!(
        "-PT-0.5S".parse::<XmlDuration>().unwrap().nanoseconds(),
        500_000_000
    );
    assert_eq!(
        "PT1,25S".parse::<XmlDuration>().unwrap().to_std().unwrap(),
        Duration::from_millis(1250)
    );
    for s in [
        "",
        "P",
        "PT",
        "P1DT",
        "P1M",
        "PT1H1H",
        "PT1S1M",
        "PT1.0000000001S",
        "PT1.S",
        "PT9999999999999999999999999999999999999999S",
    ] {
        assert!(s.parse::<XmlDuration>().is_err(), "{s}");
    }
    assert!(XmlDuration::from_nanoseconds(-1).to_std().is_err());
    assert!(XmlDuration::from_nanoseconds(i128::MAX).to_std().is_err());
    assert_eq!(XmlDuration::nullable(Some(" ")).unwrap(), None);
}
#[test]
fn offset_instants_leap_dates_and_signed_years() {
    let a: XmlOffsetDateTime = "2020-02-29T00:30:00.000000001+01:00".parse().unwrap();
    let b: XmlOffsetDateTime = "2020-02-28T23:30:00.000000001Z".parse().unwrap();
    assert_eq!(a.instant_nanoseconds(), b.instant_nanoseconds());
    assert_ne!(a, b); // Local value and offset remain part of equality.
    for s in [
        "-0001-01-01T00:00:00-18:00",
        "+10000-01-01T23:59:59.999999999+00:00:01",
        "0000-02-29T12:30:00Z",
    ] {
        let v: XmlOffsetDateTime = s.parse().unwrap();
        assert_eq!(v.to_string().parse::<XmlOffsetDateTime>().unwrap(), v);
    }
    for s in [
        "2019-02-29T00:00:00Z",
        "2020-01-01T24:00:00Z",
        "2020-01-01T00:00:60Z",
        "2020-01-01T00:00:00+18:01",
        "2020-01-01T00:00:00+01:-1",
        "2020-01-01T00:00:00+01:+1",
        "2020-01-01T00:00:00+01:60",
        "2020-01-01T00:00:00",
    ] {
        assert!(s.parse::<XmlOffsetDateTime>().is_err(), "{s}");
    }
    assert_eq!(XmlOffsetDateTime::nullable(Some("")).unwrap(), None);
}
fn map_codec() -> XmlObjectCodec<BTreeMap<String, String>> {
    XmlObjectCodec::new(
        "customProperties",
        None,
        |n| Ok(read_xml_properties(Some(n))?.unwrap()),
        |v| write_xml_properties(Some(v)).unwrap(),
    )
}
#[test]
fn property_maps_round_trip_escaped_attributes_and_reject_ambiguous_entries() {
    let io = map_codec();
    let values = BTreeMap::from([("<&\"'".into(), "&<>\"'\r\n\t".into())]);
    assert_eq!(io.read(&io.write(&values).unwrap()).unwrap(), values);
    assert!(io.read("<customProperties><property name='a' value='1'/><property name='a' value='2'/></customProperties>").is_err());
    assert!(
        io.read("<customProperties><property name='a'/></customProperties>")
            .is_err()
    );
    assert!(
        io.read(
            "<customProperties><property xmlns='wrong' name='a' value='1'/></customProperties>"
        )
        .is_err()
    );
    assert_eq!(io.read("<customProperties/>").unwrap(), BTreeMap::new());
}
#[test]
fn xml_properties_configure_real_solver_and_preflight_bad_values() {
    let mut registry = ConfigPropertyRegistry::<SolverConfig<'static>>::default();
    registry
        .register("maxSteps", |c, v: u64| c.local_search[0].max_steps = v)
        .unwrap();
    registry
        .register("assertScores", |c, v: bool| {
            c.limits.assert_phase_scores = v
        })
        .unwrap();
    let mut config = SolverConfig::default();
    let original_steps = config.local_search[0].max_steps;
    let bad = map_codec().read("<customProperties><property name='maxSteps' value='3'/><property name='assertScores' value='bad'/></customProperties>").unwrap();
    assert!(apply_xml_properties(&registry, &mut config, &bad, "solver").is_err());
    assert_eq!(config.local_search[0].max_steps, original_steps);
    let values = map_codec().read("<customProperties><property name='maxSteps' value='3'/><property name='assertScores' value='true'/></customProperties>").unwrap();
    apply_xml_properties(&registry, &mut config, &values, "solver").unwrap();
    assert_eq!(config.local_search[0].max_steps, 3);
    assert!(config.limits.assert_phase_scores);
    let xml = SolverXmlConfiguration::parse(
        "<solver><termination><spentLimit>PT5M120S</spentLimit></termination></solver>",
    )
    .unwrap();
    config.limits.max_time = xml.native_spent_limit().unwrap();
    assert_eq!(config.limits.max_time, Some(Duration::from_secs(420)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i32, 1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap_or(&0)))
    })
    .unwrap();
    let result = solve(&p, &n, config).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(1)));
    for limit in ["-PT1S", "bad"] {
        let xml = SolverXmlConfiguration::parse(&format!(
            "<solver><termination><spentLimit>{limit}</spentLimit></termination></solver>"
        ))
        .unwrap();
        assert!(xml.native_spent_limit().is_err());
    }
    assert_eq!(
        SolverXmlConfiguration::parse("<solver/>")
            .unwrap()
            .native_spent_limit()
            .unwrap(),
        None
    );
}
#[test]
fn codecs_validate_namespaces_and_never_decode_during_writes() {
    let calls = Rc::new(Cell::new(0));
    let decoded = calls.clone();
    let io = XmlObjectCodec::new(
        "object",
        Some("urn:native".into()),
        move |n| {
            decoded.set(decoded.get() + 1);
            Ok(n.text().unwrap_or("").to_owned())
        },
        |v: &String| SolverXmlElement {
            name: "object".into(),
            attributes: vec![],
            text: v.clone(),
            children: vec![],
        },
    );
    let value = "text & < > \" '\r\n".to_owned();
    let text = io.write(&value).unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(io.read(&text).unwrap(), value);
    assert!(io.read("<object xmlns='urn:other'>v</object>").is_err());
    assert_eq!(
        io.read_overriding_namespace("<x:object xmlns:x='urn:other'>v</x:object>")
            .unwrap(),
        "v"
    );
    assert_eq!(calls.get(), 2);
    for read in [false, true] {
        let text = "<!DOCTYPE object [<!ENTITY x SYSTEM 'file:///does-not-exist'>]><object xmlns='urn:native'>&x;</object>";
        let error = if read {
            io.read_overriding_namespace(text)
        } else {
            io.read(text)
        }
        .unwrap_err();
        assert!(matches!(
            error.cause,
            SolverXmlCause::Syntax(roxmltree::Error::DtdDetected)
        ));
    }
    assert_eq!(calls.get(), 2);
    assert!(matches!(
        io.read("<object"),
        Err(SolverXmlError {
            cause: SolverXmlCause::Syntax(_),
            ..
        })
    ));
    let wrong = XmlObjectCodec::new(
        "object",
        None,
        |_| Ok(()),
        |_: &()| SolverXmlElement {
            name: "other".into(),
            attributes: vec![],
            text: String::new(),
            children: vec![],
        },
    );
    assert!(wrong.write(&()).is_err());
}
