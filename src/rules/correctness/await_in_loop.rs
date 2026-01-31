use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/await-in-loop";

pub struct AwaitInLoop;

impl Rule for AwaitInLoop {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Await expression inside a loop body causes sequential execution"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_await_in_loop(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_await_in_loop(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let awaits = parser::find_nodes_by_kind(root, "await_expression");

    for await_node in awaits {
        // Walk ancestors looking for a loop, but stop at function boundaries
        let mut current = await_node.parent();
        while let Some(ancestor) = current {
            let kind = ancestor.kind();
            if kind == "function_definition" || kind == "lambda" {
                break;
            }
            if kind == "for_statement" || kind == "while_statement" {
                // Check if await uses the loop variable (for batching suggestion)
                let loop_var = get_loop_variable(ancestor, parsed);
                let await_uses_loop_var = loop_var
                    .as_ref()
                    .map(|v| await_uses_variable(await_node, v, parsed))
                    .unwrap_or(false);

                let mut diag =
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!(
                        "Await inside `{}` loop causes sequential execution of async operations.",
                        if kind == "for_statement" { "for" } else { "while" }
                    ),
                        await_node.start_position().row + 1,
                    )
                    .span(
                        await_node.start_position().column,
                        await_node.end_position().row + 1,
                        await_node.end_position().column,
                    );

                // Add note suggesting batching if await doesn't use loop variable
                if !await_uses_loop_var {
                    diag = diag.with_note(
                        "Consider batching: collect coroutines in an array and await them after the loop.",
                    );
                }

                diagnostics.push(diag);
                break;
            }
            current = ancestor.parent();
        }
    }
}

/// Get the loop variable name from a for_statement.
fn get_loop_variable(loop_node: Node, parsed: &ParsedFile) -> Option<String> {
    if loop_node.kind() != "for_statement" {
        return None;
    }

    // For loops: first identifier/name child is the loop variable
    let mut cursor = loop_node.walk();
    for child in loop_node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "name" {
            return Some(parsed.node_text(child).to_string());
        }
    }
    None
}

/// Check if an await expression uses a specific variable name.
fn await_uses_variable(await_node: Node, var_name: &str, parsed: &ParsedFile) -> bool {
    let identifiers = collect_identifiers(await_node, parsed);
    identifiers.contains(var_name)
}

/// Collect all identifier names used in a node and its descendants.
fn collect_identifiers(node: Node, parsed: &ParsedFile) -> HashSet<String> {
    let mut identifiers = HashSet::new();

    if node.kind() == "identifier" || node.kind() == "name" {
        identifiers.insert(parsed.node_text(node).to_string());
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        identifiers.extend(collect_identifiers(child, parsed));
    }

    identifiers
}
