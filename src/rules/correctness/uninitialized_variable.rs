use std::collections::HashSet;

use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/uninitialized-variable";

pub struct UninitializedVariable;

impl Rule for UninitializedVariable {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Variable may be used before initialization"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Collect all locally declared variable names by scanning the AST for each function.
        // This is more reliable than using the CFG because it properly distinguishes
        // var declarations from other identifier uses.
        let mut local_vars_by_func: std::collections::HashMap<String, HashSet<String>> =
            std::collections::HashMap::new();

        // Also collect lambda line ranges to filter out uses inside lambdas
        let mut lambda_ranges_by_func: std::collections::HashMap<String, Vec<(usize, usize)>> =
            std::collections::HashMap::new();

        for func_sym in &ctx.file_sym.functions {
            let local_vars = collect_local_var_declarations(ctx.parsed, &func_sym.name);
            local_vars_by_func.insert(func_sym.name.clone(), local_vars);

            let lambda_ranges = collect_lambda_ranges(ctx.parsed, &func_sym.name);
            lambda_ranges_by_func.insert(func_sym.name.clone(), lambda_ranges);
        }

        for (func_name, result) in &ctx.flow_results.functions {
            let local_vars = match local_vars_by_func.get(func_name) {
                Some(vars) => vars,
                None => continue,
            };
            let lambda_ranges = lambda_ranges_by_func
                .get(func_name)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            for (var_name, line) in &result.uninitialized_uses {
                // Only flag variables that are declared with `var` in this function
                if !local_vars.contains(var_name) {
                    continue;
                }
                // Skip underscore-prefixed variables (intentionally ignored)
                if var_name.starts_with('_') {
                    continue;
                }
                // Skip uses that occur inside lambda bodies (they have their own scope)
                if is_inside_lambda(*line, lambda_ranges) {
                    continue;
                }

                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!("Variable `{}` may be used before initialization.", var_name),
                    *line,
                ));
            }
        }

        diagnostics
    }
}

/// Check if a line number falls within any lambda body range.
fn is_inside_lambda(line: usize, lambda_ranges: &[(usize, usize)]) -> bool {
    lambda_ranges
        .iter()
        .any(|(start, end)| line >= *start && line <= *end)
}

/// Collect the line ranges of all lambda functions in a function body.
/// Returns a vector of (start_line, end_line) tuples.
fn collect_lambda_ranges(parsed: &ParsedFile, func_name: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    let functions = parser::find_nodes_by_kind(parsed.root_node(), "function_definition");
    for func in functions {
        let name = func
            .child_by_field_name("name")
            .map(|n| parsed.node_text(n));
        if name != Some(func_name) {
            continue;
        }

        if let Some(body) = func.child_by_field_name("body") {
            collect_lambdas_recursive(body, &mut ranges);
        }
    }

    ranges
}

/// Recursively collect lambda line ranges.
fn collect_lambdas_recursive(node: Node, ranges: &mut Vec<(usize, usize)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "lambda" {
            let start = child.start_position().row + 1;
            let end = child.end_position().row + 1;
            ranges.push((start, end));
            // Don't recurse into the lambda - we just want the outer boundaries
        } else {
            collect_lambdas_recursive(child, ranges);
        }
    }
}

/// Collect all variable names declared with `var` directly in a function body.
/// Excludes variables declared inside lambda functions.
fn collect_local_var_declarations(parsed: &ParsedFile, func_name: &str) -> HashSet<String> {
    let mut local_vars = HashSet::new();

    // Find the function definition
    let functions = parser::find_nodes_by_kind(parsed.root_node(), "function_definition");
    for func in functions {
        let name = func
            .child_by_field_name("name")
            .map(|n| parsed.node_text(n));
        if name != Some(func_name) {
            continue;
        }

        // Find the function body
        if let Some(body) = func.child_by_field_name("body") {
            collect_var_decls_in_scope(body, parsed, &mut local_vars);
        }
    }

    local_vars
}

/// Recursively collect variable declarations, but stop at lambda boundaries.
fn collect_var_decls_in_scope(node: Node, parsed: &ParsedFile, vars: &mut HashSet<String>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        // Skip lambda function bodies - they have their own scope
        if child.kind() == "lambda" {
            continue;
        }

        if child.kind() == "variable_statement" {
            // Extract the variable name
            if let Some(name_node) = child.child_by_field_name("name") {
                vars.insert(parsed.node_text(name_node).to_string());
            } else {
                // Fallback: find first identifier/name child
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "name" || inner.kind() == "identifier" {
                        vars.insert(parsed.node_text(inner).to_string());
                        break;
                    }
                }
            }
        }

        // Recurse into nested blocks (if, for, while, etc.) but not lambdas
        collect_var_decls_in_scope(child, parsed, vars);
    }
}
