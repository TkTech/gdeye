use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/shadowed-variable";

pub struct ShadowedVariable;

impl Rule for ShadowedVariable {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Local variable shadows a member variable or parameter"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_shadowed_variables(ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

fn check_shadowed_variables(file_sym: &FileSymbols, diagnostics: &mut Vec<Diagnostic>) {
    for func in &file_sym.functions {
        // Collect names that exist in outer scopes
        let member_names: Vec<&str> = file_sym.variables.iter().map(|v| v.name.as_str()).collect();
        let param_names: Vec<&str> = func.parameters.iter().map(|p| p.name.as_str()).collect();

        for var in &func.local_vars {
            if var.name.starts_with('_') {
                continue;
            }

            if member_names.contains(&var.name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Local variable `{}` in `{}` shadows a member variable.",
                        var.name, func.name
                    ),
                    var.line,
                ));
            } else if param_names.contains(&var.name.as_str()) {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Local variable `{}` in `{}` shadows a parameter.",
                        var.name, func.name
                    ),
                    var.line,
                ));
            }
        }
    }
}
