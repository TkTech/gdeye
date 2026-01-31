use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/await-correctness";

pub struct AwaitCorrectness;

impl Rule for AwaitCorrectness {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Incorrect usage of await (in _process or on non-coroutine)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_await_in_process(ctx.parsed, &mut diagnostics);
        check_await_on_non_coroutine(ctx.parsed, ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

/// Check for await inside _process/_physics_process functions.
fn check_await_in_process(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let funcs = parser::find_nodes_by_kind(root, "function_definition");

    for func in funcs {
        let mut cursor = func.walk();
        let children: Vec<_> = func.children(&mut cursor).collect();

        let name_node = children.iter().find(|c| c.kind() == "name");
        let name = match name_node {
            Some(n) => parsed.node_text(*n),
            None => continue,
        };

        if name != "_process" && name != "_physics_process" {
            continue;
        }

        // Find all await nodes inside this function
        let awaits = parser::find_nodes_by_kind(func, "await_expression");
        for await_node in awaits {
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Await inside `{}` will suspend the frame callback, \
                         which is almost certainly unintended.",
                        name
                    ),
                    await_node.start_position().row + 1,
                )
                .span(
                    await_node.start_position().column,
                    await_node.end_position().row + 1,
                    await_node.end_position().column,
                ),
            );
        }
    }
}

/// Check for await on a call to a local function that is not a coroutine.
fn check_await_on_non_coroutine(
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let awaits = parser::find_nodes_by_kind(root, "await_expression");

    // Collect local functions that contain await (are coroutines)
    let coroutine_names: Vec<&str> = file_sym
        .functions
        .iter()
        .filter(|f| function_contains_await(parsed, f.name.as_str()))
        .map(|f| f.name.as_str())
        .collect();

    for await_node in awaits {
        let mut cursor = await_node.walk();
        let children: Vec<_> = await_node.children(&mut cursor).collect();

        // await node's child should be the expression being awaited
        let awaited_expr = match children.last() {
            Some(n) if n.kind() != "await" => *n,
            _ => continue,
        };

        // Only check simple function calls (not method calls, signal access, etc.)
        if awaited_expr.kind() != "call" {
            continue;
        }

        let mut call_cursor = awaited_expr.walk();
        let call_children: Vec<_> = awaited_expr.children(&mut call_cursor).collect();

        if call_children.is_empty() {
            continue;
        }

        let func_name_node = call_children[0];
        // Skip method calls (attribute nodes like obj.method)
        if func_name_node.kind() == "attribute" {
            continue;
        }

        let func_name = parsed.node_text(func_name_node);

        // Skip if function is not defined locally
        let is_local = file_sym.functions.iter().any(|f| f.name == func_name);
        if !is_local {
            continue;
        }

        // Skip if function is a coroutine (contains await)
        if coroutine_names.contains(&func_name) {
            continue;
        }

        diagnostics.push(
            Diagnostic::new(
                RULE_ID,
                Severity::Warning,
                format!(
                    "Awaiting `{}` which is not a coroutine (does not contain await).",
                    func_name
                ),
                await_node.start_position().row + 1,
            )
            .span(
                await_node.start_position().column,
                await_node.end_position().row + 1,
                await_node.end_position().column,
            ),
        );
    }
}

/// Check if a function (by name) in the parsed file contains an await expression.
fn function_contains_await(parsed: &ParsedFile, name: &str) -> bool {
    let root = parsed.root_node();
    let funcs = parser::find_nodes_by_kind(root, "function_definition");

    for func in funcs {
        let mut cursor = func.walk();
        let children: Vec<_> = func.children(&mut cursor).collect();

        let name_node = children.iter().find(|c| c.kind() == "name");
        let func_name = match name_node {
            Some(n) => parsed.node_text(*n),
            None => continue,
        };

        if func_name == name {
            let awaits = parser::find_nodes_by_kind(func, "await_expression");
            return !awaits.is_empty();
        }
    }

    false
}
