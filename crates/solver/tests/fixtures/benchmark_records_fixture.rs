// Adapted TEST-only result graph helper, Apache-2.0; see compat/provenance.json.
use std::{path::PathBuf, sync::Arc};
use timetuck::*;
pub fn selected_records() -> Vec<RecordSelection> {
    let solvers: [Arc<RecordSolver>; 3] = std::array::from_fn(|i| {
        Arc::new(RecordSolver {
            name: format!("Solver {}", ['X', 'Y', 'Z'][i]),
            sub_runs: 1,
            config: SolverXmlConfiguration::parse(&format!(
                "<solver><randomSeed>{i}</randomSeed></solver>"
            ))
            .unwrap(),
            runs: vec![],
        })
    });
    let a = Arc::new(RecordProblem::from_file(PathBuf::from("problemA.xml")).unwrap());
    let b = Arc::new(RecordProblem::from_file(PathBuf::from("problemB.xml")).unwrap());
    let second_a = Arc::new(RecordProblem::from_file(PathBuf::from("problemA.xml")).unwrap());
    [
        (0, a.clone(), -1),
        (0, b.clone(), -20),
        (1, a, -300),
        (1, b, -4000),
        (2, second_a, -50000),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (solver, problem, score))| RecordSelection {
        solver: solvers[solver].clone(),
        problem,
        run: RecordSingleRun {
            id: id as u64 + 1,
            summary_score: Some(RecordScore::Simple(SimpleScore::new(score))),
            sub_runs: if id == 0 {
                vec![
                    RecordSubRun {
                        index: 0,
                        ..Default::default()
                    },
                    RecordSubRun {
                        index: 1,
                        ..Default::default()
                    },
                ]
            } else {
                vec![RecordSubRun::default()]
            },
            ..Default::default()
        },
    })
    .collect()
}
