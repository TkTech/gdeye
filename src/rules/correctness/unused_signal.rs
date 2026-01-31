use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/unused-signal";

pub struct UnusedSignal;

impl Rule for UnusedSignal {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Signal is declared but never emitted or connected"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for signal in &ctx.file_sym.signals {
            if !signal.used {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Signal `{}` is declared but never emitted or connected in this file.",
                        signal.name
                    ),
                    signal.line,
                ));
            }
        }
        diagnostics
    }
}
