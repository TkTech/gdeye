use super::super::helpers::CALLBACK_FUNCTIONS;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/unused-function";

pub struct UnusedFunction;

impl Rule for UnusedFunction {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function is declared but never called"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_unused_functions(ctx, &mut diagnostics);
        diagnostics
    }
}

fn check_unused_functions(ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    let parsed = ctx.parsed;
    let file_sym = ctx.file_sym;

    // Skip if file has dynamic access patterns
    let has_dynamic = crate::cross_file_usage::has_dynamic_access(parsed);
    if has_dynamic {
        return;
    }

    // Collect @rpc annotated function names (called by networking layer)
    let source = parsed.source();
    let rpc_functions = collect_rpc_functions(source);

    for func in &file_sym.functions {
        let func_key = (ctx.path.to_path_buf(), func.name.clone());

        // Check if function is reachable from entry points
        let is_reachable = ctx.reachable_functions.contains(&func_key);

        // Also check cross-file usage flag (for cases reachability doesn't cover)
        if is_reachable || func.used {
            continue;
        }

        // Skip functions starting with _ (private/callbacks/overrides)
        // These are entry points, so should already be in reachable_functions,
        // but check anyway for safety
        if func.name.starts_with('_') {
            continue;
        }

        // Skip known Godot engine callbacks
        if CALLBACK_FUNCTIONS.contains(&func.name.as_str()) {
            continue;
        }

        // Skip static functions on classes with class_name (may be called externally)
        // We detect this conservatively: if file has a class_name, skip all functions
        // since they could be called via ClassName.func() from files we haven't analyzed
        if file_sym.class_name.is_some() {
            continue;
        }

        // Skip @rpc annotated functions (called by networking layer)
        if rpc_functions.contains(&func.name.as_str()) {
            continue;
        }

        // Determine if it has any call sites (even from dead code)
        let has_call_sites = ctx
            .call_graph
            .get_call_sites(ctx.path, &func.name)
            .is_some_and(|sites| !sites.is_empty());

        // Provide different message based on whether it has callers
        let message = if has_call_sites {
            format!(
                "Function `{}` is only called from other dead code.",
                func.name
            )
        } else {
            format!("Function `{}` is declared but never called.", func.name)
        };

        diagnostics.push(Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            message,
            func.line,
        ));
    }
}

/// Collect function names that are annotated with @rpc.
fn collect_rpc_functions(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("@rpc") {
            // Look for the next `func` declaration
            for next_line in &lines[i + 1..] {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with("func ") {
                    if let Some(name) = next_trimmed
                        .strip_prefix("func ")
                        .and_then(|s| s.split('(').next())
                    {
                        result.push(name.trim());
                    }
                    break;
                }
                // Skip empty lines and comments between @rpc and func
                if !next_trimmed.is_empty()
                    && !next_trimmed.starts_with('#')
                    && !next_trimmed.starts_with('@')
                {
                    break;
                }
            }
        }
    }
    result
}
