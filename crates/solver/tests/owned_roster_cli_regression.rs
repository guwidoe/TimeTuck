//! Original CLI and representative application checks; no upstream-method credit.
#[path = "../examples/owned_roster_cli/mod.rs"]
mod cli;
#[path = "../examples/owned_roster_support/mod.rs"]
mod owned_roster_support;
use cli::{Files, Invocation};
use owned_roster_support::*;
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use timetuck::{HardSoftScore, SolutionFileIo};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "timetuck-roster-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn files(&self, backend: Backend) -> Files {
        Files {
            input: self.0.join("input.json"),
            config: self.0.join("config.json"),
            output: self.0.join("solved.json"),
            backend,
            solver_xml: None,
        }
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn parse(args: &[&str]) -> Result<Invocation, String> {
    cli::parse(args.iter().map(|arg| OsString::from(*arg)))
}
fn prepare(files: &Files) {
    SolutionFileIo::new(RosterCodec)
        .write(&sample(), &files.input)
        .unwrap();
    std::fs::write(
        &files.config,
        configuration_binding()
            .encode(&Configuration::default())
            .unwrap(),
    )
    .unwrap();
}
#[test]
fn parsing_preserves_demo_help_version_and_rejects_ambiguous_usage() {
    assert!(matches!(parse(&[]).unwrap(), Invocation::Demo));
    assert!(matches!(
        parse(&["--unknown", "--help"]).unwrap(),
        Invocation::Help
    ));
    assert!(matches!(
        parse(&["--input", "-h"]).unwrap(),
        Invocation::Help
    ));
    assert!(matches!(
        parse(&["--version"]).unwrap(),
        Invocation::Version
    ));
    assert!(cli::HELP.contains("--solver-xml"));
    for args in [
        vec!["--wat"],
        vec!["--input"],
        vec!["--input", "--output"],
        vec!["--input", "x", "--input", "y"],
        vec![
            "--input",
            "x",
            "--config",
            "c",
            "--output",
            "o",
            "--backend",
            "other",
        ],
        vec!["--input", "x", "--config", "c"],
        vec!["--input", ""],
    ] {
        assert!(parse(&args).is_err(), "{args:?}");
    }
    let Invocation::Files(files) = parse(&[
        "--input",
        "in file",
        "--config",
        "config",
        "--output",
        "out",
        "--solver-xml",
        "solver.xml",
    ])
    .unwrap() else {
        panic!("expected files")
    };
    assert!(matches!(files.backend, Backend::Dense));
    assert_eq!(files.input, PathBuf::from("in file"));
    assert_eq!(files.solver_xml, Some(PathBuf::from("solver.xml")));
}
#[test]
fn file_invocation_executes_both_backends_and_preserves_documents_on_errors() {
    for backend in [Backend::Dense, Backend::Stream] {
        let dir = Directory::new();
        let mut files = dir.files(backend);
        prepare(&files);
        let original = std::fs::read(&files.input).unwrap();
        let summary = cli::execute(&files).unwrap();
        assert!(summary.contains("evaluations"));
        let solved: Roster = SolutionFileIo::new(RosterCodec)
            .read(&files.output)
            .unwrap();
        assert_eq!(
            solved.score,
            Some(independent_score(&solved, &Configuration::default()))
        );
        assert!(solved.shifts.iter().all(|s| s.employee.is_some()));
        assert_eq!(std::fs::read(&files.input).unwrap(), original);
        let saved = std::fs::read(&files.output).unwrap();
        std::fs::write(&files.config, "bad config").unwrap();
        assert!(cli::execute(&files).is_err());
        assert_eq!(std::fs::read(&files.output).unwrap(), saved);
        prepare(&files);
        let xml = dir.0.join("solver.xml");
        files.solver_xml = Some(xml.clone());
        std::fs::write(&xml, "<solver><termination><spentLimit>PT0S</spentLimit></termination><constructionHeuristic/><localSearch/></solver>").unwrap();
        // A root zero-time stop precedes score initialization in the existing
        // application; its independent-score guard rejects and preserves output.
        assert!(cli::execute(&files).is_err());
        assert_eq!(std::fs::read(&files.output).unwrap(), saved);
        std::fs::write(&xml, "<solver><termination><spentLimit>PT15S</spentLimit></termination><constructionHeuristic/><localSearch><termination><stepCountLimit>2</stepCountLimit></termination></localSearch></solver>").unwrap();
        cli::execute(&files).unwrap();
        let reread: Roster = SolutionFileIo::new(RosterCodec)
            .read(&files.output)
            .unwrap();
        assert_eq!(
            reread.score,
            Some(independent_score(&reread, &Configuration::default()))
        );
        assert!(reread.shifts.iter().all(|s| s.employee.is_some()));
        let saved = std::fs::read(&files.output).unwrap();
        std::fs::write(&xml, "bad XML").unwrap();
        assert!(cli::execute(&files).is_err());
        assert_eq!(std::fs::read(&files.output).unwrap(), saved);
        for source in [files.input.clone(), files.config.clone(), xml] {
            files.output = source.clone();
            let saved = std::fs::read(&source).unwrap();
            assert!(cli::execute(&files).unwrap_err().contains("aliases"));
            assert_eq!(std::fs::read(&source).unwrap(), saved);
        }
    }
}
#[cfg(unix)]
#[test]
fn non_utf8_paths_and_existing_symlink_hardlink_aliases_are_preserved() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let dir = Directory::new();
    let mut files = dir.files(Backend::Dense);
    files.input = dir.0.join(OsString::from_vec(b"roster-\xff.json".to_vec()));
    prepare(&files);
    let args = [
        OsString::from("--input"),
        files.input.clone().into_os_string(),
        OsString::from("--config"),
        files.config.clone().into_os_string(),
        OsString::from("--output"),
        files.output.clone().into_os_string(),
    ];
    let Invocation::Files(parsed) = cli::parse(args).unwrap() else {
        panic!("expected files")
    };
    assert_eq!(parsed.input, files.input);
    cli::execute(&parsed).unwrap();
    let original = std::fs::read(&files.input).unwrap();
    for hard in [false, true] {
        let alias = dir.0.join(if hard { "hard.json" } else { "symbolic.json" });
        if hard {
            std::fs::hard_link(&files.input, &alias).unwrap();
        } else {
            symlink(&files.input, &alias).unwrap();
        }
        files.output = alias.clone();
        assert!(cli::execute(&files).unwrap_err().contains("aliases"));
        assert_eq!(std::fs::read(&alias).unwrap(), original);
        assert_eq!(std::fs::read(&files.input).unwrap(), original);
    }
}
#[test]
fn representative_file_roster_solves_128_shifts_with_a_planted_quality_certificate() {
    let settings = Configuration {
        steps: 16,
        preference_weight: 3,
    };
    let roster = Roster {
        employees: (0..16)
            .map(|e| Employee {
                id: format!("employee-{e}"),
                skills: vec![format!("skill-{}", e % 4)],
            })
            .collect(),
        shifts: (0..128)
            .map(|s| Shift {
                id: format!("shift-{s}"),
                start: (s / 16) * 100,
                end: (s / 16) * 100 + 60,
                skill: format!("skill-{}", s % 4),
                preferred: (s % 16) as usize,
                employee: None,
            })
            .collect(),
        score: None,
    };
    let mut certificate = roster.clone();
    for shift in &mut certificate.shifts {
        shift.employee = Some(shift.preferred);
    }
    let expected = HardSoftScore::new(0, 0);
    assert_eq!(independent_score(&certificate, &settings), expected);
    for backend in [Backend::Dense, Backend::Stream] {
        let dir = Directory::new();
        let files = dir.files(backend);
        let xml = dir.0.join("solver.xml");
        let io = SolutionFileIo::new(RosterCodec);
        io.write(&roster, &files.input).unwrap();
        std::fs::write(
            &files.config,
            configuration_binding().encode(&settings).unwrap(),
        )
        .unwrap();
        std::fs::write(&xml, "<solver><termination><spentLimit>PT30S</spentLimit><scoreCalculationCountLimit>10000</scoreCalculationCountLimit><bestScoreLimit>0hard/0soft</bestScoreLimit></termination><constructionHeuristic/><localSearch><termination><stepCountLimit>16</stepCountLimit></termination></localSearch></solver>").unwrap();
        let retained = std::fs::read(&files.input).unwrap();
        let result =
            run_files_with_xml(&files.input, &files.config, &xml, &files.output, backend).unwrap();
        assert_eq!(result.roster.score, Some(expected));
        assert_eq!(independent_score(&result.roster, &settings), expected);
        assert!(
            result
                .roster
                .shifts
                .iter()
                .all(|s| s.employee == Some(s.preferred))
        );
        assert!(result.work.evaluations > 0 && result.work.evaluations <= 10000);
        let reread: Roster = io.read(&files.output).unwrap();
        assert_eq!(reread, result.roster);
        assert_eq!(std::fs::read(&files.input).unwrap(), retained);
    }
}
