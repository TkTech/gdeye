//! Error types for gdeye operations.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using gdeye's Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during gdeye operations.
#[derive(Error, Debug)]
pub enum Error {
    // ============ File I/O Errors ============
    #[error("Failed to read file {path}: {source}")]
    FileRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to write file {path}: {source}")]
    FileWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    // ============ Parse Errors ============
    #[error("Failed to parse {path}: {message}")]
    ParseError { path: PathBuf, message: String },

    #[error("Failed to parse scene file {path}: {message}")]
    SceneParseError { path: PathBuf, message: String },

    #[error("Failed to parse project.godot: {message}")]
    ProjectParseError { message: String },

    // ============ Configuration Errors ============
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid glob pattern '{pattern}': {message}")]
    InvalidGlobPattern { pattern: String, message: String },

    #[error("Unknown rule: {0}")]
    UnknownRule(String),

    // ============ ClassDB Errors ============
    #[error("Godot binary not found in PATH")]
    GodotNotFound,

    #[error("Godot --dump-extension-api failed: {0}")]
    GodotDumpFailed(String),

    #[error("No bundled ClassDB versions available")]
    NoBundledVersions,

    #[error("No bundled ClassDB matching version '{version}'. Available: {available}")]
    UnknownVersion { version: String, available: String },

    #[error("Failed to decompress bundled ClassDB v{version}: {source}")]
    DecompressError {
        version: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse ClassDB JSON: {0}")]
    ClassDbJsonError(#[from] serde_json::Error),

    // ============ Analysis Errors ============
    #[error("No .gd files found to analyze")]
    NoFilesFound,

    #[error("Analysis failed for {path}: {message}")]
    AnalysisError { path: PathBuf, message: String },

    // ============ Generic I/O ============
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Create a file read error.
    pub fn file_read(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::FileRead {
            path: path.into(),
            source,
        }
    }

    /// Create a file write error.
    pub fn file_write(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::FileWrite {
            path: path.into(),
            source,
        }
    }

    /// Create a parse error.
    pub fn parse(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Error::ParseError {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Error::ConfigError(message.into())
    }
}

// Keep the old name as an alias for backwards compatibility within the crate
pub type GdEyeError = Error;
