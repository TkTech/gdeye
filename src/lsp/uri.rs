//! URI/path conversion utilities using the `url` crate.

use std::path::{Path, PathBuf};

use url::Url;

/// Errors that can occur during URI conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UriError {
    /// The URI is not a valid file:// URI.
    InvalidUri,
    /// The URI cannot be converted to a file path.
    NotFilePath,
}

impl std::fmt::Display for UriError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UriError::InvalidUri => write!(f, "invalid URI"),
            UriError::NotFilePath => write!(f, "URI is not a file path"),
        }
    }
}

impl std::error::Error for UriError {}

/// Convert a file URI to a filesystem path.
pub fn uri_to_path(uri: &str) -> Result<PathBuf, UriError> {
    let url = Url::parse(uri).map_err(|_| UriError::InvalidUri)?;
    url.to_file_path().map_err(|_| UriError::NotFilePath)
}

/// Convert a filesystem path to a file URI.
pub fn path_to_uri(path: &Path) -> String {
    Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", path.display()))
}

/// Convert a file URI to a path, falling back to treating it as a raw path.
pub fn uri_to_path_lossy(uri: &str) -> PathBuf {
    uri_to_path(uri).unwrap_or_else(|_| PathBuf::from(uri))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_uri_returns_error() {
        let uri = "not a valid uri %%%";
        assert_eq!(uri_to_path(uri), Err(UriError::InvalidUri));
    }

    #[test]
    fn non_file_uri_returns_error() {
        let uri = "http://example.com/test.gd";
        assert_eq!(uri_to_path(uri), Err(UriError::NotFilePath));
    }

    #[test]
    fn lossy_fallback_on_invalid_uri() {
        let raw_path = "/home/user/test.gd";
        let path = uri_to_path_lossy(raw_path);
        assert_eq!(path, PathBuf::from(raw_path));
    }
}
