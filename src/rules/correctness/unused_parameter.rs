use crate::symbols::FileSymbols;

use super::super::helpers::CALLBACK_FUNCTIONS;
use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "correctness/unused-parameter";

pub struct UnusedParameter;

impl Rule for UnusedParameter {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function parameter is never used"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_unused_parameters(ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

fn check_unused_parameters(file_sym: &FileSymbols, diagnostics: &mut Vec<Diagnostic>) {
    for func in &file_sym.functions {
        // Skip Godot callback functions where unused params are normal
        if CALLBACK_FUNCTIONS.contains(&func.name.as_str()) {
            continue;
        }
        // Skip private functions that start with _ (often overrides)
        if func.name.starts_with('_') {
            continue;
        }

        for param in &func.parameters {
            if !param.used && !param.name.starts_with('_') {
                let fix = Fix::new(
                    format!("Rename `{}` to `_{}`", param.name, param.name),
                    vec![TextEdit {
                        start_byte: param.name_start_byte,
                        end_byte: param.name_end_byte,
                        replacement: format!("_{}", param.name),
                    }],
                );
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Info,
                    format!(
                        "Parameter `{}` in function `{}` is never used. Prefix with `_` to suppress.",
                        param.name, func.name
                    ),
                    param.line,
                ).with_fix(fix));
            }
        }
    }
}
