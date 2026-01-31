use crate::parser::ParsedFile;
use crate::symbols::{FileSymbols, VarDecl};

use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "correctness/dead-store";

/// Unified rule for detecting dead stores (unused variables and useless assignments).
///
/// This rule detects:
/// 1. Variables declared but never read
/// 2. Values assigned to variables but overwritten before being read
/// 3. Values assigned in the last statement of scope (value lost)
pub struct DeadStore;

impl Rule for DeadStore {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Value written to variable is never read"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check unused variables (declared but never read)
        check_unused_variables(ctx.parsed, ctx.file_sym, &mut diagnostics);

        // Check dead assignments from flow analysis (assigned but overwritten before read)
        check_dead_assignments(ctx, &mut diagnostics);

        diagnostics
    }
}

/// Check for variables that are declared but never used.
fn check_unused_variables(
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = parsed.source();

    // Check member variables (file-scope) — only if file has no dynamic access patterns
    let has_dynamic = crate::cross_file_usage::has_dynamic_access(parsed);
    if !has_dynamic {
        for var in &file_sym.variables {
            if !var.used
                && !var.name.starts_with('_')
                && !var.is_export
                && !var.is_onready
                && var.scope == crate::symbols::Scope::File
            {
                let fix = make_safe_fix(source, var);
                let mut diag = Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!("Member variable `{}` is declared but never used.", var.name),
                    var.line,
                );
                if let Some(f) = fix {
                    diag = diag.with_fix(f);
                }
                diagnostics.push(diag);
            }
        }
    }

    // Check function-local variables
    for func in &file_sym.functions {
        for var in &func.local_vars {
            if !var.used && !var.name.starts_with('_') {
                let fix = make_safe_fix(source, var);
                let mut diag = Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Local variable `{}` in function `{}` is declared but never used.",
                        var.name, func.name
                    ),
                    var.line,
                );
                if let Some(f) = fix {
                    diag = diag.with_fix(f);
                }
                diagnostics.push(diag);
            }
        }
    }
}

/// Check for dead assignments from flow analysis.
fn check_dead_assignments(ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    for result in ctx.flow_results.functions.values() {
        for (var_name, line) in &result.dead_assignments {
            // Skip _ prefixed variables (intentionally unused)
            if var_name.starts_with('_') {
                continue;
            }

            // Avoid duplicate: if this line was already flagged as unused variable, skip
            if diagnostics
                .iter()
                .any(|d| d.line == *line && d.message.contains(&format!("`{}`", var_name)))
            {
                continue;
            }

            diagnostics.push(Diagnostic::new(
                RULE_ID,
                Severity::Warning,
                format!(
                    "Value assigned to `{}` is never read (overwritten or lost).",
                    var_name
                ),
                *line,
            ));
        }
    }
}

/// Create a safe fix for an unused variable.
///
/// Returns None if we can't safely suggest a fix.
/// Returns a removal fix only if:
/// - The variable has no initializer with function calls (potential side effects)
/// - The line contains only this variable declaration
///
/// Otherwise returns a rename fix to prefix with `_`.
fn make_safe_fix(source: &str, var: &VarDecl) -> Option<Fix> {
    // If the variable has an initializer with a function call, the call might have
    // side effects. In this case, only offer to prefix with underscore, not remove.
    if var.initializer_call.is_some() {
        return Some(make_prefix_fix(var));
    }

    // Check if removal is safe by verifying the line contains only this declaration
    if is_safe_to_remove(source, var.start_byte, var.end_byte) {
        Some(make_removal_fix(
            source,
            var.start_byte,
            var.end_byte,
            &var.name,
        ))
    } else {
        // Line has other content, offer prefix fix instead
        Some(make_prefix_fix(var))
    }
}

/// Create a fix that prefixes the variable name with underscore.
fn make_prefix_fix(var: &VarDecl) -> Fix {
    Fix::new(
        format!(
            "Prefix `{}` with underscore to mark as intentionally unused",
            var.name
        ),
        vec![TextEdit {
            start_byte: var.name_start_byte,
            end_byte: var.name_end_byte,
            replacement: format!("_{}", var.name),
        }],
    )
}

/// Check if a variable statement can be safely removed.
///
/// Returns true only if:
/// - The line contains only whitespace before the statement
/// - The line contains only whitespace/comments after the statement (up to newline)
fn is_safe_to_remove(source: &str, start_byte: usize, end_byte: usize) -> bool {
    let bytes = source.as_bytes();

    // Check what's before the statement on the same line
    let mut pos = start_byte;
    while pos > 0 && bytes[pos - 1] != b'\n' {
        pos -= 1;
        // Only whitespace allowed before the statement
        if !bytes[pos].is_ascii_whitespace() {
            return false;
        }
    }

    // Check what's after the statement on the same line
    let mut pos = end_byte;
    while pos < bytes.len() && bytes[pos] != b'\n' {
        let ch = bytes[pos];
        // Allow whitespace and comments after
        if ch == b'#' {
            // Comment - skip to end of line, that's fine
            break;
        }
        if !ch.is_ascii_whitespace() {
            return false;
        }
        pos += 1;
    }

    true
}

/// Create a fix that removes a variable statement, including its full line.
fn make_removal_fix(source: &str, start_byte: usize, end_byte: usize, var_name: &str) -> Fix {
    let bytes = source.as_bytes();

    // Extend backward to start of line (consume leading whitespace/indentation)
    let mut line_start = start_byte;
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }

    // Extend forward to consume trailing newline
    let mut line_end = end_byte;
    while line_end < bytes.len() && bytes[line_end] != b'\n' {
        line_end += 1;
    }
    // Consume the newline itself
    if line_end < bytes.len() && bytes[line_end] == b'\n' {
        line_end += 1;
    }

    Fix::new_unsafe(
        format!("Remove unused variable `{}`", var_name),
        vec![TextEdit {
            start_byte: line_start,
            end_byte: line_end,
            replacement: String::new(),
        }],
    )
}
