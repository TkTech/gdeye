use tree_sitter::Node;

use crate::config::{QuoteStyle, TrailingComma};
use crate::fmt::comments::CommentStore;
use crate::fmt::ir::*;
use crate::fmt::FmtConfig;

/// Convert a tree-sitter CST node into a Doc IR for pretty-printing.
pub fn format_node(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    match node.kind() {
        "source" => format_source_node(node, src, comments, config),
        "function_definition" | "constructor_definition" => {
            format_function_def(node, src, comments, config)
        }
        "class_definition" => format_class_def(node, src, comments, config),
        "variable_statement" => format_var_stmt(node, src, comments, config),
        "const_statement" => format_const_stmt(node, src, comments, config),
        "if_statement" => format_if_stmt(node, src, comments, config),
        "elif_clause" => format_elif_clause(node, src, comments, config),
        "else_clause" => format_else_clause(node, src, comments, config),
        "for_statement" => format_for_stmt(node, src, comments, config),
        "while_statement" => format_while_stmt(node, src, comments, config),
        "match_statement" => format_match_stmt(node, src, comments, config),
        "return_statement" => format_return_stmt(node, src, comments, config),
        "break_statement" => text("break"),
        "continue_statement" => text("continue"),
        "pass_statement" => text("pass"),
        "expression_statement" => format_expression_stmt(node, src, comments, config),
        "signal_statement" => format_signal_stmt(node, src, comments, config),
        "enum_definition" => format_enum_def(node, src, comments, config),
        "extends_statement" => format_extends_stmt(node, src, comments),
        "class_name_statement" => format_class_name_stmt(node, src, comments),
        "annotation" => format_annotation(node, src, comments, config),
        "call" => format_call(node, src, comments, config),
        "attribute" => format_attribute(node, src, comments, config),
        "binary_operator" => format_binary_op(node, src, comments, config),
        "unary_operator" => format_unary_op(node, src, comments, config),
        "assignment" => format_assignment(node, src, comments, config),
        "augmented_assignment" => format_augmented_assignment(node, src, comments, config),
        "array" => format_array(node, src, comments, config),
        "dictionary" => format_dictionary(node, src, comments, config),
        "conditional_expression" => format_conditional_expr(node, src, comments, config),
        "lambda" => format_lambda(node, src, comments, config),
        "parenthesized_expression" => format_parenthesized(node, src, comments, config),
        "subscript" => format_subscript(node, src, comments, config),
        "get_node" => text(node_text(node, src)),
        "string" => format_string(node, src, config),
        "integer" => text(node_text(node, src)),
        "float" => text(node_text(node, src)),
        "true" | "false" => text(node_text(node, src)),
        "null" => text("null"),
        "self" => text("self"),
        "identifier" => text(node_text(node, src)),
        "name" => text(node_text(node, src)),
        "type" => text(node_text(node, src)),
        "comment" => text(node_text(node, src)),
        _ => text(node_text(node, src)),
    }
}

/// Format a string literal, potentially normalizing quotes based on config.
fn format_string(node: Node, src: &str, config: &FmtConfig) -> Doc {
    let original = node_text(node, src);

    // Preserve raw strings, multi-line strings, and string names
    if original.starts_with("r\"")
        || original.starts_with("r'")
        || original.starts_with("\"\"\"")
        || original.starts_with("'''")
        || original.starts_with("&\"")
        || original.starts_with("&'")
        || original.starts_with("^\"")
        || original.starts_with("^'")
        || original.starts_with("%\"")
        || original.starts_with("%'")
    {
        return text(original);
    }

    match config.quote_style {
        QuoteStyle::Preserve => text(original),
        QuoteStyle::Double => normalize_quotes(original, '"'),
        QuoteStyle::Single => normalize_quotes(original, '\''),
    }
}

/// Normalize string quotes to the target quote character.
/// Only converts if it won't require adding escapes.
fn normalize_quotes(s: &str, target_quote: char) -> Doc {
    let current_quote = if s.starts_with('"') {
        '"'
    } else if s.starts_with('\'') {
        '\''
    } else {
        return text(s); // Unknown format, preserve
    };

    // Already using target quote
    if current_quote == target_quote {
        return text(s);
    }

    // Extract content without quotes
    let content = &s[1..s.len() - 1];

    // Check if target quote appears unescaped in content
    // If so, we'd need to add escapes, so preserve original
    if contains_unescaped(content, target_quote) {
        return text(s);
    }

    // Safe to convert: replace quotes and unescape/re-escape as needed
    let mut result = String::with_capacity(s.len());
    result.push(target_quote);

    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if next == current_quote {
                    // Unescape the old quote: \' -> ' or \" -> "
                    result.push(next);
                    chars.next();
                } else {
                    // Keep other escapes as-is
                    result.push(c);
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }

    result.push(target_quote);
    text(result)
}

/// Check if a string contains an unescaped instance of the given character.
fn contains_unescaped(s: &str, ch: char) -> bool {
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == ch {
            return true;
        }
    }
    false
}

/// Generate trailing comma doc based on config.
fn trailing_comma(config: &FmtConfig) -> Doc {
    match config.trailing_comma {
        TrailingComma::All => text(","),
        TrailingComma::Multiline => if_break(text(","), text("")),
        TrailingComma::None => text(""),
    }
}

/// Format the top-level source node with blank line rules between declarations.
fn format_source_node(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut prev_kind: Option<&str> = None;
    let mut prev_end_row: Option<usize> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "comment" {
            continue;
        }

        let leading = comments.take_leading(child.start_byte());

        // Compute blank lines from structural rules.
        let structural_blanks = if let Some(prev) = prev_kind {
            blank_lines_between(prev, child.kind(), config)
        } else {
            0
        };

        // Also preserve blank lines from the original source (capped at max_blank_lines).
        let first_row = if let Some(first_comment) = leading.first() {
            first_comment.start_row
        } else {
            child.start_position().row
        };

        let source_blanks = if let Some(prev_row) = prev_end_row {
            let gap = first_row.saturating_sub(prev_row);
            if gap > 1 {
                (gap - 1).min(config.max_blank_lines)
            } else {
                0
            }
        } else {
            0
        };

        let blanks = structural_blanks.max(source_blanks);

        // Emit separator: 1 + blanks newlines to produce `blanks` blank lines.
        if !parts.is_empty() {
            for _ in 0..=blanks {
                parts.push(hardline());
            }
        }

        // Emit leading comments, each followed by a newline.
        for c in &leading {
            parts.push(text(&c.text));
            parts.push(hardline());
        }

        parts.push(format_top_level_stmt(child, src, comments, config));

        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            parts.push(line_suffix(text(format!(" {}", tc.text))));
        }

        prev_end_row = Some(child.end_position().row);
        prev_kind = Some(child.kind());
    }

    // Emit any dangling comments at the end of the file.
    let dangling = comments.take_dangling();
    for c in dangling {
        if !parts.is_empty() {
            parts.push(hardline());
        }
        parts.push(text(&c.text));
    }

    // Trailing newline.
    if !parts.is_empty() {
        parts.push(hardline());
    }

    concat(parts)
}

/// Determine how many blank lines to insert between two top-level declaration kinds.
fn blank_lines_between(prev: &str, next: &str, config: &FmtConfig) -> usize {
    match (prev, next) {
        // Don't insert blank lines between an annotation and its target.
        ("annotation", _) => 0,
        // Two blank lines before/after functions and classes at top level (capped by config).
        (_, "function_definition") | (_, "class_definition") => config.max_blank_lines,
        ("function_definition", _) | ("class_definition", _) => config.max_blank_lines,
        _ => 0,
    }
}

/// Format a top-level statement, handling annotations that precede var/func/class.
fn format_top_level_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    format_node(node, src, comments, config)
}

/// Format a function definition.
fn format_function_def(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();

    let mut has_static = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "static_keyword" {
            has_static = true;
        }
    }

    if has_static {
        parts.push(text("static "));
    }
    parts.push(text("func "));

    // Name
    if node.kind() == "constructor_definition" {
        parts.push(text("_init"));
    } else if let Some(name) = node.child_by_field_name("name") {
        parts.push(text(node_text(name, src)));
    } else {
        // Fallback: find a child with kind "name"
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "name" {
                parts.push(text(node_text(child, src)));
                break;
            }
        }
    }

    // Parameters
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameters" {
            parts.push(format_parameters(child, src, comments, config));
        }
    }

    // Return type - track end position for trailing comment
    let mut cursor = node.walk();
    let mut last_end_byte: usize = 0;
    let mut last_end_row: usize = 0;
    for child in node.children(&mut cursor) {
        if child.kind() == "type" {
            parts.push(text(" -> "));
            parts.push(text(node_text(child, src)));
            last_end_byte = child.end_byte();
            last_end_row = child.end_position().row;
            break;
        } else if child.kind() == "parameters" {
            last_end_byte = child.end_byte();
            last_end_row = child.end_position().row;
        }
    }

    // Body — only emit colon if the function has a body (abstract functions are bodyless).
    let mut has_body = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "body" {
            has_body = true;
            break;
        }
    }

    if has_body {
        parts.push(text(":"));

        // Trailing comment after the colon (e.g., `func foo():  # comment`)
        if let Some(tc) = comments.take_trailing(last_end_byte, last_end_row) {
            parts.push(line_suffix(text(format!(" {}", tc.text))));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "body" {
                parts.push(format_body(child, src, comments, config));
            }
        }
    } else {
        // Trailing comment on bodyless function (e.g., `func foo() -> void  # comment`)
        if let Some(tc) = comments.take_trailing(last_end_byte, last_end_row) {
            parts.push(line_suffix(text(format!(" {}", tc.text))));
        }
    }

    concat(parts)
}

/// Format a class definition.
fn format_class_def(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("class "));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            parts.push(text(node_text(child, src)));
        }
    }

    // Check for extends inside class
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "extends_statement" {
            parts.push(text(" "));
            parts.push(format_extends_stmt(child, src, comments));
        }
    }

    parts.push(text(":"));

    // Body (inner classes use "class_body", top-level classes use "body")
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "body" || child.kind() == "class_body" {
            parts.push(format_class_body(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format the body of a class (1 blank line between members/methods).
fn format_class_body(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut prev_end_row: Option<usize> = None;
    let mut prev_kind: Option<&str> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }

        // Leading comments.
        let leading = comments.take_leading(child.start_byte());

        // Determine blank lines: use the max of the structural rule and
        // what the original source had.
        let structural_blanks = if let Some(prev) = prev_kind {
            class_blank_lines(prev, child.kind())
        } else {
            0
        };

        let first_row = if let Some(first_comment) = leading.first() {
            first_comment.start_row
        } else {
            child.start_position().row
        };

        let source_blanks = if let Some(prev_row) = prev_end_row {
            if first_row > prev_row + 1 {
                1
            } else {
                0
            }
        } else {
            0
        };

        let extra_blanks = structural_blanks.max(source_blanks);
        for _ in 0..extra_blanks {
            parts.push(hardline());
        }

        for c in &leading {
            parts.push(hardline());
            parts.push(text(&c.text));
        }

        parts.push(hardline());
        parts.push(format_node(child, src, comments, config));

        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            parts.push(line_suffix(text(format!(" {}", tc.text))));
        }

        prev_end_row = Some(child.end_position().row);
        prev_kind = Some(child.kind());
    }

    indent(concat(parts))
}

/// Blank lines between class body members.
fn class_blank_lines(prev: &str, next: &str) -> usize {
    match (prev, next) {
        // Don't insert blank lines between an annotation and its target.
        ("annotation", _) => 0,
        (_, "function_definition") | ("function_definition", _) => 1,
        _ => 0,
    }
}

/// Format a body block (indented statements).
fn format_body(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut prev_end_row: Option<usize> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }

        // Leading comments.
        let leading = comments.take_leading(child.start_byte());

        // Preserve one blank line between statements when the original source
        // had a blank line between them.
        let first_row = if let Some(first_comment) = leading.first() {
            first_comment.start_row
        } else {
            child.start_position().row
        };

        if let Some(prev_row) = prev_end_row {
            if first_row > prev_row + 1 {
                // There was at least one blank line in the original — preserve one.
                parts.push(hardline());
            }
        }

        for c in &leading {
            parts.push(hardline());
            parts.push(text(&c.text));
        }

        parts.push(hardline());
        parts.push(format_node(child, src, comments, config));

        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            parts.push(line_suffix(text(format!(" {}", tc.text))));
        }

        prev_end_row = Some(child.end_position().row);
    }

    // Consume orphan comments at the end of the body (comments that were the
    // last child with no next sibling, attached to parent's end_byte).
    let orphan = comments.take_leading(node.end_byte());
    for c in &orphan {
        parts.push(hardline());
        parts.push(text(&c.text));
    }

    if parts.is_empty() {
        // Empty body: emit `pass`
        indent(concat(vec![hardline(), text("pass")]))
    } else {
        indent(concat(parts))
    }
}

/// Format a variable statement: `var name: type = value` or `@annotation var name ...`
fn format_var_stmt(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();

    // Annotations (e.g., @export, @onready)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "annotations" {
            let mut ann_cursor = child.walk();
            for ann in child.children(&mut ann_cursor) {
                if ann.kind() == "annotation" {
                    parts.push(format_annotation(ann, src, comments, config));
                    parts.push(text(" "));
                }
            }
        }
    }

    // Static keyword
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "static_keyword" {
            parts.push(text("static "));
            break;
        }
    }

    parts.push(text("var "));

    // Name
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            parts.push(text(node_text(child, src)));
            break;
        }
    }

    // Type annotation
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type" {
            parts.push(text(": "));
            parts.push(text(node_text(child, src)));
            break;
        }
    }

    // Inferred type (:=)
    let mut cursor = node.walk();
    let mut has_inferred = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "inferred_type" {
            has_inferred = true;
            break;
        }
    }

    // Value
    let value = find_var_value(node, src);
    if let Some(val_node) = value {
        if has_inferred {
            parts.push(text(" := "));
        } else {
            parts.push(text(" = "));
        }
        parts.push(format_node(val_node, src, comments, config));
    }

    // Setter/getter blocks
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "setget" {
            parts.push(text(":"));
            parts.push(format_setget(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format a setget block (setter and/or getter).
fn format_setget(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        match child.kind() {
            "set_body" => {
                parts.push(hardline());
                parts.push(format_set_body(child, src, comments, config));
            }
            "get_body" => {
                parts.push(hardline());
                parts.push(format_get_body(child, src, comments, config));
            }
            _ => {}
        }
    }

    indent(concat(parts))
}

/// Format a setter body: `set(param): statements`
fn format_set_body(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("set"));

    let mut body_node: Option<Node> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "parameters" {
            // Extract the parameter name from the parameters node.
            let mut param_name: Option<&str> = None;
            let mut pc = child.walk();
            for pchild in child.children(&mut pc) {
                if pchild.is_named() && (pchild.kind() == "identifier" || pchild.kind() == "name") {
                    param_name = Some(node_text(pchild, src));
                    break;
                }
            }
            parts.push(text("("));
            if let Some(name) = param_name {
                parts.push(text(name));
            }
            parts.push(text(")"));
        } else if child.kind() == "body" {
            body_node = Some(child);
        }
    }

    parts.push(text(":"));

    if let Some(body) = body_node {
        parts.push(format_body(body, src, comments, config));
    } else {
        parts.push(indent(concat(vec![hardline(), text("pass")])));
    }

    concat(parts)
}

/// Format a getter body: `get: statements`
fn format_get_body(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("get:"));

    let mut body_node: Option<Node> = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "body" {
            body_node = Some(child);
        }
    }

    if let Some(body) = body_node {
        parts.push(format_body(body, src, comments, config));
    } else {
        parts.push(indent(concat(vec![hardline(), text("pass")])));
    }

    concat(parts)
}

/// Find the value expression in a variable statement.
fn find_var_value<'a>(node: Node<'a>, _src: &str) -> Option<Node<'a>> {
    // The value is typically the last named child that isn't name/type/inferred_type/annotations.
    let count = node.named_child_count();
    for i in (0..count).rev() {
        if let Some(child) = node.named_child(i as u32) {
            if !matches!(
                child.kind(),
                "name" | "type" | "inferred_type" | "annotations" | "setget" | "static_keyword"
            ) {
                return Some(child);
            }
        }
    }
    None
}

/// Format a const statement.
fn format_const_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("const "));

    // Name
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            parts.push(text(node_text(child, src)));
            break;
        }
    }

    // Type annotation
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type" {
            parts.push(text(": "));
            parts.push(text(node_text(child, src)));
            break;
        }
    }

    // Inferred type
    let mut cursor = node.walk();
    let mut has_inferred = false;
    for child in node.children(&mut cursor) {
        if child.kind() == "inferred_type" {
            has_inferred = true;
            break;
        }
    }

    // Value
    let value = find_var_value(node, src);
    if let Some(val_node) = value {
        if has_inferred {
            parts.push(text(" := "));
        } else {
            parts.push(text(" = "));
        }
        parts.push(format_node(val_node, src, comments, config));
    }

    concat(parts)
}

/// Format parameters: `(param, param)` with grouping for line-breaking.
fn format_parameters(
    node: Node,
    src: &str,
    _comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut params: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        match child.kind() {
            "identifier" => {
                params.push(text(node_text(child, src)));
            }
            "typed_parameter" => {
                params.push(format_typed_param(child, src));
            }
            "typed_default_parameter" => {
                params.push(format_typed_default_param(child, src));
            }
            "default_parameter" => {
                params.push(format_default_param(child, src));
            }
            _ => {
                params.push(text(node_text(child, src)));
            }
        }
    }

    if params.is_empty() {
        return text("()");
    }

    let sep = concat(vec![text(","), line()]);
    group(concat(vec![
        text("("),
        indent(concat(vec![softline(), join(params, sep)])),
        trailing_comma(config),
        softline(),
        text(")"),
    ]))
}

/// Format a typed parameter: `name: Type`
fn format_typed_param(node: Node, src: &str) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut first = true;
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "identifier" || child.kind() == "name" {
            if first {
                parts.push(text(node_text(child, src)));
                first = false;
            }
        } else if child.kind() == "type" {
            parts.push(text(": "));
            parts.push(text(node_text(child, src)));
        }
    }
    concat(parts)
}

/// Format a typed default parameter: `name: Type = value`
fn format_typed_default_param(node: Node, src: &str) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut name_done = false;
    let mut type_done = false;
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        match child.kind() {
            "identifier" | "name" if !name_done => {
                parts.push(text(node_text(child, src)));
                name_done = true;
            }
            "type" => {
                parts.push(text(": "));
                parts.push(text(node_text(child, src)));
                type_done = true;
            }
            _ if name_done && (type_done || child.kind() != "type") => {
                // This is the default value.
                parts.push(text(" = "));
                parts.push(text(node_text(child, src)));
            }
            _ => {}
        }
    }
    concat(parts)
}

/// Format a default parameter: `name = value`
fn format_default_param(node: Node, src: &str) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut name_done = false;
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if !name_done {
            parts.push(text(node_text(child, src)));
            name_done = true;
        } else {
            parts.push(text(" = "));
            parts.push(text(node_text(child, src)));
        }
    }
    concat(parts)
}

/// Format an if statement.
fn format_if_stmt(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("if "));

    let mut cursor = node.walk();
    let mut condition_done = false;
    let mut cond_end_byte: usize = 0;
    let mut cond_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        match child.kind() {
            "body" if !condition_done => {
                parts.push(text(":"));
                if let Some(tc) = comments.take_trailing(cond_end_byte, cond_end_row) {
                    parts.push(line_suffix(text(format!(" {}", tc.text))));
                }
                parts.push(format_body(child, src, comments, config));
                condition_done = true;
            }
            "body" => {
                parts.push(format_body(child, src, comments, config));
            }
            "elif_clause" => {
                let leading = comments.take_leading(child.start_byte());
                for c in &leading {
                    parts.push(hardline());
                    parts.push(text(&c.text));
                }
                parts.push(hardline());
                parts.push(format_elif_clause(child, src, comments, config));
            }
            "else_clause" => {
                let leading = comments.take_leading(child.start_byte());
                for c in &leading {
                    parts.push(hardline());
                    parts.push(text(&c.text));
                }
                parts.push(hardline());
                parts.push(format_else_clause(child, src, comments, config));
            }
            _ if !child.is_named() => {}
            _ if !condition_done => {
                parts.push(format_node(child, src, comments, config));
                cond_end_byte = child.end_byte();
                cond_end_row = child.end_position().row;
                parts.push(text(":"));
                if let Some(tc) = comments.take_trailing(cond_end_byte, cond_end_row) {
                    parts.push(line_suffix(text(format!(" {}", tc.text))));
                }
                condition_done = true;
            }
            _ => {}
        }
    }

    concat(parts)
}

/// Format an elif clause.
fn format_elif_clause(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("elif "));

    let mut cursor = node.walk();
    let mut condition_done = false;
    let mut cond_end_byte: usize = 0;
    let mut cond_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        if child.kind() == "body" {
            if !condition_done {
                parts.push(text(":"));
                if let Some(tc) = comments.take_trailing(cond_end_byte, cond_end_row) {
                    parts.push(line_suffix(text(format!(" {}", tc.text))));
                }
                condition_done = true;
            }
            parts.push(format_body(child, src, comments, config));
        } else if !child.is_named() {
            continue;
        } else if !condition_done {
            parts.push(format_node(child, src, comments, config));
            cond_end_byte = child.end_byte();
            cond_end_row = child.end_position().row;
            parts.push(text(":"));
            if let Some(tc) = comments.take_trailing(cond_end_byte, cond_end_row) {
                parts.push(line_suffix(text(format!(" {}", tc.text))));
            }
            condition_done = true;
        }
    }

    concat(parts)
}

/// Format an else clause.
fn format_else_clause(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("else:"));

    let else_row = node.start_position().row;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "body" {
            // Consume trailing comment on the `else:` line (e.g., `else: # brake`).
            // The comment is classified as leading for the body node's start_byte
            // because the body is unnamed and find_next_code_sibling skips unnamed nodes.
            let body_leading = comments.take_leading(child.start_byte());
            for c in &body_leading {
                if c.start_row == else_row {
                    parts.push(line_suffix(text(format!(" {}", c.text))));
                }
            }
            parts.push(format_body(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format a for statement.
fn format_for_stmt(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("for "));

    let mut cursor = node.walk();
    let mut var_done = false;
    let mut iterable_done = false;
    let mut iter_end_byte: usize = 0;
    let mut iter_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        if child.kind() == "body" {
            parts.push(text(":"));
            if let Some(tc) = comments.take_trailing(iter_end_byte, iter_end_row) {
                parts.push(line_suffix(text(format!(" {}", tc.text))));
            }
            parts.push(format_body(child, src, comments, config));
        } else if !child.is_named() {
            continue;
        } else if !var_done {
            parts.push(format_node(child, src, comments, config));
            parts.push(text(" in "));
            var_done = true;
        } else if !iterable_done {
            parts.push(format_node(child, src, comments, config));
            iter_end_byte = child.end_byte();
            iter_end_row = child.end_position().row;
            iterable_done = true;
        }
    }

    concat(parts)
}

/// Format a while statement.
fn format_while_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("while "));

    let mut cursor = node.walk();
    let mut condition_done = false;
    let mut cond_end_byte: usize = 0;
    let mut cond_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        if child.kind() == "body" {
            parts.push(text(":"));
            if let Some(tc) = comments.take_trailing(cond_end_byte, cond_end_row) {
                parts.push(line_suffix(text(format!(" {}", tc.text))));
            }
            parts.push(format_body(child, src, comments, config));
        } else if !child.is_named() {
            continue;
        } else if !condition_done {
            parts.push(format_node(child, src, comments, config));
            cond_end_byte = child.end_byte();
            cond_end_row = child.end_position().row;
            condition_done = true;
        }
    }

    concat(parts)
}

/// Format a match statement.
fn format_match_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("match "));

    let mut cursor = node.walk();
    let mut expr_done = false;
    let mut expr_end_byte: usize = 0;
    let mut expr_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue;
        }
        if child.kind() == "match_body" {
            parts.push(text(":"));
            // Trailing comment after the colon (e.g., `match x:  # comment`)
            if let Some(tc) = comments.take_trailing(expr_end_byte, expr_end_row) {
                parts.push(line_suffix(text(format!(" {}", tc.text))));
            }
            parts.push(format_match_body(child, src, comments, config));
        } else if !child.is_named() {
            continue;
        } else if !expr_done {
            parts.push(format_node(child, src, comments, config));
            expr_end_byte = child.end_byte();
            expr_end_row = child.end_position().row;
            expr_done = true;
        }
    }

    concat(parts)
}

/// Format the body of a match statement (pattern sections).
fn format_match_body(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }
        if child.kind() == "pattern_section" || child.kind() == "match_branch" {
            let leading = comments.take_leading(child.start_byte());
            for c in &leading {
                parts.push(hardline());
                parts.push(text(&c.text));
            }
            parts.push(hardline());
            parts.push(format_pattern_section(child, src, comments, config));
        }
    }

    indent(concat(parts))
}

/// Format a pattern section within a match.
fn format_pattern_section(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut pattern_count = 0;
    let mut pattern_end_byte: usize = 0;
    let mut pattern_end_row: usize = 0;

    for child in node.children(&mut cursor) {
        if child.kind() == "comment" {
            continue; // Handled by comment store.
        }
        if child.kind() == "body" {
            parts.push(text(":"));
            // Trailing comment on the pattern line (e.g., `0: # REPLACE`).
            if let Some(tc) = comments.take_trailing(pattern_end_byte, pattern_end_row) {
                parts.push(line_suffix(text(format!(" {}", tc.text))));
            }
            parts.push(format_body(child, src, comments, config));
        } else if !child.is_named() {
            continue;
        } else {
            // Pattern value(s) — may be multiple (e.g., `2, 3:`).
            if pattern_count > 0 {
                parts.push(text(", "));
            }
            parts.push(format_node(child, src, comments, config));
            pattern_end_byte = child.end_byte();
            pattern_end_row = child.end_position().row;
            pattern_count += 1;
        }
    }

    concat(parts)
}

/// Format a return statement.
fn format_return_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let named_count = node.named_child_count();
    if named_count == 0 {
        return text("return");
    }

    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("return "));

    if let Some(expr) = node.named_child(0) {
        parts.push(format_node(expr, src, comments, config));
    }

    concat(parts)
}

/// Format an expression statement (wraps expressions like calls, assignments).
fn format_expression_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    if let Some(child) = node.named_child(0) {
        format_node(child, src, comments, config)
    } else {
        text(node_text(node, src))
    }
}

/// Format a signal statement.
fn format_signal_stmt(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("signal "));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "name" {
            parts.push(text(node_text(child, src)));
        } else if child.kind() == "parameters" {
            parts.push(format_parameters(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format an enum definition.
fn format_enum_def(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("enum "));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "name" {
            parts.push(text(node_text(child, src)));
            parts.push(text(" "));
        } else if child.kind() == "enumerator_list" {
            parts.push(format_enumerator_list(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format an enumerator list: `{ A, B, C }`
fn format_enumerator_list(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut items: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut prev_end_row: Option<usize> = None;
    let mut has_comments = false;

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }
        if child.kind() == "enumerator" {
            let leading = comments.take_leading(child.start_byte());
            let mut item_doc = format_enumerator(child, src, comments, config);

            // Preserve blank lines between groups of enumerators.
            let first_row = if let Some(first_comment) = leading.first() {
                first_comment.start_row
            } else {
                child.start_position().row
            };
            let has_blank_line = prev_end_row
                .map(|prev| first_row > prev + 1)
                .unwrap_or(false);

            if !leading.is_empty() || has_blank_line {
                has_comments = true;
                let mut parts: Vec<Doc> = Vec::new();
                // An extra hardline creates a blank line (the separator already
                // provides one newline before this item).
                if has_blank_line {
                    parts.push(hardline());
                }
                for c in &leading {
                    parts.push(text(&c.text));
                    parts.push(hardline());
                }
                parts.push(item_doc);
                item_doc = concat(parts);
            }

            if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
                has_comments = true;
                item_doc = concat(vec![item_doc, line_suffix(text(format!(" {}", tc.text)))]);
            }
            prev_end_row = Some(child.end_position().row);
            items.push(item_doc);
        }
    }

    if items.is_empty() {
        return text("{}");
    }

    let sep = concat(vec![text(","), line()]);
    let mut indent_content = vec![line(), join(items, sep)];
    // Force multiline when comments are present so trailing comments
    // stay associated with their enumerators.
    if has_comments {
        indent_content.push(break_parent());
    }

    group(concat(vec![
        text("{"),
        indent(concat(indent_content)),
        trailing_comma(config),
        line(),
        text("}"),
    ]))
}

/// Format a single enumerator: `NAME` or `NAME = value`
fn format_enumerator(
    node: Node,
    src: &str,
    _comments: &mut CommentStore,
    _config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut name_done = false;

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if !name_done {
            parts.push(text(node_text(child, src)));
            name_done = true;
        } else {
            parts.push(text(" = "));
            parts.push(text(node_text(child, src)));
        }
    }

    concat(parts)
}

/// Format an extends statement.
fn format_extends_stmt(node: Node, src: &str, _comments: &mut CommentStore) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("extends "));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "type" {
            parts.push(text(node_text(child, src)));
        }
    }

    concat(parts)
}

/// Format a class_name statement.
fn format_class_name_stmt(node: Node, src: &str, _comments: &mut CommentStore) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("class_name "));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" || child.kind() == "identifier" {
            parts.push(text(node_text(child, src)));
        }
    }

    concat(parts)
}

/// Format an annotation: `@export`, `@onready`, `@export_range(0, 100)`, etc.
fn format_annotation(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("@"));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "identifier" || child.kind() == "name" {
            parts.push(text(node_text(child, src)));
        } else if child.kind() == "arguments" {
            parts.push(format_arguments(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format a function call: `func(args)`
fn format_call(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "arguments" {
            parts.push(format_arguments(child, src, comments, config));
        } else {
            // The function expression (identifier, attribute, etc.)
            parts.push(format_node(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format arguments: `(arg, arg)` with grouping.
fn format_arguments(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut args: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }
        let leading = comments.take_leading(child.start_byte());
        let mut item_doc = format_node(child, src, comments, config);

        if !leading.is_empty() {
            let mut parts: Vec<Doc> = Vec::new();
            for c in &leading {
                parts.push(text(&c.text));
                parts.push(hardline());
            }
            parts.push(item_doc);
            item_doc = concat(parts);
        }

        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            item_doc = concat(vec![item_doc, line_suffix(text(format!(" {}", tc.text)))]);
        }
        args.push(item_doc);
    }

    if args.is_empty() {
        return text("()");
    }

    // Don't add trailing comma if the last argument is a lambda with a
    // multi-line body — the comma would appear at the body's indentation
    // level and be parsed as part of the lambda body by Godot.
    let has_trailing_multiline_lambda = has_multiline_lambda_last(node);

    let sep = concat(vec![text(","), line()]);
    let mut group_content = vec![text("("), indent(concat(vec![softline(), join(args, sep)]))];
    if !has_trailing_multiline_lambda {
        group_content.push(trailing_comma(config));
    }
    group_content.push(softline());
    group_content.push(text(")"));

    group(concat(group_content))
}

/// Check if the last named argument is a lambda with a multi-line body.
fn has_multiline_lambda_last(args_node: Node) -> bool {
    let mut last_named: Option<Node> = None;
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if child.is_named() && child.kind() != "comment" {
            last_named = Some(child);
        }
    }
    if let Some(last) = last_named {
        if last.kind() == "lambda" {
            let mut cursor = last.walk();
            for child in last.children(&mut cursor) {
                if child.kind() == "body" {
                    // Multi-line if more than one statement, or if the single
                    // statement is a compound statement (if/for/while/match).
                    if child.named_child_count() > 1 {
                        return true;
                    }
                    if let Some(stmt) = child.named_child(0) {
                        if is_compound_statement(stmt) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

/// Check if a node is a compound statement (has its own body/block).
fn is_compound_statement(node: Node) -> bool {
    matches!(
        node.kind(),
        "if_statement" | "for_statement" | "while_statement" | "match_statement"
    )
}

/// Format an attribute access: `obj.prop` or `obj.method()`
fn format_attribute(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut first = true;

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if first {
            parts.push(format_node(child, src, comments, config));
            first = false;
        } else if child.kind() == "attribute_call" {
            parts.push(text("."));
            parts.push(format_attribute_call(child, src, comments, config));
        } else {
            parts.push(text("."));
            parts.push(format_node(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format an attribute call: `method(args)`
fn format_attribute_call(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "arguments" {
            parts.push(format_arguments(child, src, comments, config));
        } else {
            parts.push(text(node_text(child, src)));
        }
    }

    concat(parts)
}

/// Format a binary operator expression: `left op right`
fn format_binary_op(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    // The operator is an unnamed child between the two named operands.
    // We need to find it by iterating all children.
    // Operators can be multi-token (e.g., `is not`, `not in`).
    let child_count = node.child_count();
    let mut left: Option<Node> = None;
    let mut op_parts: Vec<String> = Vec::new();
    let mut right: Option<Node> = None;

    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            if child.is_named() {
                if left.is_none() {
                    left = Some(child);
                } else if right.is_none() {
                    right = Some(child);
                }
            } else if left.is_some() && right.is_none() {
                let t = node_text(child, src).trim().to_string();
                if !t.is_empty() {
                    op_parts.push(t);
                }
            }
        }
    }
    let op: Option<String> = if op_parts.is_empty() {
        None
    } else {
        Some(op_parts.join(" "))
    };

    let left_doc = left
        .map(|n| format_node(n, src, comments, config))
        .unwrap_or_else(|| text(""));
    let op_str = op.unwrap_or_default();
    let right_doc = right
        .map(|n| format_node(n, src, comments, config))
        .unwrap_or_else(|| text(""));

    group(concat(vec![
        left_doc,
        text(format!(" {} ", op_str)),
        right_doc,
    ]))
}

/// Format a unary operator: `-x`, `not x`, `!x`
fn format_unary_op(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let child_count = node.child_count();
    let mut op: Option<String> = None;
    let mut operand: Option<Node> = None;

    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            if child.is_named() {
                operand = Some(child);
            } else {
                let t = node_text(child, src).trim().to_string();
                if !t.is_empty() {
                    op = Some(t);
                }
            }
        }
    }

    let op_str = op.unwrap_or_default();
    let operand_doc = operand
        .map(|n| format_node(n, src, comments, config))
        .unwrap_or_else(|| text(""));

    // Word operators like `not` need a space.
    if op_str == "not" {
        concat(vec![text("not "), operand_doc])
    } else {
        concat(vec![text(&op_str), operand_doc])
    }
}

/// Format an assignment: `target = value`
fn format_assignment(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut lhs_done = false;

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if !lhs_done {
            parts.push(format_node(child, src, comments, config));
            parts.push(text(" = "));
            lhs_done = true;
        } else {
            parts.push(format_node(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format an augmented assignment: `target += value`
fn format_augmented_assignment(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let child_count = node.child_count();
    let mut lhs: Option<Node> = None;
    let mut op: Option<String> = None;
    let mut rhs: Option<Node> = None;

    for i in 0..child_count {
        if let Some(child) = node.child(i as u32) {
            if child.is_named() {
                if lhs.is_none() {
                    lhs = Some(child);
                } else if rhs.is_none() {
                    rhs = Some(child);
                }
            } else {
                let t = node_text(child, src).trim().to_string();
                if !t.is_empty() {
                    op = Some(t);
                }
            }
        }
    }

    let lhs_doc = lhs
        .map(|n| format_node(n, src, comments, config))
        .unwrap_or_else(|| text(""));
    let op_str = op.unwrap_or_else(|| "=".to_string());
    let rhs_doc = rhs
        .map(|n| format_node(n, src, comments, config))
        .unwrap_or_else(|| text(""));

    concat(vec![lhs_doc, text(format!(" {} ", op_str)), rhs_doc])
}

/// Format an array literal: `[items]`
fn format_array(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut items: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }
        let leading = comments.take_leading(child.start_byte());
        let mut item_doc = format_node(child, src, comments, config);

        if !leading.is_empty() {
            let mut parts: Vec<Doc> = Vec::new();
            for c in &leading {
                parts.push(text(&c.text));
                parts.push(hardline());
            }
            parts.push(item_doc);
            item_doc = concat(parts);
        }

        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            item_doc = concat(vec![item_doc, line_suffix(text(format!(" {}", tc.text)))]);
        }
        items.push(item_doc);
    }

    // Orphan comments at the end of the array (after the last element).
    let orphan_comments = comments.take_leading(node.end_byte());

    if items.is_empty() && orphan_comments.is_empty() {
        return text("[]");
    }

    let sep = concat(vec![text(","), line()]);
    let mut indent_content = vec![softline(), join(items, sep)];

    let has_orphan = !orphan_comments.is_empty();
    for c in &orphan_comments {
        indent_content.push(hardline());
        indent_content.push(text(&c.text));
    }
    if has_orphan {
        indent_content.push(break_parent());
    }

    let mut group_content = vec![text("["), indent(concat(indent_content))];
    if !has_orphan {
        group_content.push(trailing_comma(config));
    }
    group_content.push(softline());
    group_content.push(text("]"));

    group(concat(group_content))
}

/// Format a dictionary literal: `{pairs}`
fn format_dictionary(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut pairs: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }
        let leading = comments.take_leading(child.start_byte());
        let mut item_doc = if child.kind() == "pair" {
            format_pair(child, src, comments, config)
        } else {
            format_node(child, src, comments, config)
        };

        // Attach leading comments above the item.
        if !leading.is_empty() {
            let mut parts: Vec<Doc> = Vec::new();
            for c in &leading {
                parts.push(text(&c.text));
                parts.push(hardline());
            }
            parts.push(item_doc);
            item_doc = concat(parts);
        }

        // Attach trailing comments.
        if let Some(tc) = comments.take_trailing(child.end_byte(), child.end_position().row) {
            item_doc = concat(vec![item_doc, line_suffix(text(format!(" {}", tc.text)))]);
        }
        pairs.push(item_doc);
    }

    // Orphan comments at the end of the dictionary (after the last pair).
    let orphan_comments = comments.take_leading(node.end_byte());

    if pairs.is_empty() && orphan_comments.is_empty() {
        return text("{}");
    }

    let sep = concat(vec![text(","), line()]);
    let mut indent_content = vec![softline(), join(pairs, sep)];

    let has_orphan = !orphan_comments.is_empty();
    for c in &orphan_comments {
        indent_content.push(hardline());
        indent_content.push(text(&c.text));
    }
    if has_orphan {
        indent_content.push(break_parent());
    }

    let mut group_content = vec![text("{"), indent(concat(indent_content))];
    if !has_orphan {
        group_content.push(trailing_comma(config));
    }
    group_content.push(softline());
    group_content.push(text("}"));

    group(concat(group_content))
}

/// Format a key-value pair: `key: value`
fn format_pair(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let mut cursor = node.walk();
    let mut key_done = false;

    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if !key_done {
            parts.push(format_node(child, src, comments, config));
            parts.push(text(": "));
            key_done = true;
        } else {
            parts.push(format_node(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format a conditional (ternary) expression: `value if condition else alternative`
fn format_conditional_expr(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut children: Vec<Node> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            children.push(child);
        }
    }

    if children.len() >= 3 {
        let value = format_node(children[0], src, comments, config);
        let condition = format_node(children[1], src, comments, config);
        let alternative = format_node(children[2], src, comments, config);
        group(concat(vec![
            value,
            text(" if "),
            condition,
            text(" else "),
            alternative,
        ]))
    } else {
        text(node_text(node, src))
    }
}

/// Format a lambda expression.
fn format_lambda(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("func"));

    let mut cursor = node.walk();
    let mut has_params = false;
    for child in node.children(&mut cursor) {
        if !child.is_named() {
            continue;
        }
        if child.kind() == "parameters" {
            parts.push(format_parameters(child, src, comments, config));
            has_params = true;
        } else if child.kind() == "name" {
            // Named lambdas (rare but possible)
            parts.push(text(" "));
            parts.push(text(node_text(child, src)));
        }
    }

    if !has_params {
        parts.push(text("()"));
    }

    // Check if the body is a single expression (inline lambda) or a block.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "body" {
            // Check if body has exactly one simple statement that fits inline.
            let stmt_count = child.named_child_count();
            if stmt_count == 1 {
                if let Some(stmt) = child.named_child(0) {
                    // Only inline simple expressions, not compound statements
                    // that have their own bodies (if, for, while, match).
                    if !is_compound_statement(stmt) {
                        // Inline: `func(x): expr`
                        parts.push(text(": "));
                        parts.push(format_node(stmt, src, comments, config));
                        return concat(parts);
                    }
                }
            }
            // Multi-line body
            parts.push(text(":"));
            parts.push(format_body(child, src, comments, config));
        }
    }

    concat(parts)
}

/// Format a parenthesized expression: `(expr)`
fn format_parenthesized(
    node: Node,
    src: &str,
    comments: &mut CommentStore,
    config: &FmtConfig,
) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    parts.push(text("("));

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            parts.push(format_node(child, src, comments, config));
        }
    }

    parts.push(text(")"));
    concat(parts)
}

/// Format a subscript expression: `expr[index]`
fn format_subscript(node: Node, src: &str, comments: &mut CommentStore, config: &FmtConfig) -> Doc {
    let mut parts: Vec<Doc> = Vec::new();
    let named_children: Vec<Node> = {
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .filter(|c| c.is_named())
            .collect()
    };

    if named_children.len() >= 2 {
        parts.push(format_node(named_children[0], src, comments, config));
        // named_children[1] is `subscript_arguments` which includes brackets in its text.
        // Format its inner named child (the index expression) and wrap in our own brackets.
        let subscript_args = named_children[1];
        let inner: Vec<Node> = {
            let mut cursor = subscript_args.walk();
            subscript_args
                .children(&mut cursor)
                .filter(|c| c.is_named())
                .collect()
        };
        if inner.len() == 1 {
            parts.push(text("["));
            parts.push(format_node(inner[0], src, comments, config));
            parts.push(text("]"));
        } else {
            // Fallback: emit the subscript_arguments as-is (already has brackets)
            parts.push(text(node_text(subscript_args, src)));
        }
    } else {
        parts.push(text(node_text(node, src)));
    }

    concat(parts)
}

/// Get the text of a node from the source.
fn node_text<'a>(node: Node, src: &'a str) -> &'a str {
    node.utf8_text(src.as_bytes()).unwrap_or("")
}
