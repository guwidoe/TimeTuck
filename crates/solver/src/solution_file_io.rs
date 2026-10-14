//! Optional owned-solution file persistence with executable caller codecs.
use std::{
    error::Error,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Debug)]
pub enum SolutionFileError {
    Io(io::Error),
    Codec(String),
    InvalidLimit,
    TooLarge { limit: usize },
}
impl fmt::Display for SolutionFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Codec(e) => write!(f, "solution codec: {e}"),
            Self::InvalidLimit => {
                f.write_str("solution byte limit must be positive and below usize::MAX")
            }
            Self::TooLarge { limit } => write!(f, "solution exceeds byte limit ({limit})"),
        }
    }
}
impl Error for SolutionFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for SolutionFileError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
/// Actual owned model conversion, supplied explicitly rather than inferred from
/// reflection. Callbacks own their effects; format compatibility is codec-specific.
pub trait SolutionCodec<S> {
    fn encode(&self, solution: &S) -> Result<Vec<u8>, String>;
    fn decode(&self, input: &[u8]) -> Result<S, String>;
}
pub struct SolutionFileIo<C> {
    codec: C,
    limit: usize,
}
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
struct PendingFile(Option<PathBuf>);
impl Drop for PendingFile {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            let _ = fs::remove_file(p);
        }
    }
}
impl<C> SolutionFileIo<C> {
    pub fn new(codec: C) -> Self {
        Self {
            codec,
            limit: 16 * 1024 * 1024,
        }
    }
    pub fn with_byte_limit(mut self, limit: usize) -> Result<Self, SolutionFileError> {
        if limit == 0 || limit == usize::MAX {
            return Err(SolutionFileError::InvalidLimit);
        }
        self.limit = limit;
        Ok(self)
    }
    pub fn codec(&self) -> &C {
        &self.codec
    }
    pub fn read<S>(&self, path: impl AsRef<Path>) -> Result<S, SolutionFileError>
    where
        C: SolutionCodec<S>,
    {
        let file = File::open(path)?;
        let mut bytes = vec![];
        file.take((self.limit + 1) as u64).read_to_end(&mut bytes)?;
        if bytes.len() > self.limit {
            return Err(SolutionFileError::TooLarge { limit: self.limit });
        }
        self.codec.decode(&bytes).map_err(SolutionFileError::Codec)
    }
    /// Encode/validate before touching the destination, then publish via a sibling
    /// temporary file and rename. Rename replacement follows host filesystem rules;
    /// no directory fsync/crash-durability guarantee is made.
    pub fn write<S>(&self, solution: &S, path: impl AsRef<Path>) -> Result<(), SolutionFileError>
    where
        C: SolutionCodec<S>,
    {
        let bytes = self
            .codec
            .encode(solution)
            .map_err(SolutionFileError::Codec)?;
        if bytes.len() > self.limit {
            return Err(SolutionFileError::TooLarge { limit: self.limit });
        }
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = None;
        for _ in 0..16 {
            let name = parent.join(format!(
                ".timetuck-solution-{}-{}.tmp",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new().write(true).create_new(true).open(&name) {
                Ok(file) => {
                    temporary = Some((file, name));
                    break;
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
        }
        let (mut file, name) = temporary.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "unable to allocate solution temporary file",
            )
        })?;
        let mut pending = PendingFile(Some(name));
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(pending.0.as_ref().unwrap(), path)?;
        pending.0 = None;
        Ok(())
    }
}
