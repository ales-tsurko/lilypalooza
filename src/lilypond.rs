use std::fmt;
use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use thiserror::Error;

/// LilyPond executable name looked up in `PATH`.
pub const LILYPOND_BIN: &str = "lilypond";
/// Minimum supported LilyPond version used by [`check_lilypond`].
pub const MIN_LILYPOND_VERSION: Version = Version::new(2, 24, 0);

/// Checks that LilyPond is installed and satisfies [`MIN_LILYPOND_VERSION`].
pub fn check_lilypond() -> Result<VersionCheck, LilypondError> {
    check_lilypond_with_min_version(MIN_LILYPOND_VERSION)
}

/// Checks that LilyPond is installed and satisfies the provided minimum version.
pub fn check_lilypond_with_min_version(
    min_required: Version,
) -> Result<VersionCheck, LilypondError> {
    let output = Command::new(LILYPOND_BIN)
        .arg("--version")
        .output()
        .map_err(map_spawn_error)?;

    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(LilypondError::CommandFailed {
            context: "lilypond --version",
            details,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    let detected = parse_version(&combined).ok_or(LilypondError::VersionParseFailed {
        raw_output: combined,
    })?;

    if detected < min_required {
        return Err(LilypondError::VersionTooOld {
            detected,
            min_required,
        });
    }

    Ok(VersionCheck {
        detected,
        min_required,
    })
}

/// Starts a LilyPond compilation process and returns a session for non-blocking log polling.
pub fn spawn_compile(request: CompileRequest) -> Result<CompileSession, LilypondError> {
    let mut command = Command::new(LILYPOND_BIN);
    command
        .args(&request.args)
        .arg(&request.score_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(working_dir) = request.working_dir {
        command.current_dir(working_dir);
    }

    let mut child = command.spawn().map_err(map_spawn_error)?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| LilypondError::CommandFailed {
            context: "spawn lilypond compile process",
            details: "stdout pipe was not available".to_string(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| LilypondError::CommandFailed {
            context: "spawn lilypond compile process",
            details: "stderr pipe was not available".to_string(),
        })?;

    let (event_tx, event_rx) = mpsc::channel();

    let stdout_handle = spawn_log_reader(stdout, LogStream::Stdout, event_tx.clone());
    let stderr_handle = spawn_log_reader(stderr, LogStream::Stderr, event_tx.clone());

    thread::spawn(move || {
        let wait_result = child.wait();

        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        match wait_result {
            Ok(status) => {
                let _ = event_tx.send(CompileEvent::Finished {
                    success: status.success(),
                    exit_code: status.code(),
                });
            }
            Err(error) => {
                let _ = event_tx.send(CompileEvent::ProcessError(format!(
                    "failed to wait for LilyPond process: {error}"
                )));
            }
        }
    });

    Ok(CompileSession { events: event_rx })
}

/// Semantic LilyPond version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// Major version number.
    pub major: u16,
    /// Minor version number.
    pub minor: u16,
    /// Patch version number.
    pub patch: u16,
}

impl Version {
    /// Creates a new version value.
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Result of a LilyPond version compatibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionCheck {
    /// Installed LilyPond version.
    pub detected: Version,
    /// Minimum required version used during the check.
    pub min_required: Version,
}

/// Errors produced by LilyPond checks and compile process startup.
#[derive(Debug, Error)]
pub enum LilypondError {
    /// LilyPond binary could not be found in `PATH`.
    #[error("binary not found in PATH: {bin}")]
    BinaryNotFound { bin: &'static str },
    /// A LilyPond command failed with a non-zero status.
    #[error("{context} failed: {details}")]
    CommandFailed {
        context: &'static str,
        details: String,
    },
    /// LilyPond version could not be parsed from command output.
    #[error("failed to parse LilyPond version from output: {raw_output}")]
    VersionParseFailed { raw_output: String },
    /// Installed LilyPond version is below the configured minimum.
    #[error("LilyPond version {detected} is below minimum required version {min_required}")]
    VersionTooOld {
        detected: Version,
        min_required: Version,
    },
    /// I/O error while spawning or communicating with the process.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Compile request configuration for a single LilyPond invocation.
#[derive(Debug, Clone)]
pub struct CompileRequest {
    /// Path to the `.ly` score file.
    pub score_path: PathBuf,
    /// Extra CLI arguments passed to LilyPond.
    pub args: Vec<String>,
    /// Optional working directory for the process.
    pub working_dir: Option<PathBuf>,
}

impl CompileRequest {
    /// Creates a request for the given score path.
    pub fn new(score_path: impl Into<PathBuf>) -> Self {
        Self {
            score_path: score_path.into(),
            args: Vec::new(),
            working_dir: None,
        }
    }
}

/// Origin stream of a log line from LilyPond process output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Event stream emitted by a running compile session.
#[derive(Debug, Clone)]
pub enum CompileEvent {
    /// A single line from LilyPond process output.
    Log { stream: LogStream, line: String },
    /// Internal process/pipe handling error.
    ProcessError(String),
    /// Process completion notification.
    Finished {
        success: bool,
        exit_code: Option<i32>,
    },
}

/// Handle used by the GUI loop to receive compile events.
pub struct CompileSession {
    events: Receiver<CompileEvent>,
}

impl CompileSession {
    /// Non-blocking receive for GUI polling loops.
    pub fn try_recv(&self) -> Result<CompileEvent, TryRecvError> {
        self.events.try_recv()
    }
}

fn spawn_log_reader<R>(
    reader: R,
    stream: LogStream,
    event_tx: Sender<CompileEvent>,
) -> thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(reader);

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if event_tx.send(CompileEvent::Log { stream, line }).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = event_tx.send(CompileEvent::ProcessError(format!(
                        "failed to read LilyPond log: {error}"
                    )));
                    break;
                }
            }
        }
    })
}

fn map_spawn_error(error: io::Error) -> LilypondError {
    if error.kind() == io::ErrorKind::NotFound {
        LilypondError::BinaryNotFound { bin: LILYPOND_BIN }
    } else {
        LilypondError::Io(error)
    }
}

fn parse_version(raw: &str) -> Option<Version> {
    raw.split_whitespace().find_map(parse_version_token)
}

fn parse_version_token(token: &str) -> Option<Version> {
    let trimmed = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split('.');
    let major = parse_numeric_prefix(parts.next()?)?;
    let minor = parse_numeric_prefix(parts.next()?)?;
    let patch = parts.next().and_then(parse_numeric_prefix).unwrap_or(0);

    Some(Version::new(major, minor, patch))
}

fn parse_numeric_prefix(part: &str) -> Option<u16> {
    let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{Version, parse_version};

    #[test]
    fn parses_version_from_lilypond_output() {
        let output = "GNU LilyPond 2.24.3 (running Guile 2.2)";
        let version = parse_version(output);
        assert_eq!(version, Some(Version::new(2, 24, 3)));
    }

    #[test]
    fn parses_version_with_trailing_punctuation() {
        let output = "GNU LilyPond 2.25.1, some extra text";
        let version = parse_version(output);
        assert_eq!(version, Some(Version::new(2, 25, 1)));
    }

    #[test]
    fn version_ordering_works() {
        assert!(Version::new(2, 24, 0) > Version::new(2, 23, 82));
    }
}
