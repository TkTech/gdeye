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
    fn simple_unix_path() {
        let uri = "file:///home/user/test.gd";
        let path = uri_to_path(uri).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/test.gd"));
    }

    #[test]
    fn path_with_spaces() {
        let uri = "file:///home/user/my%20project/test.gd";
        let path = uri_to_path(uri).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/my project/test.gd"));
    }

    #[test]
    fn path_with_special_chars() {
        let uri = "file:///home/user/project%23test/file%5B1%5D.gd";
        let path = uri_to_path(uri).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/project#test/file[1].gd"));
    }

    #[test]
    fn not_file_uri() {
        let uri = "http://example.com/test.gd";
        assert!(uri_to_path(uri).is_err());
    }

    #[test]
    fn roundtrip_simple() {
        let original = Path::new("/home/user/test.gd");
        let uri = path_to_uri(original);
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, original);
    }

    #[test]
    fn roundtrip_with_spaces() {
        let original = Path::new("/home/user/my project/test file.gd");
        let uri = path_to_uri(original);
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, original);
    }

    #[test]
    fn roundtrip_with_special_chars() {
        let original = Path::new("/home/user/project#1/file[test].gd");
        let uri = path_to_uri(original);
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, original);
    }

    #[test]
    fn uri_to_path_lossy_fallback() {
        let raw_path = "/home/user/test.gd";
        let path = uri_to_path_lossy(raw_path);
        assert_eq!(path, PathBuf::from(raw_path));
    }

    #[test]
    fn path_to_uri_encoding() {
        let path = Path::new("/home/user/my project/test.gd");
        let uri = path_to_uri(path);
        assert!(uri.contains("my%20project"));
        assert!(!uri.contains(' '));
    }

    #[test]
    #[cfg(windows)]
    fn windows_uri_to_path() {
        let uri = "file:///C:/Users/test/project/test.gd";
        let path = uri_to_path(uri).unwrap();
        assert_eq!(path, PathBuf::from("C:\\Users\\test\\project\\test.gd"));
    }

    #[test]
    #[cfg(windows)]
    fn windows_path_to_uri() {
        let path = Path::new("C:\\Users\\test\\project\\test.gd");
        let uri = path_to_uri(path);
        assert_eq!(uri, "file:///C:/Users/test/project/test.gd");
    }

    #[test]
    #[cfg(windows)]
    fn windows_roundtrip() {
        let original = Path::new("C:\\Users\\test\\my project\\test.gd");
        let uri = path_to_uri(original);
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, original);
    }
}
