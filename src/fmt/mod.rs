pub mod comments;
pub mod format;
pub mod ir;
pub mod printer;

use std::path::Path;

use crate::config::{IndentStyle, QuoteStyle, TrailingComma};
use crate::parser;

/// Configuration for the formatter.
#[derive(Debug, Clone)]
pub struct FmtConfig {
    /// Maximum line width before breaking (default: 100).
    pub print_width: usize,
    /// Indentation style: tabs or spaces (default: tabs).
    pub indent_style: IndentStyle,
    /// Number of spaces per indentation level when using spaces (default: 4).
    pub indent_size: usize,
    /// Quote style for strings (default: preserve).
    pub quote_style: QuoteStyle,
    /// Trailing comma behavior (default: multiline).
    pub trailing_comma: TrailingComma,
    /// Maximum consecutive blank lines allowed (default: 2).
    pub max_blank_lines: usize,
}

impl Default for FmtConfig {
    fn default() -> Self {
        Self {
            print_width: 100,
            indent_style: IndentStyle::Tabs,
            indent_size: 4,
            quote_style: QuoteStyle::Preserve,
            trailing_comma: TrailingComma::Multiline,
            max_blank_lines: 2,
        }
    }
}

impl From<crate::config::FormatterConfig> for FmtConfig {
    fn from(config: crate::config::FormatterConfig) -> Self {
        Self {
            print_width: config.print_width,
            indent_style: config.indent_style,
            indent_size: config.indent_size,
            quote_style: config.quote_style,
            trailing_comma: config.trailing_comma,
            max_blank_lines: config.max_blank_lines,
        }
    }
}

impl From<&crate::config::FormatterConfig> for FmtConfig {
    fn from(config: &crate::config::FormatterConfig) -> Self {
        Self {
            print_width: config.print_width,
            indent_style: config.indent_style,
            indent_size: config.indent_size,
            quote_style: config.quote_style,
            trailing_comma: config.trailing_comma,
            max_blank_lines: config.max_blank_lines,
        }
    }
}

/// Result of a formatting operation.
pub struct FmtResult {
    /// The formatted output string.
    pub output: String,
    /// Whether the output is identical to the input (no changes needed).
    pub unchanged: bool,
}

/// Format GDScript source code.
///
/// Returns the formatted source, or an error if the source has parse errors
/// (in which case the original source is returned unchanged).
pub fn format_source(source: &str, config: &FmtConfig) -> Result<FmtResult, String> {
    let parsed = parser::parse_source(source)?;
    let root = parsed.root_node();

    // Bail if the tree contains parse errors — don't format broken files.
    if has_errors(root) {
        return Ok(FmtResult {
            output: source.to_string(),
            unchanged: true,
        });
    }

    let mut comment_store = comments::CommentStore::extract(root, source);
    let doc = format::format_node(root, source, &mut comment_store, config);
    let raw = printer::print(&doc, config);

    // Strip trailing whitespace from each line.
    let output: String = raw
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    // Preserve trailing newline if present.
    let output = if raw.ends_with('\n') && !output.ends_with('\n') {
        format!("{}\n", output)
    } else {
        output
    };

    let unchanged = output == source;
    Ok(FmtResult { output, unchanged })
}

/// Format a GDScript file on disk.
///
/// Reads the file, formats it, and returns the result. Does not write
/// the file back — the caller decides whether to overwrite.
pub fn format_file(path: &Path, config: &FmtConfig) -> Result<FmtResult, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    format_source(&source, config)
}

/// Generate a unified diff between the original and formatted source.
pub fn make_diff(path: &str, original: &str, formatted: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    if original == formatted {
        return String::new();
    }

    let diff = TextDiff::from_lines(original, formatted);
    let mut output = String::new();
    let mut has_changes = false;

    let mut hunks = String::new();
    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        has_changes = true;
        hunks.push_str(&format!("{}", hunk.header()));
        for change in hunk.iter_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            hunks.push_str(sign);
            hunks.push_str(change.value());
            if !change.value().ends_with('\n') {
                hunks.push('\n');
            }
        }
    }

    if has_changes {
        output.push_str(&format!("--- {}\n", path));
        output.push_str(&format!("+++ {}\n", path));
        output.push_str(&hunks);
    }

    output
}

/// Check if any node in the tree has an ERROR kind (parse error).
fn has_errors(node: tree_sitter::Node) -> bool {
    if node.kind() == "ERROR" || node.is_error() {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if has_errors(child) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{IndentStyle, QuoteStyle, TrailingComma};

    #[test]
    fn format_simple_var() {
        let src = "var x = 1\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, "var x = 1\n");
        assert!(result.unchanged);
    }

    #[test]
    fn format_adds_trailing_newline() {
        let src = "var x = 1";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert!(result.output.ends_with('\n'));
    }

    #[test]
    fn format_preserves_parse_errors() {
        let src = "var = = =\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        // Should return unchanged because of parse errors.
        assert_eq!(result.output, src);
        assert!(result.unchanged);
    }

    #[test]
    fn format_function_def() {
        let src = "func foo():\n\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, "func foo():\n\tpass\n");
    }

    #[test]
    fn format_function_with_return_type() {
        let src = "func foo() -> int:\n\treturn 1\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, "func foo() -> int:\n\treturn 1\n");
    }

    #[test]
    fn format_if_elif_else() {
        let src =
            "func foo():\n\tif x > 0:\n\t\tpass\n\telif x == 0:\n\t\tpass\n\telse:\n\t\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_for_loop() {
        let src = "func foo():\n\tfor i in range(10):\n\t\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_extends() {
        let src = "extends Node2D\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_blank_lines_between_functions() {
        let src = "extends Node\n\n\nfunc foo():\n\tpass\n\n\nfunc bar():\n\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_var_with_type() {
        let src = "var speed: float = 50.0\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_const_with_type() {
        let src = "const MAX: int = 100\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_inferred_type() {
        let src = "var x := 10\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_array_literal() {
        let src = "func foo():\n\tvar arr = [1, 2, 3]\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_dictionary_literal() {
        let src = "func foo():\n\tvar d = {\"a\": 1, \"b\": 2}\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_annotation() {
        let src = "@export var speed: float = 10.0\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_match_stmt() {
        let src = "func foo():\n\tmatch x:\n\t\t1:\n\t\t\tpass\n\t\t_:\n\t\t\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_signal() {
        let src = "signal health_changed(amount: int)\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_enum() {
        let src = "enum Dir { UP, DOWN, LEFT, RIGHT }\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_idempotent() {
        let src = "extends Node\n\n\nfunc foo(x: int) -> int:\n\tvar y = x + 1\n\treturn y\n";
        let result1 = format_source(src, &FmtConfig::default()).unwrap();
        let result2 = format_source(&result1.output, &FmtConfig::default()).unwrap();
        assert_eq!(
            result1.output, result2.output,
            "Formatting is not idempotent"
        );
    }

    #[test]
    fn make_diff_no_changes() {
        let src = "var x = 1\n";
        let diff = make_diff("test.gd", src, src);
        assert!(diff.is_empty() || !diff.contains('+'));
    }

    #[test]
    fn make_diff_with_changes() {
        let original = "var x=1\n";
        let formatted = "var x = 1\n";
        let diff = make_diff("test.gd", original, formatted);
        assert!(diff.contains("---"));
        assert!(diff.contains("+++"));
    }

    #[test]
    fn format_function_trailing_comment() {
        // Input with irregular spacing, should normalize to single space
        let src = "func foo():    # this is a comment\n\tpass\n";
        let expected = "func foo(): # this is a comment\n\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, expected);
    }

    #[test]
    fn format_function_trailing_comment_preserved() {
        // Already normalized, should be unchanged
        let src = "func foo(): # this is a comment\n\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
        assert!(result.unchanged);
    }

    #[test]
    fn format_function_return_type_trailing_comment() {
        let src = "func foo() -> int: # returns an int\n\treturn 1\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
        assert!(result.unchanged);
    }

    #[test]
    fn format_match_trailing_comment() {
        let src = "func foo():\n\tmatch x: # match on x\n\t\t1:\n\t\t\tpass\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert_eq!(result.output, src);
        assert!(result.unchanged);
    }

    // ==================== New configuration option tests ====================

    #[test]
    fn format_indent_with_spaces() {
        let src = "func foo():\n\tpass\n";
        let config = FmtConfig {
            indent_style: IndentStyle::Spaces,
            indent_size: 4,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        assert_eq!(result.output, "func foo():\n    pass\n");
    }

    #[test]
    fn format_indent_with_2_spaces() {
        let src = "func foo():\n\tif true:\n\t\tpass\n";
        let config = FmtConfig {
            indent_style: IndentStyle::Spaces,
            indent_size: 2,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        assert_eq!(result.output, "func foo():\n  if true:\n    pass\n");
    }

    #[test]
    fn format_quote_style_double() {
        let src = "var x = 'hello'\n";
        let config = FmtConfig {
            quote_style: QuoteStyle::Double,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        assert_eq!(result.output, "var x = \"hello\"\n");
    }

    #[test]
    fn format_quote_style_single() {
        let src = "var x = \"hello\"\n";
        let config = FmtConfig {
            quote_style: QuoteStyle::Single,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        assert_eq!(result.output, "var x = 'hello'\n");
    }

    #[test]
    fn format_quote_style_preserve() {
        let src1 = "var x = 'hello'\n";
        let src2 = "var x = \"hello\"\n";
        let config = FmtConfig {
            quote_style: QuoteStyle::Preserve,
            ..Default::default()
        };
        let result1 = format_source(src1, &config).unwrap();
        let result2 = format_source(src2, &config).unwrap();
        assert_eq!(result1.output, src1);
        assert_eq!(result2.output, src2);
    }

    #[test]
    fn format_quote_style_smart_no_escape() {
        // Don't convert if it would require escapes
        let src = "var x = \"it's a test\"\n";
        let config = FmtConfig {
            quote_style: QuoteStyle::Single,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // Should preserve because 's would need escaping
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_quote_style_preserves_raw_strings() {
        let src = "var x = r\"raw string\"\n";
        let config = FmtConfig {
            quote_style: QuoteStyle::Single,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // Should preserve raw strings
        assert_eq!(result.output, src);
    }

    #[test]
    fn format_trailing_comma_all() {
        // TrailingComma::All adds comma even on single line
        let src = "var arr = [1, 2, 3]\n";
        let config = FmtConfig {
            trailing_comma: TrailingComma::All,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // Should have trailing comma even on single line
        assert_eq!(result.output, "var arr = [1, 2, 3,]\n");
    }

    #[test]
    fn format_trailing_comma_none() {
        let src = "var arr = [\n\t1,\n\t2,\n\t3,\n]\n";
        let config = FmtConfig {
            trailing_comma: TrailingComma::None,
            print_width: 20,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // Should not have trailing comma after last element
        assert!(
            !result.output.contains("3,"),
            "Should not have trailing comma: {}",
            result.output
        );
    }

    #[test]
    fn format_max_blank_lines() {
        let src = "var x = 1\n\n\n\n\nvar y = 2\n";
        let config = FmtConfig {
            max_blank_lines: 1,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // Should reduce to max 1 blank line
        assert_eq!(result.output, "var x = 1\n\nvar y = 2\n");
    }

    #[test]
    fn format_max_blank_lines_between_functions() {
        let src = "func foo():\n\tpass\n\n\n\nfunc bar():\n\tpass\n";
        let config = FmtConfig {
            max_blank_lines: 1,
            ..Default::default()
        };
        let result = format_source(src, &config).unwrap();
        // With max_blank_lines=1, should have 1 blank line between functions
        // (that's 2 newlines: one to end first function, one blank line)
        assert_eq!(
            result.output,
            "func foo():\n\tpass\n\nfunc bar():\n\tpass\n"
        );
    }

    #[test]
    fn format_subscript_no_double_brackets() {
        let src = "func foo():\n\tvar x = dict[key]\n\tvar y = arr[0]\n\tvar z = data[a + b]\n\tdict[key] = 42\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert!(
            !result.output.contains("[["),
            "subscript should not produce double brackets: {}",
            result.output
        );
        assert!(
            !result.output.contains("]]"),
            "subscript should not produce double brackets: {}",
            result.output
        );
        assert!(result.output.contains("dict[key]"));
        assert!(result.output.contains("arr[0]"));
        assert!(result.output.contains("data[a + b]"));
    }

    #[test]
    fn format_inner_class_preserves_body() {
        let src = "class Inner extends RefCounted:\n\tvar x: int\n\n\tfunc _init(p_x: int) -> void:\n\t\tx = p_x\n";
        let result = format_source(src, &FmtConfig::default()).unwrap();
        assert!(
            result.output.contains("var x: int"),
            "inner class body should be preserved: {}",
            result.output
        );
        assert!(
            result.output.contains("func _init"),
            "inner class methods should be preserved: {}",
            result.output
        );
        assert!(
            result.output.contains("x = p_x"),
            "inner class method bodies should be preserved: {}",
            result.output
        );
    }
}
