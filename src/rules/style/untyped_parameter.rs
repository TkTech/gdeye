use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "style/untyped-parameter";

pub struct UntypedParameter;

impl Rule for UntypedParameter {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function parameter lacks a type annotation"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for func in &ctx.file_sym.functions {
            for param in &func.parameters {
                if param.type_annotation.is_some() {
                    continue;
                }

                let mut diag = if let Some(ref inferred) = param.inferred_type {
                    // We have an inferred type from call site analysis
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Info,
                        format!(
                            "Parameter `{}` in function `{}` has no type annotation (inferred: `{}`).",
                            param.name, func.name, inferred
                        ),
                        param.line,
                    )
                } else {
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Info,
                        format!(
                            "Parameter `{}` in function `{}` has no type annotation.",
                            param.name, func.name
                        ),
                        param.line,
                    )
                };

                // Add fix if we have an inferred type
                if let Some(ref inferred) = param.inferred_type {
                    let fix = Fix::new(
                        format!("Add type annotation `: {}`", inferred),
                        vec![TextEdit {
                            start_byte: param.name_end_byte,
                            end_byte: param.name_end_byte,
                            replacement: format!(": {}", inferred),
                        }],
                    );
                    diag = diag.with_fix(fix);
                }

                diagnostics.push(diag);
            }
        }
        diagnostics
    }
}
