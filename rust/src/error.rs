/// Centralized error type for the Reel codebase.
///
/// Replaces `Result<T, String>` throughout. The `From<AppError> for String` impl
/// enables gradual migration: functions still returning `Result<T, String>` can
/// use `?` on functions returning `AppResult<T>`.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum AppError {
    NoLibraryPath,
    NotFound(PathBuf),
    Api { service: &'static str, detail: String },
    Db(String),
    Io(io::Error),
    Parse(String),
    Process(String),
    Cancelled,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLibraryPath => write!(f, "No library path configured"),
            Self::NotFound(p) => write!(f, "Not found: {}", p.display()),
            Self::Api { service, detail } => write!(f, "{} API error: {}", service, detail),
            Self::Db(msg) => write!(f, "Database error: {}", msg),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Parse(msg) => write!(f, "Parse error: {}", msg),
            Self::Process(msg) => write!(f, "Processing error: {}", msg),
            Self::Cancelled => write!(f, "Operation cancelled"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e.to_string())
    }
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

pub type AppResult<T> = Result<T, AppError>;
