//! Errors of the command-line runner.

use std::fmt;
use std::path::PathBuf;

use incentive_core::ConfigError;

/// Why a command failed.
#[derive(Debug)]
pub enum CliError {
    /// The configuration or an assignment was rejected.
    Config(ConfigError),
    /// A file could not be read or written.
    Io {
        /// The file or directory.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The seed list could not be read.
    Seeds(String),
    /// A metric key is unknown.
    UnknownMetric(String),
    /// Output could not be serialised.
    Json(String),
    /// Standard output could not be written (for example a closed pipe).
    Stdout(std::io::Error),
    /// An exploration option or run failed.
    Explore(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(e) => write!(f, "{e}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Seeds(spec) => write!(
                f,
                "invalid seeds {spec:?}: use a range like 1..8 or a list like 1,5,9"
            ),
            Self::UnknownMetric(key) => {
                write!(f, "unknown metric {key:?}; see the keys in a summary JSON")
            }
            Self::Json(message) => write!(f, "could not write JSON: {message}"),
            Self::Stdout(e) => write!(f, "could not write to standard output: {e}"),
            Self::Explore(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<ConfigError> for CliError {
    fn from(e: ConfigError) -> Self {
        Self::Config(e)
    }
}
