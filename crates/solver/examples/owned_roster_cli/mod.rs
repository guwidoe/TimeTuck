//! File invocation for the original owned roster application.
use crate::owned_roster_support::{Backend, run_files, run_files_with_xml};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const HELP: &str = "Solve an owned roster document. With no arguments, create and solve a demo.\n\nUsage: owned_roster_workflow --input PATH --config PATH --output PATH\n                            [--backend dense|stream] [--solver-xml PATH]\n\n  --input PATH       Roster JSON input\n  --config PATH      Roster configuration JSON\n  --output PATH      Solved JSON destination (must differ from source files)\n  --backend NAME     dense (default) or stream\n  --solver-xml PATH  Optional solver XML controls\n  -h, --help         Show this help\n  --version          Show native package version\n\nOutput: saved JSON plus a concise stdout summary; errors go to stderr.\nExit codes: 0 success, 1 application failure, 2 invalid arguments.\n";

#[derive(Debug)]
pub struct Files {
    pub input: PathBuf,
    pub config: PathBuf,
    pub output: PathBuf,
    pub backend: Backend,
    pub solver_xml: Option<PathBuf>,
}
#[derive(Debug)]
pub enum Invocation {
    Demo,
    Help,
    Version,
    Files(Files),
}
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Invocation, String> {
    let args: Vec<_> = args.into_iter().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return Ok(Invocation::Help);
    }
    if args.iter().any(|a| a == "--version") {
        return Ok(Invocation::Version);
    }
    if args.is_empty() {
        return Ok(Invocation::Demo);
    }
    let (mut input, mut config, mut output, mut backend, mut solver_xml) =
        (None, None, None, None, None);
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let slot = match flag.to_str() {
            Some("--input") => &mut input,
            Some("--config") => &mut config,
            Some("--output") => &mut output,
            Some("--backend") => &mut backend,
            Some("--solver-xml") => &mut solver_xml,
            _ => return Err(format!("unknown argument {flag:?}; use --help")),
        };
        if slot.is_some() {
            return Err(format!("duplicate argument {flag:?}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag:?}"))?;
        if value.is_empty() || value.to_str().is_some_and(|v| v.starts_with("--")) {
            return Err(format!("missing value for {flag:?}"));
        }
        *slot = Some(value);
    }
    let backend = match backend.as_deref().and_then(|a| a.to_str()) {
        None if backend.is_none() => Backend::Dense,
        Some("dense") => Backend::Dense,
        Some("stream") => Backend::Stream,
        _ => return Err("--backend must be dense or stream".into()),
    };
    Ok(Invocation::Files(Files {
        input: input.ok_or("required argument --input")?.into(),
        config: config.ok_or("required argument --config")?.into(),
        output: output.ok_or("required argument --output")?.into(),
        backend,
        solver_xml: solver_xml.map(PathBuf::from),
    }))
}

/// Check existing symlink and hard-link aliases before an output replacement.
fn guard_output(output: &Path, sources: impl IntoIterator<Item = PathBuf>) -> Result<(), String> {
    let output_metadata = match std::fs::metadata(output) {
        Ok(m) => Some(m),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("cannot inspect output: {e}")),
    };
    let output_canonical = output_metadata
        .as_ref()
        .map(|_| std::fs::canonicalize(output))
        .transpose()
        .map_err(|e| e.to_string())?;
    for source in sources {
        let canonical = std::fs::canonicalize(&source)
            .map_err(|e| format!("cannot inspect {}: {e}", source.display()))?;
        let mut alias = output_canonical.as_ref() == Some(&canonical);
        #[cfg(unix)]
        if let Some(output_metadata) = &output_metadata {
            use std::os::unix::fs::MetadataExt;
            let source_metadata = std::fs::metadata(&source).map_err(|e| e.to_string())?;
            alias |= output_metadata.dev() == source_metadata.dev()
                && output_metadata.ino() == source_metadata.ino();
        }
        if alias {
            return Err(format!("output aliases source file {}", source.display()));
        }
    }
    Ok(())
}

pub fn execute(files: &Files) -> Result<String, String> {
    let mut sources = vec![files.input.clone(), files.config.clone()];
    sources.extend(files.solver_xml.iter().cloned());
    guard_output(&files.output, sources)?;
    let result = match &files.solver_xml {
        Some(xml) => run_files_with_xml(
            &files.input,
            &files.config,
            xml,
            &files.output,
            files.backend,
        ),
        None => run_files(&files.input, &files.config, &files.output, files.backend),
    }?;
    let score = result.roster.score.ok_or("solve returned no score")?;
    Ok(format!(
        "{:?}: {} ({} evaluations); {}",
        files.backend,
        score,
        result.work.evaluations,
        files.output.display()
    ))
}
