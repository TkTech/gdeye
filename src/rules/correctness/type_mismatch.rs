use crate::classdb::ClassDb;
use crate::symbols::{FileSymbols, VarDecl};
use crate::types;

use super::super::helpers::is_user_subclass_of;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/type-mismatch";

pub struct TypeMismatch;

impl Rule for TypeMismatch {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Variable type annotation contradicts initializer type"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_type_mismatch(
            ctx.file_sym,
            ctx.all_file_symbols,
            ctx.class_db,
            &mut diagnostics,
        );
        diagnostics
    }
}

fn check_type_mismatch(
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let local_func_returns: Vec<(String, Option<String>)> = file_sym
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.return_type.clone()))
        .collect();
    let extends_class = file_sym
        .extends
        .clone()
        .unwrap_or_else(|| "RefCounted".to_string());

    // Check member variables
    for var in &file_sym.variables {
        check_var_type_mismatch(
            var,
            &local_func_returns,
            &extends_class,
            all_file_symbols,
            class_db,
            diagnostics,
        );
    }

    // Check function local variables
    for func in &file_sym.functions {
        for var in &func.local_vars {
            check_var_type_mismatch(
                var,
                &local_func_returns,
                &extends_class,
                all_file_symbols,
                class_db,
                diagnostics,
            );
        }
    }
}

fn check_var_type_mismatch(
    var: &VarDecl,
    local_func_returns: &[(String, Option<String>)],
    extends_class: &str,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Only check if we have an explicit annotation
    let annotation = match &var.type_annotation {
        Some(a) => a,
        None => return,
    };

    // Skip `:=` (type inference operator) - not a real type annotation
    if annotation == ":=" || annotation.is_empty() {
        return;
    }

    // Determine the actual type from the initializer
    let init_type = if let Some(ref init) = var.initializer_type {
        // Direct initializer type (literal/constructor)
        init.clone()
    } else if let Some(ref call_name) = var.initializer_call {
        // Resolve call return type from ClassDB or local functions
        match types::resolve_call_return_type(
            call_name,
            local_func_returns,
            extends_class,
            class_db,
        ) {
            Some(t) => t,
            None => return,
        }
    } else {
        return;
    };

    if !types::types_compatible(annotation, &init_type, class_db)
        && !is_user_subclass_of(&init_type, annotation, all_file_symbols, class_db)
    {
        diagnostics.push(Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            format!(
                "Variable `{}` declared as `{}` but initialized with `{}`.",
                var.name, annotation, init_type
            ),
            var.line,
        ));
    }
}
