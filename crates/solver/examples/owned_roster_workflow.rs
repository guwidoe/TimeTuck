//! Original complete configuration/file/manager scheduling workflow.
mod owned_roster_support;
use owned_roster_support::*;
use timetuck::SolutionFileIo;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!(
        "timetuck-roster-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&dir)?;
    let input = dir.join("input.json");
    let config = dir.join("configuration.json");
    SolutionFileIo::new(RosterCodec).write(&sample(), &input)?;
    std::fs::write(
        &config,
        configuration_binding().encode(&Configuration::default())?,
    )?;
    configuration_binding().write_schema(dir.join("configuration-schema.json"))?;
    for backend in [Backend::Dense, Backend::Stream] {
        let output = dir.join(format!("{backend:?}-solved.json"));
        let solved = run_files(&input, &config, &output, backend)?;
        println!(
            "{backend:?}: {} ({} evaluations); {}",
            solved.roster.score.unwrap(),
            solved.work.evaluations,
            output.display()
        );
    }
    println!(
        "Input, configuration and generated schema: {}",
        dir.display()
    );
    Ok(())
}
