// Adapted pinned BenchmarkAggregatorTest; Apache-2.0. See compat/provenance.json.
#[path = "fixtures/benchmark_archive_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn aggregate_benchmarks_non_existent_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.path_archive("non_existent_directory")
            .aggregate_archived_runs()
            .unwrap_err()
            .to_string()
            .contains("Benchmark directory does not exist")
    );
}
#[test]
fn aggregate_benchmarks_directory_is_file() {
    let f = ArchiveFixture::setup();
    assert!(
        f.file("regularFile.txt")
            .aggregate_archived_runs()
            .unwrap_err()
            .to_string()
            .contains("Benchmark directory does not exist")
    );
}
#[test]
fn aggregate_benchmarks_empty_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.empty("empty")
            .aggregate_archived_runs()
            .unwrap_err()
            .to_string()
            .contains("No benchmark results found in directory")
    );
}
#[test]
fn aggregate_benchmarks_with_valid_results() {
    let f = ArchiveFixture::setup();
    let p = f.archive().aggregate_archived_runs().unwrap();
    html(&p);
    assert!(
        p.parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("aggregation")
    );
}
#[test]
fn aggregate_benchmarks_with_xml_config() {
    let f = ArchiveFixture::setup();
    let archive = BenchmarkArchive::from_xml_archive_file(&f.xml).unwrap();
    assert_eq!(archive.archive_report_locale(), Some("en_US"));
    let p = archive.aggregate_archived_runs().unwrap();
    html(&p);
}
#[test]
fn aggregate_selected_benchmarks_non_existent_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.path_archive("non_existent_directory")
            .aggregate_selected_archives(&["benchmark1".into(), "benchmark2".into()])
            .unwrap_err()
            .to_string()
            .contains("No benchmark results found in directory")
    );
}
#[test]
fn aggregate_selected_benchmarks_directory_is_file() {
    let f = ArchiveFixture::setup();
    assert!(
        f.file("regularFile2.txt")
            .aggregate_selected_archives(&["benchmark1".into(), "benchmark2".into()])
            .unwrap_err()
            .to_string()
            .contains("No benchmark results found in directory")
    );
}
#[test]
fn aggregate_selected_benchmarks_no_valid_directories() {
    let f = ArchiveFixture::setup();
    assert!(
        f.archive()
            .aggregate_selected_archives(&["nonExistent1".into(), "nonExistent2".into()])
            .unwrap_err()
            .to_string()
            .contains("No valid benchmark results found in the selected directories")
    );
}
#[test]
fn aggregate_selected_benchmarks_with_valid_directories() {
    let f = ArchiveFixture::setup();
    html(
        &f.archive()
            .aggregate_selected_archives(&f.names[..2])
            .unwrap(),
    );
}
#[test]
fn get_available_benchmark_directories_non_existent_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.path_archive("non_existent_directory")
            .available_archived_runs()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn get_available_benchmark_directories_empty_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.empty("empty2")
            .available_archived_runs()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn get_available_benchmark_directories_with_valid_directories() {
    let f = ArchiveFixture::setup();
    let names = f.archive().available_archived_runs().unwrap();
    assert_eq!(names.len(), 3);
    assert_eq!(names, f.names);
    for n in names {
        let b = n.as_bytes();
        assert_eq!(b.len(), 17);
        assert_eq!((b[4], b[7], b[10]), (b'-', b'-', b'_'));
        assert!(
            b.iter()
                .enumerate()
                .all(|(i, c)| matches!(i, 4 | 7 | 10) || c.is_ascii_digit())
        );
    }
}
#[test]
fn aggregate_and_show_report_in_browser_non_existent_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.path_archive("non_existent_directory")
            .aggregate_and_open_archives(|_| panic!("invalid report opened"))
            .unwrap_err()
            .to_string()
            .contains("Benchmark directory does not exist")
    );
}
#[test]
fn aggregate_and_show_report_in_browser_empty_directory() {
    let f = ArchiveFixture::setup();
    assert!(
        f.empty("empty3")
            .aggregate_and_open_archives(|_| panic!("invalid report opened"))
            .unwrap_err()
            .to_string()
            .contains("No benchmark results found in directory")
    );
}
#[test]
fn aggregate_and_show_report_in_browser_with_valid_results() {
    let f = ArchiveFixture::setup();
    html(
        &f.archive()
            .aggregate_and_open_archives(|_| Err(std::io::Error::other("browser unavailable")))
            .unwrap(),
    );
}
