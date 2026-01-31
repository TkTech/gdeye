use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "perf/loop-invariant";

/// Detects loop-invariant expressions that could be hoisted outside the loop.
///
/// This rule focuses on **computation hoisting** - expressions that are computed
/// repeatedly but don't change between iterations. For allocation-specific issues
/// (Array/Dictionary literals, .new() calls), see the `perf/allocation` rule.
pub struct LoopInvariant;

impl Rule for LoopInvariant {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Computation inside loop is invariant and could be hoisted"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "performance"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        let for_loops = parser::find_nodes_by_kind(ctx.parsed.root_node(), "for_statement");
        let while_loops = parser::find_nodes_by_kind(ctx.parsed.root_node(), "while_statement");

        for loop_node in for_loops.iter().chain(while_loops.iter()) {
            check_loop(*loop_node, ctx.parsed, &mut diagnostics);
        }

        diagnostics
    }
}

fn check_loop(loop_node: Node, parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    // 1. Collect modified vars (loop var + all assignments in body)
    let mut modified = HashSet::new();

    // For 'for' loops, get the loop variable
    if loop_node.kind() == "for_statement" {
        if let Some(var) = get_loop_var(loop_node, parsed) {
            modified.insert(var);
        }
    }

    // Collect all assignments in the loop body
    if let Some(body) = loop_node.child_by_field_name("body") {
        collect_modified_vars(body, parsed, &mut modified);
    }

    // 2. Find invariant expensive expressions in the loop body
    if let Some(body) = loop_node.child_by_field_name("body") {
        find_invariant_expressions(body, parsed, &modified, diagnostics);
    }
}

/// Get the loop variable name from a for statement.
fn get_loop_var(for_node: Node, parsed: &ParsedFile) -> Option<String> {
    let mut cursor = for_node.walk();
    for child in for_node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "name" {
            return Some(parsed.node_text(child).to_string());
        }
        // Stop at 'in' keyword or body
        if child.kind() == "in" || child.kind() == "body" {
            break;
        }
    }
    None
}

/// Collect all variables that are modified (assigned) within a node tree.
fn collect_modified_vars(node: Node, parsed: &ParsedFile, modified: &mut HashSet<String>) {
    match node.kind() {
        "assignment" | "augmented_assignment" => {
            // Get the LHS of the assignment
            if let Some(lhs) = node.child(0) {
                if lhs.kind() == "identifier" || lhs.kind() == "name" {
                    modified.insert(parsed.node_text(lhs).to_string());
                }
            }
        }
        "variable_statement" => {
            // Variable declaration - extract name
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "name" || child.kind() == "identifier" {
                    modified.insert(parsed.node_text(child).to_string());
                    break;
                }
            }
        }
        _ => {}
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_modified_vars(child, parsed, modified);
    }
}

/// Find invariant expressions in the loop body and report diagnostics.
fn find_invariant_expressions(
    node: Node,
    parsed: &ParsedFile,
    modified: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Check variable statements with initializers
    if node.kind() == "variable_statement" {
        if let Some(value) = node.child_by_field_name("value") {
            if is_invariant(value, parsed, modified) && is_expensive(value) {
                let line = node.start_position().row + 1;
                let expr_text = truncate_text(parsed.node_text(value), 30);
                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Info,
                        format!(
                            "Expression `{}` is loop-invariant and could be hoisted outside the loop.",
                            expr_text
                        ),
                        line,
                    )
                    .span(
                        node.start_position().column,
                        node.end_position().row + 1,
                        node.end_position().column,
                    ),
                );
                return; // Don't recurse further for this node
            }
        }
    }

    // Recurse into children, but skip nested loops (they have their own scope)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "for_statement" && child.kind() != "while_statement" {
            find_invariant_expressions(child, parsed, modified, diagnostics);
        }
    }
}

/// Check if an expression is loop-invariant (doesn't depend on modified vars).
fn is_invariant(node: Node, parsed: &ParsedFile, modified: &HashSet<String>) -> bool {
    match node.kind() {
        "identifier" | "name" => {
            let name = parsed.node_text(node);
            !modified.contains(name)
        }
        "integer" | "float" | "string" | "true" | "false" | "null" => true,
        // Array/dictionary literals are handled by perf/allocation rule
        // Don't flag them here to avoid duplicate diagnostics
        "dictionary" | "array" => false,
        // Function calls are never considered invariant - we can't know if they
        // have side effects or call non-deterministic functions internally
        "call" | "attribute_call" => false,
        _ => {
            // For other nodes, check all named children
            let mut cursor = node.walk();
            let children: Vec<_> = node
                .children(&mut cursor)
                .filter(|c| c.is_named())
                .collect();
            children
                .into_iter()
                .all(|c| is_invariant(c, parsed, modified))
        }
    }
}

/// Check if an expression is "expensive" enough to warrant hoisting.
///
/// Note: Array/dictionary allocations are handled by perf/allocation,
/// so this focuses on other expensive operations like complex arithmetic.
fn is_expensive(node: Node) -> bool {
    match node.kind() {
        // Allocations are handled by perf/allocation - skip here
        "dictionary" | "array" => false,
        // Binary operations with multiple operators could be worth hoisting
        "binary_operator" => {
            // Check if there are nested binary operations (complex expression)
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            children.iter().any(|c| c.kind() == "binary_operator")
        }
        _ => {
            // Check if any child is expensive
            let mut cursor = node.walk();
            let children: Vec<_> = node
                .children(&mut cursor)
                .filter(|c| c.is_named())
                .collect();
            children.into_iter().any(|c| is_expensive(c))
        }
    }
}

/// Truncate text for display in diagnostic messages.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len - 3])
    }
}
