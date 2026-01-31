use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "style/naming-convention";

/// Check if a name follows PascalCase convention.
fn is_pascal_case(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Must start with uppercase
    let first = name.chars().next().unwrap();
    if !first.is_uppercase() {
        return false;
    }
    // Must not contain underscores (except trailing for disambiguation)
    // Allow digits
    !name.contains('_')
}

/// Check if a name follows UPPER_SNAKE_CASE convention (for constants).
fn is_upper_snake_case(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let first = name.chars().next().unwrap();
    if !first.is_uppercase() {
        return false;
    }
    name.chars()
        .all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Check if a name follows snake_case convention.
fn is_snake_case(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Must start with lowercase or underscore
    let first = name.chars().next().unwrap();
    if !first.is_lowercase() && first != '_' {
        return false;
    }
    // All characters must be lowercase, digits, or underscores
    name.chars()
        .all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '_')
}

pub struct NamingConvention;

impl Rule for NamingConvention {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Naming convention violation (snake_case functions, PascalCase classes)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_naming_conventions(ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

fn check_naming_conventions(file_sym: &FileSymbols, diagnostics: &mut Vec<Diagnostic>) {
    // Check class_name
    if let Some(ref name) = file_sym.class_name {
        if !is_pascal_case(name) {
            diagnostics.push(Diagnostic::new(
                RULE_ID,
                Severity::Info,
                format!("Class name `{}` should use PascalCase.", name),
                1,
            ));
        }
    }

    // Check inner class names
    for cls in &file_sym.inner_classes {
        if !is_pascal_case(&cls.name) {
            diagnostics.push(Diagnostic::new(
                RULE_ID,
                Severity::Info,
                format!("Class `{}` should use PascalCase.", cls.name),
                cls.line,
            ));
        }
    }

    // Check function names (skip built-in overrides starting with _)
    for func in &file_sym.functions {
        if func.name.starts_with('_') {
            continue; // Godot callbacks like _ready, _process
        }
        if !is_snake_case(&func.name) {
            diagnostics.push(Diagnostic::new(
                RULE_ID,
                Severity::Info,
                format!("Function `{}` should use snake_case.", func.name),
                func.line,
            ));
        }
    }

    // Check member variable names (allow UPPER_SNAKE_CASE for constant-like vars)
    for var in &file_sym.variables {
        if var.name.starts_with('_') {
            continue; // Private convention
        }
        if !is_snake_case(&var.name) && !is_upper_snake_case(&var.name) {
            diagnostics.push(Diagnostic::new(
                RULE_ID,
                Severity::Info,
                format!("Variable `{}` should use snake_case.", var.name),
                var.line,
            ));
        }
    }
}
