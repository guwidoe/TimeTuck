//! Original merged-data, publication, validation and browser-launch controls.
#[allow(dead_code)]
#[path = "fixtures/benchmark_archive_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::{Value, json};
use timetuck::*;
fn data(path: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}
#[test]
fn selected_reports_preserve_rows_order_and_exclude_duplicate_runs() {
    let f = ArchiveFixture::setup();
    let selection = vec![
        f.names[2].clone(),
        f.names[0].clone(),
        f.names[2].clone(),
        "../outside".into(),
    ];
    let report = f.archive().aggregate_selected_archives(&selection).unwrap();
    let merged = data(&report.parent().unwrap().join("aggregate.json"));
    assert_eq!(merged["summary"]["runs"], 2);
    assert_eq!(merged["summary"]["successful"], 2);
    assert_eq!(merged["summary"]["failed"], 0);
    assert_eq!(merged["summary"]["warmups"], 0);
    for (i, name) in [f.names[2].clone(), f.names[0].clone()].iter().enumerate() {
        let original = data(&f.root.join(name).join("summary.json"));
        assert_eq!(merged["runs"][i]["report"], original);
        assert_eq!(merged["trials"][i]["trial"], original["trials"][0]);
        assert_eq!(merged["trials"][i]["run"], *name);
    }
    let again = f.archive().aggregate_selected_archives(&selection).unwrap();
    assert_ne!(report, again);
    assert!(
        again
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("aggregation")
    );
    assert!(report.exists());
    assert_eq!(f.archive().available_archived_runs().unwrap(), f.names);
}
#[test]
fn incomplete_failed_uninitialized_and_warmup_trials_remain_visible_with_exact_measured_totals() {
    let f = ArchiveFixture::setup();
    let input = f.temp.join("modified");
    std::fs::create_dir(&input).unwrap();
    let mut report = data(&f.root.join(&f.names[0]).join("summary.json"));
    let mut row = report["trials"][0].clone();
    row["result"]["score"] = json!("<script>bad & score</script>");
    row["result"]["evaluations"] = json!(10);
    let mut warmup = row.clone();
    warmup["warmup"] = json!(true);
    warmup["result"]["evaluations"] = json!(999);
    let mut uninitialized = row.clone();
    uninitialized["result"]["initialized"] = json!(false);
    let mut failure = row.clone();
    failure["result"] = json!({"error":"<img src=x onerror=bad()>"});
    row["result"]["evaluations"] = json!(20);
    report["complete"] = json!(false);
    report["trials"] = json!([warmup, uninitialized, failure, row]);
    std::fs::write(
        input.join("summary.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    let archived = f
        .archive()
        .archive_completed_report(&input, "2026-10-12_120003")
        .unwrap();
    let name = archived.file_name().unwrap().to_str().unwrap().to_owned();
    let html = f.archive().aggregate_selected_archives(&[name]).unwrap();
    let aggregate = data(&html.parent().unwrap().join("aggregate.json"));
    assert_eq!(
        aggregate["summary"],
        json!({"runs":1,"complete_runs":0,"successful":2,"failed":1,"warmups":1,"uninitialized":1,"evaluations":30})
    );
    assert_eq!(aggregate["runs"][0]["report"], report);
    assert_eq!(aggregate["trials"].as_array().unwrap().len(), 4);
    let text = std::fs::read_to_string(html).unwrap();
    assert!(text.contains("&lt;script&gt;bad &amp; score&lt;/script&gt;"));
    assert!(text.contains("&lt;img src=x onerror=bad()&gt;"));
    assert!(!text.contains("<script>"));
    assert!(!text.contains("<img"));
    assert!(text.contains("warm-ups excluded"));
}
#[test]
fn invalid_calendar_traversal_empty_and_malformed_journals_are_never_published_or_discovered() {
    let f = ArchiveFixture::setup();
    let source = f.root.join(&f.names[0]);
    for stamp in [
        "../outside",
        "2026-02-29_120000",
        "2026-04-31_120000",
        "2026-10-12_240000",
        "0000-01-01_000000",
        "2026-10-12_120000_1",
    ] {
        assert!(matches!(
            f.archive().archive_completed_report(&source, stamp),
            Err(BenchmarkArchiveError::InvalidTimestamp)
        ));
    }
    for (i,report) in [json!({"schema_version":1,"complete":true,"trials":[]}),json!({"schema_version":2,"complete":true,"trials":[]}),json!({"schema_version":1,"complete":false,"trials":[{"configuration":0,"problem":0,"repetition":0,"warmup":false,"result":{"score":"x","initialized":true,"evaluations":-1}}]})].iter().enumerate(){let path=f.root.join(format!("2026-10-12_12001{i}"));std::fs::create_dir(&path).unwrap();std::fs::write(path.join("summary.json"),serde_json::to_vec(report).unwrap()).unwrap();assert!(matches!(f.archive().archive_completed_report(&path,"2026-10-12_120005"),Err(BenchmarkArchiveError::InvalidReport)));}
    assert_eq!(f.archive().available_archived_runs().unwrap(), f.names);
    assert!(!f.temp.join("outside").exists());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&source, f.root.join("2026-10-12_120006")).unwrap();
        assert_eq!(f.archive().available_archived_runs().unwrap(), f.names);
        assert!(matches!(
            f.archive()
                .aggregate_selected_archives(&["2026-10-12_120006".into()]),
            Err(BenchmarkArchiveError::NoSelectedResults)
        ));
    }
}
#[test]
fn concurrent_timestamp_collisions_preserve_all_real_reports() {
    let f = ArchiveFixture::setup();
    let archive = f.archive();
    let source = f.root.join(&f.names[0]);
    let outputs = std::thread::scope(|scope| {
        let threads = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    archive
                        .archive_completed_report(&source, "2024-02-29_235959")
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        outputs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        8
    );
    let original = data(&source.join("summary.json"));
    for output in outputs {
        assert_eq!(data(&output.join("summary.json")), original);
    }
    assert_eq!(archive.available_archived_runs().unwrap().len(), 11);
}
#[test]
fn launcher_runs_once_after_both_artifacts_exist_and_failure_preserves_report() {
    let f = ArchiveFixture::setup();
    let mut calls = 0;
    let path = f
        .archive()
        .aggregate_and_open_archives(|path| {
            calls += 1;
            assert!(path.exists());
            let combined = data(&path.parent().unwrap().join("aggregate.json"));
            assert_eq!(combined["summary"]["runs"], 3);
            Err(std::io::Error::other("headless"))
        })
        .unwrap();
    assert_eq!(calls, 1);
    assert!(path.exists());
}
#[test]
fn counter_overflow_does_not_publish_a_misleading_aggregate() {
    let f = ArchiveFixture::setup();
    let source = f.root.join(&f.names[0]);
    let mut report = data(&source.join("summary.json"));
    report["trials"][0]["result"]["evaluations"] = json!(u64::MAX);
    let row = report["trials"][0].clone();
    report["trials"].as_array_mut().unwrap().push(row);
    std::fs::write(
        source.join("summary.json"),
        serde_json::to_vec(&report).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        f.archive()
            .aggregate_selected_archives(&[f.names[0].clone()]),
        Err(BenchmarkArchiveError::EvaluationOverflow)
    ));
    assert!(!f.root.join("aggregation").exists());
}

#[test]
fn archive_xml_retains_locale_and_delegates_solver_validation() {
    let f = ArchiveFixture::setup();
    let original = std::fs::read_to_string(&f.xml).unwrap();
    let a = BenchmarkArchive::from_xml_archive_file(&f.xml).unwrap();
    assert_eq!(a.archive_report_locale(), Some("en_US"));
    let html = a.aggregate_archived_runs().unwrap();
    assert!(
        std::fs::read_to_string(html)
            .unwrap()
            .contains("lang=\"en-US\"")
    );
    for replacement in [
        "<benchmarkReport><locale>en_US</locale><locale>de_DE</locale></benchmarkReport>",
        "<benchmarkReport><unknown/></benchmarkReport>",
        "<benchmarkReport><locale>&lt;script&gt;</locale></benchmarkReport>",
        "<benchmarkReport><locale/></benchmarkReport>",
        "<benchmarkReport/><benchmarkReport/>",
    ] {
        let altered = original.replace(
            "<benchmarkReport><locale>en_US</locale></benchmarkReport>",
            replacement,
        );
        let path = f.temp.join("invalid.xml");
        std::fs::write(&path, altered).unwrap();
        assert!(matches!(
            BenchmarkArchive::from_xml_archive_file(path),
            Err(BenchmarkArchiveError::Configuration(_))
        ));
    }
    let path = f.temp.join("bad-solver.xml");
    std::fs::write(&path, original.replace("<termination>", "<unknown/>")).unwrap();
    assert!(BenchmarkArchive::from_xml_archive_file(path).is_err());
}
