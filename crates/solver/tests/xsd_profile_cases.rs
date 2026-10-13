// Apache-2.0 TEST fixture/method adaptation; public API specifications separately attributed.
use timetuck::*;
#[test]
fn validate_by_merged_xsd() {
    let solver = include_str!("fixtures/xsd_profile/public-solver.xsd");
    let benchmark = include_str!("fixtures/xsd_profile/public-benchmark.xsd");
    let schema = XsdProfile::merge(benchmark, &[solver], XsdMergePolicy::OverlayTypes).unwrap();
    let path = std::env::temp_dir().join(format!("timetuck-merged-xsd-{}.xsd", std::process::id()));
    std::fs::write(&path, schema.to_xml()).unwrap();
    let loaded = XsdProfile::compile(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let xml = include_str!("fixtures/xsd_profile/validBenchmarkConfig.xml");
    loaded
        .validate(xml, XsdValidationLimits::default())
        .unwrap();
    // Actual existing configuration decoder runs only after successful schema validation.
    let config = BenchmarkRecordConfig::parse(xml).unwrap();
    assert!(config.to_xml().contains("Entity Tabu Search"));
    std::fs::remove_file(path).unwrap();
}
