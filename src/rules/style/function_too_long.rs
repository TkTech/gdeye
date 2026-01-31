use super::super::{Diagnostic, OptionType, Rule, RuleContext, RuleOption, Severity};

const RULE_ID: &str = "style/function-too-long";

/// Default maximum function length in lines.
const DEFAULT_MAX_FUNCTION_LENGTH: usize = 80;

pub struct FunctionTooLong;

impl Rule for FunctionTooLong {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function exceeds maximum line count"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn options(&self) -> Vec<RuleOption> {
        vec![RuleOption {
            name: "max_length",
            description: "Maximum number of lines allowed in a function body",
            default: "80",
            value_type: OptionType::Integer,
        }]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let max_length = ctx
            .config
            .rule_option(RULE_ID, "max_length")
            .and_then(|v| v.as_integer())
            .unwrap_or(DEFAULT_MAX_FUNCTION_LENGTH as i64) as usize;

        let mut diagnostics = Vec::new();
        for func in &ctx.file_sym.functions {
            let length = func.end_line.saturating_sub(func.line) + 1;
            if length > max_length {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Info,
                    format!(
                        "Function `{}` is {} lines long (maximum: {}).",
                        func.name, length, max_length
                    ),
                    func.line,
                ));
            }
        }
        diagnostics
    }
}
