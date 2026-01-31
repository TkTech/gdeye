//! Document representation with versioning and position mapping.
//!
//! This module provides the [`Document`] type for representing versioned source files,
//! along with utilities for converting between different position representations
//! (byte offsets, line/column, LSP positions).

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A versioned document representing a source file.
///
/// Documents are designed to be cheaply cloneable (source is reference-counted)
/// and provide efficient position mapping between different representations.
#[derive(Debug, Clone)]
pub struct Document {
    /// The file path (or URI for unsaved files).
    pub path: PathBuf,
    /// Version number, incremented on each edit.
    pub version: i32,
    /// The source content (shared for cheap cloning).
    content: Arc<str>,
    /// Line start byte offsets for position mapping.
    line_starts: Arc<[usize]>,
}

impl Document {
    /// Create a new document with version 0.
    pub fn new(path: impl Into<PathBuf>, content: impl Into<String>) -> Self {
        let content: Arc<str> = content.into().into();
        let line_starts = compute_line_starts(&content);
        Self {
            path: path.into(),
            version: 0,
            content,
            line_starts: line_starts.into(),
        }
    }

    /// Create a new document with a specific version.
    pub fn with_version(
        path: impl Into<PathBuf>,
        version: i32,
        content: impl Into<String>,
    ) -> Self {
        let content: Arc<str> = content.into().into();
        let line_starts = compute_line_starts(&content);
        Self {
            path: path.into(),
            version,
            content,
            line_starts: line_starts.into(),
        }
    }

    /// Get the document path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the document version.
    pub fn version(&self) -> i32 {
        self.version
    }

    /// Get the source content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Get the number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Convert a (line, column) position to a byte offset.
    ///
    /// Lines and columns are 0-indexed. Returns `None` if the position is out of bounds.
    pub fn offset_at(&self, line: usize, column: usize) -> Option<usize> {
        let line_start = *self.line_starts.get(line)?;
        let line_end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.content.len());

        // Get the line content and find the byte offset for the column
        let line_content = &self.content[line_start..line_end];
        let offset = utf16_offset_to_byte(line_content, column)?;

        Some(line_start + offset)
    }

    /// Convert a byte offset to a (line, column) position.
    ///
    /// Returns 0-indexed line and column. The column is in UTF-16 code units
    /// for LSP compatibility.
    pub fn position_at(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.content.len());

        // Binary search for the line containing this offset
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(line) => line.saturating_sub(1),
        };

        let line_start = self.line_starts[line];
        let line_content = &self.content[line_start..offset];
        let column = byte_offset_to_utf16(line_content);

        (line, column)
    }

    /// Get the text for a specific line (0-indexed).
    ///
    /// The returned string does not include the line ending.
    pub fn line(&self, line: usize) -> Option<&str> {
        let start = *self.line_starts.get(line)?;
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.content.len());

        let text = &self.content[start..end];
        // Strip trailing newline characters
        Some(text.trim_end_matches(['\r', '\n']))
    }

    /// Get a slice of the content by byte range.
    pub fn slice(&self, start: usize, end: usize) -> Option<&str> {
        if start <= end && end <= self.content.len() {
            Some(&self.content[start..end])
        } else {
            None
        }
    }

    /// Get the byte offset of the start of a line.
    pub fn line_start(&self, line: usize) -> Option<usize> {
        self.line_starts.get(line).copied()
    }

    /// Get the byte offset of the end of a line (before the newline).
    pub fn line_end(&self, line: usize) -> Option<usize> {
        let start = *self.line_starts.get(line)?;
        let end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.content.len());

        // Find position before newline
        let line_content = &self.content[start..end];
        let trimmed_len = line_content.trim_end_matches(['\r', '\n']).len();
        Some(start + trimmed_len)
    }

    /// Create a new document with updated content, incrementing the version.
    pub fn update(&self, content: impl Into<String>) -> Self {
        Self::with_version(&self.path, self.version + 1, content)
    }

    /// Apply an incremental edit to the document.
    ///
    /// The range is specified as (start_line, start_col, end_line, end_col) with
    /// 0-indexed positions and UTF-16 columns.
    pub fn apply_edit(
        &self,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
        new_text: &str,
    ) -> Option<Self> {
        let start_offset = self.offset_at(start_line, start_col)?;
        let end_offset = self.offset_at(end_line, end_col)?;

        if start_offset > end_offset {
            return None;
        }

        let mut new_content = String::with_capacity(
            self.content.len() - (end_offset - start_offset) + new_text.len(),
        );
        new_content.push_str(&self.content[..start_offset]);
        new_content.push_str(new_text);
        new_content.push_str(&self.content[end_offset..]);

        Some(Self::with_version(
            &self.path,
            self.version + 1,
            new_content,
        ))
    }
}

/// Compute the byte offset of the start of each line.
fn compute_line_starts(content: &str) -> Vec<usize> {
    let mut line_starts = vec![0];
    for (i, c) in content.char_indices() {
        if c == '\n' {
            line_starts.push(i + 1);
        }
    }
    line_starts
}

/// Convert a UTF-16 column offset to a byte offset within a line.
fn utf16_offset_to_byte(line: &str, utf16_col: usize) -> Option<usize> {
    let mut utf16_count = 0;
    for (byte_offset, c) in line.char_indices() {
        if utf16_count >= utf16_col {
            return Some(byte_offset);
        }
        utf16_count += c.len_utf16();
    }
    // Allow positioning at end of line
    if utf16_count == utf16_col {
        Some(line.len())
    } else {
        None
    }
}

/// Convert a byte offset within a line to a UTF-16 column offset.
fn byte_offset_to_utf16(line_prefix: &str) -> usize {
    line_prefix.chars().map(|c| c.len_utf16()).sum()
}

/// Position in a document (0-indexed line and UTF-16 column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A range in a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    pub fn from_positions(
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start: Position::new(start_line, start_col),
            end: Position::new(end_line, end_col),
        }
    }

    /// Check if a position is within this range.
    pub fn contains(&self, pos: Position) -> bool {
        if pos.line < self.start.line || pos.line > self.end.line {
            return false;
        }
        if pos.line == self.start.line && pos.column < self.start.column {
            return false;
        }
        if pos.line == self.end.line && pos.column > self.end.column {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document() {
        let doc = Document::new("test.gd", "hello\nworld");
        assert_eq!(doc.version(), 0);
        assert_eq!(doc.content(), "hello\nworld");
        assert_eq!(doc.line_count(), 2);
    }

    #[test]
    fn line_access() {
        let doc = Document::new("test.gd", "line1\nline2\nline3");
        assert_eq!(doc.line(0), Some("line1"));
        assert_eq!(doc.line(1), Some("line2"));
        assert_eq!(doc.line(2), Some("line3"));
        assert_eq!(doc.line(3), None);
    }

    #[test]
    fn line_with_crlf() {
        let doc = Document::new("test.gd", "line1\r\nline2\r\n");
        assert_eq!(doc.line(0), Some("line1"));
        assert_eq!(doc.line(1), Some("line2"));
    }

    #[test]
    fn offset_at_simple() {
        let doc = Document::new("test.gd", "hello\nworld");
        assert_eq!(doc.offset_at(0, 0), Some(0));
        assert_eq!(doc.offset_at(0, 5), Some(5));
        assert_eq!(doc.offset_at(1, 0), Some(6));
        assert_eq!(doc.offset_at(1, 5), Some(11));
    }

    #[test]
    fn position_at_simple() {
        let doc = Document::new("test.gd", "hello\nworld");
        assert_eq!(doc.position_at(0), (0, 0));
        assert_eq!(doc.position_at(5), (0, 5));
        assert_eq!(doc.position_at(6), (1, 0));
        assert_eq!(doc.position_at(11), (1, 5));
    }

    #[test]
    fn utf16_handling() {
        // "𝄞" is a musical symbol that takes 2 UTF-16 code units
        let doc = Document::new("test.gd", "a𝄞b");
        // 'a' is at column 0, '𝄞' starts at column 1 and takes 2 UTF-16 units
        // 'b' is at column 3 (1 + 2)
        assert_eq!(doc.offset_at(0, 0), Some(0)); // 'a'
        assert_eq!(doc.offset_at(0, 1), Some(1)); // '𝄞' start
        assert_eq!(doc.offset_at(0, 3), Some(5)); // 'b' (after 4-byte char)

        // Reverse mapping
        assert_eq!(doc.position_at(0), (0, 0)); // 'a'
        assert_eq!(doc.position_at(1), (0, 1)); // '𝄞' start
        assert_eq!(doc.position_at(5), (0, 3)); // 'b'
    }

    #[test]
    fn update_increments_version() {
        let doc = Document::new("test.gd", "v1");
        let doc2 = doc.update("v2");
        assert_eq!(doc.version(), 0);
        assert_eq!(doc2.version(), 1);
        assert_eq!(doc2.content(), "v2");
    }

    #[test]
    fn apply_edit_insert() {
        let doc = Document::new("test.gd", "hello world");
        let doc2 = doc.apply_edit(0, 5, 0, 5, " beautiful").unwrap();
        assert_eq!(doc2.content(), "hello beautiful world");
    }

    #[test]
    fn apply_edit_replace() {
        let doc = Document::new("test.gd", "hello world");
        let doc2 = doc.apply_edit(0, 6, 0, 11, "rust").unwrap();
        assert_eq!(doc2.content(), "hello rust");
    }

    #[test]
    fn apply_edit_multiline() {
        let doc = Document::new("test.gd", "line1\nline2\nline3");
        let doc2 = doc.apply_edit(0, 5, 2, 0, "\nnew\n").unwrap();
        assert_eq!(doc2.content(), "line1\nnew\nline3");
    }

    #[test]
    fn range_contains() {
        let range = Range::from_positions(1, 5, 3, 10);
        assert!(!range.contains(Position::new(0, 5))); // before start line
        assert!(!range.contains(Position::new(1, 4))); // before start column
        assert!(range.contains(Position::new(1, 5))); // at start
        assert!(range.contains(Position::new(2, 0))); // middle line
        assert!(range.contains(Position::new(3, 10))); // at end
        assert!(!range.contains(Position::new(3, 11))); // after end column
        assert!(!range.contains(Position::new(4, 0))); // after end line
    }

    #[test]
    fn line_start_end() {
        let doc = Document::new("test.gd", "hello\nworld\n");
        assert_eq!(doc.line_start(0), Some(0));
        assert_eq!(doc.line_end(0), Some(5));
        assert_eq!(doc.line_start(1), Some(6));
        assert_eq!(doc.line_end(1), Some(11));
    }
}
