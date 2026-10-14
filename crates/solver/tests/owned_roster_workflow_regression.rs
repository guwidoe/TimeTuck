//! Original application-level qualification; no upstream-method credit.
#[path = "../examples/owned_roster_support/mod.rs"]
mod app;
use app::*;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use timetuck::*;
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "timetuck-roster-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn optimum(input: &Roster, config: &Configuration) -> HardSoftScore {
    fn visit(r: &mut Roster, i: usize, config: &Configuration, best: &mut Option<HardSoftScore>) {
        if i == r.shifts.len() {
            let score = independent_score(r, config);
            if best.is_none_or(|b| score > b) {
                *best = Some(score);
            }
            return;
        }
        for e in 0..r.employees.len() {
            r.shifts[i].employee = Some(e);
            visit(r, i + 1, config, best);
        }
    }
    let mut best = None;
    visit(&mut input.clone(), 0, config, &mut best);
    best.unwrap()
}
#[test]
fn real_file_workflow_reaches_independent_optimum_and_preserves_input() {
    for backend in [Backend::Dense, Backend::Stream] {
        let dir = Directory::new();
        let input = dir.0.join("input.json");
        let config = dir.0.join("config.json");
        let output = dir.0.join("output.json");
        let original = sample();
        let settings = Configuration::default();
        let io = SolutionFileIo::new(RosterCodec);
        io.write(&original, &input).unwrap();
        let original_bytes = std::fs::read(&input).unwrap();
        let binding = configuration_binding();
        std::fs::write(&config, binding.encode(&settings).unwrap()).unwrap();
        binding.write_schema(dir.0.join("schema.json")).unwrap();
        let solved = run_files(&input, &config, &output, backend).unwrap();
        assert_eq!(solved.roster.score, Some(optimum(&original, &settings)));
        assert_eq!(solved.roster.score, Some(HardSoftScore::new(0, -6)));
        assert!(solved.roster.shifts.iter().all(|s| s.employee.is_some()));
        assert!(solved.phases[1].work.completed_steps <= settings.steps as u64);
        assert_eq!(
            solved.phases[1].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::NoAcceptedMove)
        );
        assert_eq!(std::fs::read(input).unwrap(), original_bytes);
        let readback: Roster = io.read(output).unwrap();
        assert_eq!(readback, solved.roster);
        let schema: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.0.join("schema.json")).unwrap()).unwrap();
        assert!(
            schema["$defs"]["RosterConfiguration"]["properties"]["preference_weight"].is_object()
        );
    }
}
#[test]
fn same_owned_factory_uses_each_jobs_current_intervals_staff_and_preferences() {
    for backend in [Backend::Dense, Backend::Stream] {
        let settings = Configuration {
            steps: 7,
            ..Default::default()
        };
        let f = factory(settings.clone(), backend).unwrap();
        let mut manager = f.build_manager(2).unwrap();
        let first = Arc::new(sample());
        let mut second = sample();
        second.employees.push(Employee {
            id: "Dev".into(),
            skills: vec!["doctor".into(), "nurse".into()],
        });
        for (i, s) in second.shifts.iter_mut().enumerate() {
            s.preferred = 3;
            s.start = (i as i64) * 10;
            s.end = s.start + 5;
        }
        let second = Arc::new(second);
        let snapshots = [first.as_ref().clone(), second.as_ref().clone()];
        let jobs = [
            manager
                .job_builder(1)
                .with_problem(first.clone())
                .run()
                .unwrap(),
            manager
                .job_builder(2)
                .with_problem(second.clone())
                .run()
                .unwrap(),
        ];
        drop(f);
        for (job, input) in jobs.into_iter().zip(&snapshots) {
            let result = job.wait();
            let solved = result.as_ref().as_ref().unwrap();
            assert_eq!(solved.roster.score, Some(optimum(input, &settings)));
            assert_eq!(
                solved.roster.score,
                Some(independent_score(&solved.roster, &settings))
            );
            assert_eq!(solved.roster.employees, input.employees);
        }
        assert_eq!(first.as_ref(), &snapshots[0]);
        assert_eq!(second.as_ref(), &snapshots[1]);
        manager.shutdown().unwrap();
    }
}
#[test]
fn invalid_input_and_configuration_preserve_existing_output_document() {
    let dir = Directory::new();
    let input = dir.0.join("input.json");
    let config = dir.0.join("config.json");
    let output = dir.0.join("output.json");
    let io = SolutionFileIo::new(RosterCodec);
    io.write(&sample(), &input).unwrap();
    std::fs::write(&output, b"existing result").unwrap();
    for settings in [
        r#"{"steps":-1}"#,
        r#"{"preference_weight":-1}"#,
        r#"{"unknown":42}"#,
        r#"{"steps":"oops"}"#,
    ] {
        std::fs::write(&config, settings).unwrap();
        assert!(run_files(&input, &config, &output, Backend::Dense).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"existing result");
    }
    std::fs::write(&config, "{}").unwrap();
    let bytes = RosterCodec.encode(&sample()).unwrap();
    let mut doc: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    doc["shifts"][0]["employee"] = serde_json::json!("unknown employee");
    std::fs::write(&input, serde_json::to_vec(&doc).unwrap()).unwrap();
    assert!(run_files(&input, &config, &output, Backend::Stream).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"existing result");
}
#[test]
fn zero_deadline_publishes_actual_partial_score_and_reweighted_solve_is_feasible() {
    for backend in [Backend::Dense, Backend::Stream] {
        let settings = Configuration {
            preference_weight: 0,
            ..Default::default()
        };
        let f = factory(settings.clone(), backend).unwrap();
        let mut manager = f.build_manager(1).unwrap();
        let input = Arc::new(sample());
        let limited = manager
            .job_builder(1)
            .with_problem(input.clone())
            .with_time_override(SolverTimeOverride {
                max_time: Some(Duration::ZERO),
                ..Default::default()
            })
            .run()
            .unwrap();
        let limited = limited.wait();
        let limited = limited.as_ref().as_ref().unwrap();
        assert!(limited.roster.shifts.iter().all(|s| s.employee.is_none()));
        assert_eq!(
            limited.roster.score,
            Some(independent_score(&limited.roster, &settings))
        );
        assert_eq!(limited.work.completed_steps, 0);
        let codec = RosterCodec;
        assert_eq!(
            codec
                .decode(&codec.encode(&limited.roster).unwrap())
                .unwrap(),
            limited.roster
        );
        let solved = manager
            .job_builder(2)
            .with_problem(input)
            .run()
            .unwrap()
            .wait();
        let solved = solved.as_ref().as_ref().unwrap();
        assert_eq!(solved.roster.score, Some(HardSoftScore::new(0, 0)));
        assert!(solved.roster.shifts.iter().all(|s| s.employee.is_some()));
        manager.shutdown().unwrap();
    }
}
#[test]
fn warm_start_xml_step_caps_change_work_and_return_quality_on_both_backends() {
    let mut input = sample();
    for s in &mut input.shifts {
        s.employee = Some(2);
    }
    for backend in [Backend::Dense, Backend::Stream] {
        let mut scores = Vec::new();
        for steps in [1, 40] {
            let settings = Configuration {
                steps,
                ..Default::default()
            };
            let f = factory(settings.clone(), backend).unwrap();
            let mut manager = f.build_manager(1).unwrap();
            let retained = Arc::new(input.clone());
            let result = manager
                .job_builder(1)
                .with_problem(retained.clone())
                .run()
                .unwrap()
                .wait();
            let solved = result.as_ref().as_ref().unwrap();
            assert_eq!(retained.as_ref(), &input);
            assert_eq!(
                solved.roster.score,
                Some(independent_score(&solved.roster, &settings))
            );
            assert!(solved.phases[1].work.completed_steps <= steps as u64);
            if steps == 1 {
                assert_eq!(solved.phases[1].work.completed_steps, 1);
                assert_eq!(
                    solved.phases[1].termination,
                    PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
                );
            } else {
                assert_eq!(solved.roster.score, Some(optimum(&input, &settings)));
            }
            scores.push(solved.roster.score.unwrap());
            manager.shutdown().unwrap();
        }
        assert!(scores[1] > scores[0]);
    }
}
#[test]
fn custom_xml_root_history_stops_real_warm_roster_and_round_trips() {
    for backend in [Backend::Dense, Backend::Stream] {
        let dir = Directory::new();
        let input = dir.0.join("input.json");
        let config = dir.0.join("config.json");
        let xml = dir.0.join("solver.xml");
        let output = dir.0.join("output.json");
        let settings = Configuration::default();
        let mut original = sample();
        for shift in &mut original.shifts {
            shift.employee = Some(2);
        }
        let io = SolutionFileIo::new(RosterCodec);
        io.write(&original, &input).unwrap();
        let input_bytes = std::fs::read(&input).unwrap();
        std::fs::write(&config, configuration_binding().encode(&settings).unwrap()).unwrap();
        std::fs::write(&xml, "<solver><termination><unimprovedMillisecondsSpentLimit>0</unimprovedMillisecondsSpentLimit><unimprovedScoreDifferenceThreshold>100hard/100soft</unimprovedScoreDifferenceThreshold></termination><constructionHeuristic/><localSearch/></solver>").unwrap();
        for _ in 0..2 {
            let limited = run_files_with_xml(&input, &config, &xml, &output, backend).unwrap();
            assert_eq!(limited.termination, SolverTermination::Condition);
            assert!(limited.work.completed_steps <= 1);
            assert!(limited.work.evaluations < 100);
            assert!(limited.roster.score.unwrap() < optimum(&original, &settings));
            assert_eq!(
                limited.roster.score,
                Some(independent_score(&limited.roster, &settings))
            );
            assert!(limited.roster.shifts.iter().all(|s| s.employee.is_some()));
            let readback: Roster = io.read(&output).unwrap();
            assert_eq!(readback, limited.roster);
            assert_eq!(std::fs::read(&input).unwrap(), input_bytes);
        }
        let uncapped = run_files(&input, &config, &output, backend).unwrap();
        assert_eq!(uncapped.roster.score, Some(optimum(&original, &settings)));
        assert!(uncapped.work.completed_steps > 1);
    }
}
#[test]
fn invalid_custom_xml_preserves_existing_output_on_both_backends() {
    for backend in [Backend::Dense, Backend::Stream] {
        let dir = Directory::new();
        let input = dir.0.join("input.json");
        let config = dir.0.join("config.json");
        let xml = dir.0.join("solver.xml");
        let output = dir.0.join("output.json");
        SolutionFileIo::new(RosterCodec)
            .write(&sample(), &input)
            .unwrap();
        std::fs::write(&config, "{}").unwrap();
        std::fs::write(&output, b"existing result").unwrap();
        for invalid in [
            "<solver>",
            "<solver><localSearch/></solver>",
            "<solver><constructionHeuristic/><localSearch/><localSearch/></solver>",
            "<solver><termination><unimprovedMillisecondsSpentLimit>0</unimprovedMillisecondsSpentLimit><unimprovedScoreDifferenceThreshold>bad</unimprovedScoreDifferenceThreshold></termination><constructionHeuristic/><localSearch/></solver>",
            "<solver><termination><unimprovedScoreDifferenceThreshold>1hard/1soft</unimprovedScoreDifferenceThreshold></termination><constructionHeuristic/><localSearch/></solver>",
        ] {
            std::fs::write(&xml, invalid).unwrap();
            assert!(
                run_files_with_xml(&input, &config, &xml, &output, backend).is_err(),
                "{invalid}"
            );
            assert_eq!(std::fs::read(&output).unwrap(), b"existing result");
        }
    }
}
