use crate::rules::{Diagnostic, TextEdit};
use crate::util::LineIndex;

/// Result of applying fixes to source text.
pub struct FixResult {
    /// The modified source text.
    pub source: String,
    /// Number of fixes applied.
    pub num_fixed: usize,
    /// Byte ranges that were modified (in original source coordinates).
    pub applied_ranges: Vec<(usize, usize)>,
}

/// Apply all available fixes from diagnostics to the source text.
/// Overlapping edits are skipped to avoid conflicts.
/// If `include_unsafe` is false, fixes marked as unsafe are skipped.
pub fn apply_fixes(source: &str, diagnostics: &[Diagnostic], include_unsafe: bool) -> FixResult {
    // Collect all edits from diagnostics that have fixes
    let mut edits: Vec<&TextEdit> = diagnostics
        .iter()
        .filter_map(|d| d.fix.as_ref())
        .filter(|f| include_unsafe || !f.is_unsafe)
        .flat_map(|f| &f.edits)
        .collect();

    if edits.is_empty() {
        return FixResult {
            source: source.to_string(),
            num_fixed: 0,
            applied_ranges: Vec::new(),
        };
    }

    // Sort by start_byte descending so we apply from end to start,
    // preserving earlier byte offsets.
    edits.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

    let mut result = source.to_string();
    let mut applied = 0;
    let mut applied_ranges = Vec::new();
    // Track the lowest byte we've touched to detect overlaps
    let mut min_touched = usize::MAX;

    for edit in &edits {
        // Skip if this edit overlaps with one we already applied
        if edit.end_byte > min_touched {
            continue;
        }
        // Bounds check
        if edit.start_byte > result.len() || edit.end_byte > result.len() {
            continue;
        }

        result.replace_range(edit.start_byte..edit.end_byte, &edit.replacement);
        applied_ranges.push((edit.start_byte, edit.end_byte));
        min_touched = edit.start_byte;
        applied += 1;
    }

    FixResult {
        source: result,
        num_fixed: applied,
        applied_ranges,
    }
}

/// Counts of fixable diagnostics.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixCounts {
    /// Number of diagnostics with safe (auto-fixable) fixes.
    pub safe: usize,
    /// Number of diagnostics with unsafe fixes (require --unsafe).
    pub unsafe_: usize,
}

impl FixCounts {
    /// Total fixable diagnostics.
    pub fn total(&self) -> usize {
        self.safe + self.unsafe_
    }

    /// Check if any fixes are available.
    pub fn has_any(&self) -> bool {
        self.total() > 0
    }
}

/// Count how many diagnostics have safe vs unsafe fixes.
pub fn count_fixable(diagnostics: &[Diagnostic]) -> FixCounts {
    let mut counts = FixCounts::default();
    for d in diagnostics {
        if let Some(ref fix) = d.fix {
            if fix.is_unsafe {
                counts.unsafe_ += 1;
            } else {
                counts.safe += 1;
            }
        }
    }
    counts
}

/// Check if a diagnostic's span overlaps with any applied fix range.
pub fn overlaps_applied_fix(
    diag: &Diagnostic,
    line_index: &LineIndex,
    applied_ranges: &[(usize, usize)],
) -> bool {
    let diag_start = line_index.line_col_to_offset(diag.line, diag.col);
    // For diagnostics with no span, use just the start position
    let diag_end = if diag.end_line > 0 {
        line_index
            .line_col_to_offset(diag.end_line, diag.end_col)
            .max(diag_start + 1)
    } else {
        diag_start + 1
    };

    applied_ranges
        .iter()
        .any(|(start, end)| diag_start < *end && diag_end > *start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Diagnostic, Fix, Severity, TextEdit};

    fn make_diag_with_fix(line: usize, start: usize, end: usize, replacement: &str) -> Diagnostic {
        Diagnostic {
            rule: "test/rule",
            severity: Severity::Warning,
            message: "test".to_string(),
            line,
            col: 0,
            end_line: line,
            end_col: 0,
            fix: Some(Fix::new(
                "fix",
                vec![TextEdit {
                    start_byte: start,
                    end_byte: end,
                    replacement: replacement.to_string(),
                }],
            )),
            labels: vec![],
            note: None,
        }
    }

    fn make_diag_no_fix(line: usize, col: usize, end_line: usize, end_col: usize) -> Diagnostic {
        Diagnostic {
            rule: "test/rule",
            severity: Severity::Warning,
            message: "test".to_string(),
            line,
            col,
            end_line,
            end_col,
            fix: None,
            labels: vec![],
            note: None,
        }
    }

    #[test]
    fn apply_no_fixes() {
        let source = "var x = 1\nvar y = 2\n";
        let result = apply_fixes(source, &[], true);
        assert_eq!(result.source, source);
        assert_eq!(result.num_fixed, 0);
        assert!(result.applied_ranges.is_empty());
    }

    #[test]
    fn apply_single_fix() {
        let source = "var x = 1\n";
        let diag = make_diag_with_fix(1, 4, 5, "y");
        let result = apply_fixes(source, &[diag], true);
        assert_eq!(result.source, "var y = 1\n");
        assert_eq!(result.num_fixed, 1);
    }

    #[test]
    fn apply_multiple_non_overlapping() {
        let source = "var x = 1\nvar y = 2\n";
        let d1 = make_diag_with_fix(1, 4, 5, "a"); // x -> a
        let d2 = make_diag_with_fix(2, 14, 15, "b"); // y -> b
        let result = apply_fixes(source, &[d1, d2], true);
        assert_eq!(result.source, "var a = 1\nvar b = 2\n");
        assert_eq!(result.num_fixed, 2);
    }

    #[test]
    fn apply_overlapping_fixes_skips_conflict() {
        let source = "var x = 1\n";
        let d1 = make_diag_with_fix(1, 0, 9, "var y = 2"); // replace entire line
        let d2 = make_diag_with_fix(1, 4, 5, "z"); // replace x with z (overlaps)
        let result = apply_fixes(source, &[d1, d2], true);
        // Only one fix should be applied (the later one in the file, since we sort descending)
        assert_eq!(result.num_fixed, 1);
    }

    #[test]
    fn apply_fix_out_of_bounds() {
        let source = "hi";
        let diag = make_diag_with_fix(1, 100, 200, "nope");
        let result = apply_fixes(source, &[diag], true);
        assert_eq!(result.source, "hi");
        assert_eq!(result.num_fixed, 0);
    }

    #[test]
    fn overlaps_applied_fix_overlapping() {
        let source = "var x = 1\nvar y = 2\n";
        let line_index = LineIndex::new(source);
        let ranges = vec![(4, 9)]; // bytes 4-9 were modified
        let diag = make_diag_no_fix(1, 5, 1, 8); // overlaps with 4..9
        assert!(overlaps_applied_fix(&diag, &line_index, &ranges));
    }

    #[test]
    fn overlaps_applied_fix_non_overlapping() {
        let source = "var x = 1\nvar y = 2\n";
        let line_index = LineIndex::new(source);
        let ranges = vec![(4, 9)];
        let diag = make_diag_no_fix(2, 0, 2, 5); // line 2, different range
        assert!(!overlaps_applied_fix(&diag, &line_index, &ranges));
    }

    #[test]
    fn overlaps_applied_fix_no_end_line() {
        let source = "var x = 1\nvar y = 2\n";
        let line_index = LineIndex::new(source);
        let ranges = vec![(0, 3)];
        let diag = make_diag_no_fix(1, 1, 0, 0); // end_line = 0
        assert!(overlaps_applied_fix(&diag, &line_index, &ranges));
    }

    fn make_diag_with_unsafe_fix(
        line: usize,
        start: usize,
        end: usize,
        replacement: &str,
    ) -> Diagnostic {
        Diagnostic {
            rule: "test/rule",
            severity: Severity::Warning,
            message: "test".to_string(),
            line,
            col: 0,
            end_line: line,
            end_col: 0,
            fix: Some(Fix::new_unsafe(
                "fix",
                vec![TextEdit {
                    start_byte: start,
                    end_byte: end,
                    replacement: replacement.to_string(),
                }],
            )),
            labels: vec![],
            note: None,
        }
    }

    #[test]
    fn apply_unsafe_fix_when_include_unsafe_true() {
        let source = "var x = 1\n";
        let diag = make_diag_with_unsafe_fix(1, 0, 10, "var y = 2\n");
        let result = apply_fixes(source, &[diag], true);
        assert_eq!(result.source, "var y = 2\n");
        assert_eq!(result.num_fixed, 1);
    }

    #[test]
    fn skip_unsafe_fix_when_include_unsafe_false() {
        let source = "var x = 1\n";
        let diag = make_diag_with_unsafe_fix(1, 0, 10, "var y = 2\n");
        let result = apply_fixes(source, &[diag], false);
        assert_eq!(result.source, source);
        assert_eq!(result.num_fixed, 0);
    }

    #[test]
    fn apply_only_safe_fixes_when_include_unsafe_false() {
        let source = "var x = 1\nvar y = 2\n";
        let safe_fix = make_diag_with_fix(1, 4, 5, "a"); // x -> a (safe)
        let unsafe_fix = make_diag_with_unsafe_fix(2, 14, 15, "b"); // y -> b (unsafe)
        let result = apply_fixes(source, &[safe_fix, unsafe_fix], false);
        assert_eq!(result.source, "var a = 1\nvar y = 2\n");
        assert_eq!(result.num_fixed, 1);
    }

    #[test]
    fn count_fixable_safe_and_unsafe() {
        let d1 = make_diag_with_fix(1, 0, 1, "a");
        let d2 = make_diag_with_unsafe_fix(2, 0, 1, "b");
        let d3 = make_diag_no_fix(3, 0, 3, 1);
        let counts = count_fixable(&[d1, d2, d3]);
        assert_eq!(counts.safe, 1);
        assert_eq!(counts.unsafe_, 1);
        assert_eq!(counts.total(), 2);
    }
}
