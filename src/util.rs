/// Pre-computed line offset table for O(1) line-to-offset lookups.
///
/// Build once per source file, then use for all diagnostics in that file.
#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Byte offset of the start of each line (0-indexed).
    /// line_offsets[0] = 0 (start of line 1)
    /// line_offsets[1] = offset of line 2, etc.
    line_offsets: Vec<usize>,
    source_len: usize,
}

impl LineIndex {
    /// Build a line index from source text.
    pub fn new(source: &str) -> Self {
        let mut line_offsets = vec![0];
        for (i, ch) in source.char_indices() {
            if ch == '\n' {
                line_offsets.push(i + 1);
            }
        }
        LineIndex {
            line_offsets,
            source_len: source.len(),
        }
    }

    /// Convert 1-based line and 0-based column to byte offset.
    /// Returns source length if line is past end of file.
    pub fn line_col_to_offset(&self, line: usize, col: usize) -> usize {
        let line_idx = line.saturating_sub(1);
        if line_idx >= self.line_offsets.len() {
            return self.source_len;
        }
        (self.line_offsets[line_idx] + col).min(self.source_len)
    }
}

/// Convert 1-based line and 0-based column to byte offset in source text.
///
/// Note: For multiple lookups in the same file, use `LineIndex` instead
/// for O(1) lookups rather than O(n) per call.
#[allow(dead_code)] // Used in tests to validate LineIndex
pub fn line_col_to_byte_offset(source: &str, line: usize, col: usize) -> usize {
    let target_line = line.saturating_sub(1);
    let mut current_line = 0;
    let mut line_start = 0;

    for (i, ch) in source.char_indices() {
        if current_line == target_line {
            line_start = i;
            break;
        }
        if ch == '\n' {
            current_line += 1;
            // If next line is our target and there are more chars, record start
            if current_line == target_line {
                line_start = i + 1;
                break;
            }
        }
    }

    // If we never found the line (past end of file), return end
    if current_line < target_line {
        return source.len();
    }

    (line_start + col).min(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_line() {
        let source = "hello world\nsecond line\n";
        assert_eq!(line_col_to_byte_offset(source, 1, 0), 0);
        assert_eq!(line_col_to_byte_offset(source, 1, 5), 5);
    }

    #[test]
    fn second_line() {
        let source = "hello\nworld\n";
        assert_eq!(line_col_to_byte_offset(source, 2, 0), 6);
        assert_eq!(line_col_to_byte_offset(source, 2, 3), 9);
    }

    #[test]
    fn past_end() {
        let source = "hi\n";
        assert_eq!(line_col_to_byte_offset(source, 10, 0), source.len());
    }

    #[test]
    fn line_index_first_line() {
        let source = "hello world\nsecond line\n";
        let idx = LineIndex::new(source);
        assert_eq!(idx.line_col_to_offset(1, 0), 0);
        assert_eq!(idx.line_col_to_offset(1, 5), 5);
    }

    #[test]
    fn line_index_second_line() {
        let source = "hello\nworld\n";
        let idx = LineIndex::new(source);
        assert_eq!(idx.line_col_to_offset(2, 0), 6);
        assert_eq!(idx.line_col_to_offset(2, 3), 9);
    }

    #[test]
    fn line_index_past_end() {
        let source = "hi\n";
        let idx = LineIndex::new(source);
        assert_eq!(idx.line_col_to_offset(10, 0), source.len());
    }

    #[test]
    fn line_index_matches_naive() {
        let source = "line one\nline two\nline three\nfour\n";
        let idx = LineIndex::new(source);
        for line in 1..=5 {
            for col in 0..10 {
                assert_eq!(
                    idx.line_col_to_offset(line, col),
                    line_col_to_byte_offset(source, line, col),
                    "Mismatch at line {}, col {}",
                    line,
                    col
                );
            }
        }
    }
}
