use crate::symbols::VarDecl;

use super::super::helpers::effective_var_type;
use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "style/untyped-variable";

pub struct UntypedVariable;

impl Rule for UntypedVariable {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Variable lacks a type annotation when the type can be inferred"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Check file-scope variables
        for var in &ctx.file_sym.variables {
            if let Some(diag) = check_var(var, None) {
                diagnostics.push(diag);
            }
        }

        // Check local variables in functions
        for func in &ctx.file_sym.functions {
            for var in &func.local_vars {
                if let Some(diag) = check_var(var, Some(&func.name)) {
                    diagnostics.push(diag);
                }
            }
        }

        diagnostics
    }
}

fn check_var(var: &VarDecl, func_name: Option<&str>) -> Option<Diagnostic> {
    // Skip if already has a real type annotation (not `:=`)
    if let Some(ref ann) = var.type_annotation {
        if !ann.is_empty() && ann != ":=" {
            return None;
        }
        // `:=` means using inferred-type operator, already typed at runtime
        if ann == ":=" {
            return None;
        }
    }

    // Get the effective inferred type
    let inferred = effective_var_type(var)?;

    let message = if let Some(fname) = func_name {
        format!(
            "Variable `{}` in function `{}` has no type annotation (inferred: `{}`).",
            var.name, fname, inferred
        )
    } else {
        format!(
            "Variable `{}` has no type annotation (inferred: `{}`).",
            var.name, inferred
        )
    };

    let fix = Fix::new(
        format!("Add type annotation `: {}`", inferred),
        vec![TextEdit {
            start_byte: var.name_end_byte,
            end_byte: var.name_end_byte,
            replacement: format!(": {}", inferred),
        }],
    );

    let diag = Diagnostic::new(RULE_ID, Severity::Info, message, var.line).with_fix(fix);

    Some(diag)
}
