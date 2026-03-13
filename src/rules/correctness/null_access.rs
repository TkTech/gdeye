use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/null-access";

/// Functions that can return null.
/// Note: `get_node` and `$` are NOT included — they throw on missing nodes
/// rather than returning null. Only `get_node_or_null` returns null.
const NULLABLE_FUNCTIONS: &[&str] = &[
    "get_node_or_null",
    "find_child",
    "get_child",
    "get_parent",
    "get_owner",
];

pub struct NullAccess;

impl Rule for NullAccess {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Potential null reference: member access on nullable expression"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_chained_null_access(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

/// Check for immediate member access on nullable function calls:
/// `get_node("x").method()` or `find_child("x").property`
fn check_chained_null_access(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let attrs = parser::find_nodes_by_kind(root, "attribute");

    for attr in attrs {
        let mut cursor = attr.walk();
        let children: Vec<_> = attr.children(&mut cursor).collect();

        if children.len() < 2 {
            continue;
        }

        let receiver = children[0];

        // Check if receiver is a call to a nullable function
        if receiver.kind() != "call" {
            continue;
        }

        let func_name = get_call_function_name(parsed, receiver);
        let func_name = match func_name {
            Some(n) => n,
            None => continue,
        };

        if !NULLABLE_FUNCTIONS.contains(&func_name.as_str()) {
            continue;
        }

        // Get the full call expression as guard key
        let guard_key = get_guard_key(parsed, receiver);

        // Skip if inside a guard checking this expression
        if is_inside_guard(attr, parsed, &guard_key) {
            continue;
        }

        diagnostics.push(
            Diagnostic::new(
                RULE_ID,
                Severity::Warning,
                format!(
                    "Potential null reference: `{}()` can return null. \
                     Consider assigning to a variable and checking for null first.",
                    func_name
                ),
                attr.start_position().row + 1,
            )
            .span(
                attr.start_position().column,
                attr.end_position().row + 1,
                attr.end_position().column,
            ),
        );
    }
}

fn get_call_function_name(parsed: &ParsedFile, call_node: tree_sitter::Node) -> Option<String> {
    let mut cursor = call_node.walk();
    let children: Vec<_> = call_node.children(&mut cursor).collect();

    if children.is_empty() {
        return None;
    }

    let func_node = children[0];

    if func_node.kind() == "identifier" {
        return Some(parsed.node_text(func_node).to_string());
    }

    // Method call: obj.method - get the method name
    if func_node.kind() == "attribute" {
        let mut attr_cursor = func_node.walk();
        let attr_children: Vec<_> = func_node.children(&mut attr_cursor).collect();
        if let Some(last) = attr_children.last() {
            if last.kind() == "identifier" {
                return Some(parsed.node_text(*last).to_string());
            }
        }
    }

    None
}

/// Check if a node is inside a guarded block (if statement checking the node).
///
/// Returns true if the node is inside:
/// - `if node:` or `if $Path:`
/// - `if is_instance_valid(node):` or `if has_node("path"):`
/// - `if node != null:` or `if node == null:` (negated check)
fn is_inside_guard(node: tree_sitter::Node, parsed: &ParsedFile, guard_text: &str) -> bool {
    let mut current = node.parent();

    while let Some(parent) = current {
        // Check if we're inside an if_statement body
        if parent.kind() == "if_statement" {
            // Get the condition
            if let Some(condition) = parent.child_by_field_name("condition") {
                if check_condition_guards(condition, parsed, guard_text) {
                    return true;
                }
            }
        }

        current = parent.parent();
    }

    false
}

/// Check if a condition expression guards against the given guard_text.
/// Uses AST-based matching instead of string matching.
fn check_condition_guards(
    condition: tree_sitter::Node,
    parsed: &ParsedFile,
    guard_text: &str,
) -> bool {
    match condition.kind() {
        // Direct truthiness check: `if node:` or `if $Path:`
        "identifier" | "name" => parsed.node_text(condition) == guard_text,
        "get_node" => {
            // `if $Path:` - compare the full get_node text
            parsed.node_text(condition) == guard_text
        }
        // Function call: `if is_instance_valid(node):` or `if has_node("Path"):`
        "call" => check_guard_call(condition, parsed, guard_text),
        // Binary comparison: `if node != null:` or `if node == null:`
        "binary_operator" => check_guard_binary(condition, parsed, guard_text),
        // Boolean operators: `if node and other:` or `if node or other:`
        "boolean_operator" => {
            // Check both sides of the boolean operator
            let mut cursor = condition.walk();
            for child in condition.children(&mut cursor) {
                if child.is_named() && check_condition_guards(child, parsed, guard_text) {
                    return true;
                }
            }
            false
        }
        // Unary not: `if not node:` - still a guard (inverted logic)
        "unary_operator" => {
            let mut cursor = condition.walk();
            for child in condition.children(&mut cursor) {
                if child.is_named() && check_condition_guards(child, parsed, guard_text) {
                    return true;
                }
            }
            false
        }
        // Parenthesized: `if (node):`
        "parenthesized_expression" => {
            let mut cursor = condition.walk();
            for child in condition.children(&mut cursor) {
                if child.is_named() && check_condition_guards(child, parsed, guard_text) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Check if a call expression is a guard for the given expression.
/// Handles: is_instance_valid(expr), has_node("path")
fn check_guard_call(call: tree_sitter::Node, parsed: &ParsedFile, guard_text: &str) -> bool {
    let mut cursor = call.walk();
    let children: Vec<_> = call.children(&mut cursor).collect();

    if children.is_empty() {
        return false;
    }

    let func_node = children[0];
    let func_name = if func_node.kind() == "identifier" || func_node.kind() == "name" {
        parsed.node_text(func_node)
    } else {
        return false;
    };

    // Find the arguments node
    let args_node = children.iter().find(|c| c.kind() == "arguments");
    let args_node = match args_node {
        Some(n) => *n,
        None => return false,
    };

    // Get the first argument
    let mut args_cursor = args_node.walk();
    let first_arg = args_node.children(&mut args_cursor).find(|c| c.is_named());
    let first_arg = match first_arg {
        Some(a) => a,
        None => return false,
    };

    match func_name {
        "is_instance_valid" => {
            // `is_instance_valid(node)` - compare argument to guard
            parsed.node_text(first_arg) == guard_text
        }
        "has_node" => {
            // `has_node("Path")` - extract path and compare to $Path or get_node("Path")
            if first_arg.kind() == "string" {
                let path_text = parsed.node_text(first_arg);
                let path = path_text.trim_matches('"').trim_matches('\'');

                // Match against $Path
                let dollar_form = format!("${}", path);
                if guard_text == dollar_form {
                    return true;
                }

                // Match against get_node("Path")
                if guard_text.starts_with("get_node(") && guard_text.contains(path) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Check if a binary expression is a null guard.
/// Handles: `node != null`, `node == null`, `null != node`, `null == node`
fn check_guard_binary(binary: tree_sitter::Node, parsed: &ParsedFile, guard_text: &str) -> bool {
    let mut cursor = binary.walk();
    let children: Vec<_> = binary
        .children(&mut cursor)
        .filter(|c| c.is_named())
        .collect();

    if children.len() < 2 {
        return false;
    }

    let left = children[0];
    let right = children[children.len() - 1];

    let left_text = parsed.node_text(left);
    let right_text = parsed.node_text(right);

    // Check if one side is null and the other matches guard_text
    (left_text == "null" && right_text == guard_text)
        || (right_text == "null" && left_text == guard_text)
}

/// Extract a guard key from a nullable expression (the text to look for in an if condition).
fn get_guard_key(parsed: &ParsedFile, node: tree_sitter::Node) -> String {
    parsed.node_text(node).to_string()
}
