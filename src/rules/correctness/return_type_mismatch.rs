use tree_sitter::Node;

use crate::classdb::ClassDb;
use crate::parser::ParsedFile;
use crate::symbols::FileSymbols;
use crate::types;

use super::super::helpers::{effective_var_type, is_user_enum, is_user_subclass_of};
use super::super::{DiagLabel, Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/return-type-mismatch";

pub struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Return expression type contradicts function return type annotation"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_return_type_mismatch(
            ctx.parsed,
            ctx.file_sym,
            ctx.all_file_symbols,
            ctx.class_db,
            &mut diagnostics,
        );
        diagnostics
    }
}

fn check_return_type_mismatch(
    parsed: &ParsedFile,
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

    // Build member var types
    let member_vars: Vec<(String, Option<String>)> = file_sym
        .variables
        .iter()
        .map(|v| {
            let effective_type = effective_var_type(v);
            (v.name.clone(), effective_type)
        })
        .collect();

    let root = parsed.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "function_definition" {
            continue;
        }

        // Find matching FuncDecl by name node
        let func_name_node = if let Some(n) = child.child_by_field_name("name") {
            n
        } else {
            let mut c = child.walk();
            let found = child.children(&mut c).find(|n| n.kind() == "name");
            match found {
                Some(n) => n,
                None => continue,
            }
        };
        let func_name = parsed.node_text(func_name_node);

        let func_decl = match file_sym.functions.iter().find(|f| f.name == func_name) {
            Some(f) => f,
            None => continue,
        };

        // Only check functions with an explicit return type annotation
        let return_type = match &func_decl.return_type {
            Some(rt) if !rt.is_empty() && rt != "void" => rt.clone(),
            _ => continue,
        };

        // Build local var types for this function
        let local_vars: Vec<(String, Option<String>)> = func_decl
            .local_vars
            .iter()
            .map(|v| {
                let effective_type = effective_var_type(v);
                (v.name.clone(), effective_type)
            })
            .collect();

        // Build param types
        let params: Vec<(String, Option<String>)> = func_decl
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.type_annotation.clone()))
            .collect();

        let type_ctx = types::ExprTypeContext {
            extends_class: &extends_class,
            local_vars: &local_vars,
            params: &params,
            member_vars: &member_vars,
            local_func_returns: &local_func_returns,
            class_db,
            type_refinements: None,
        };

        // Get the function declaration span for the secondary label
        let func_line = child.start_position().row + 1;
        let func_col = child.start_position().column;
        let func_end_col = child.end_position().column;
        // Find where the return type annotation ends (first line of the function def)
        let func_first_line_end = {
            let mut c = child.walk();
            let mut end = func_end_col;
            for ch in child.children(&mut c) {
                if ch.kind() == "body" {
                    // The colon before body ends the signature
                    end = ch.start_position().column;
                    break;
                }
            }
            end
        };

        // Find all return statements in this function
        collect_return_mismatches(
            child,
            parsed,
            func_name,
            &return_type,
            &type_ctx,
            all_file_symbols,
            func_line,
            func_col,
            func_first_line_end,
            diagnostics,
        );
    }
}

/// Recursively find return statements in a function body and check their types.
#[allow(clippy::too_many_arguments)]
fn collect_return_mismatches(
    node: Node,
    parsed: &ParsedFile,
    func_name: &str,
    declared_return: &str,
    ctx: &types::ExprTypeContext,
    all_file_symbols: &[FileSymbols],
    func_line: usize,
    func_col: usize,
    func_end_col: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Don't recurse into nested function definitions or lambdas
    if node.kind() == "lambda" {
        return;
    }
    if node.kind() == "function_definition" && node.parent().is_some_and(|p| p.kind() != "source") {
        return;
    }

    if node.kind() == "return_statement" {
        // Get the expression after "return"
        let mut cursor = node.walk();
        let expr_node = node.children(&mut cursor).find(|c| c.kind() != "return");

        if let Some(expr) = expr_node {
            if let Some(actual_type) = types::resolve_expr_type(expr, parsed, ctx) {
                // Check various compatibility rules
                let is_compatible = types::types_compatible(declared_return, &actual_type, ctx.class_db)
                    || is_user_subclass_of(
                        &actual_type,
                        declared_return,
                        all_file_symbols,
                        ctx.class_db,
                    )
                    // Enums are ints in GDScript
                    || (declared_return == "int" && is_user_enum(&actual_type, all_file_symbols))
                    // Unknown type (possibly from GDExtension) returning as known class - don't flag
                    || is_unknown_type_returning_as_engine_class(
                        &actual_type,
                        declared_return,
                        all_file_symbols,
                        ctx.class_db,
                    );

                if !is_compatible {
                    let start = node.start_position();
                    let end = node.end_position();
                    diagnostics.push(
                        Diagnostic::new(
                            RULE_ID,
                            Severity::Warning,
                            format!(
                                "Function `{}` returns `{}` but declared return type is `{}`.",
                                func_name, actual_type, declared_return
                            ),
                            start.row + 1,
                        )
                        .span(start.column, end.row + 1, end.column)
                        .with_label(DiagLabel {
                            message: format!("return type declared as `{}` here", declared_return),
                            line: func_line,
                            col: func_col,
                            end_line: func_line,
                            end_col: func_end_col,
                        })
                        .with_note(format!(
                            "`{}` is not compatible with `{}`",
                            actual_type, declared_return
                        )),
                    );
                }
            }
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_return_mismatches(
            child,
            parsed,
            func_name,
            declared_return,
            ctx,
            all_file_symbols,
            func_line,
            func_col,
            func_end_col,
            diagnostics,
        );
    }
}

/// Check if the actual type is unknown (not in ClassDB, not user-defined) but the
/// declared type is a known engine class. This allows GDExtension types to pass through
/// without false positives.
fn is_unknown_type_returning_as_engine_class(
    actual: &str,
    declared: &str,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> bool {
    // If actual type is known in ClassDB, this doesn't apply
    if class_db.class_exists(actual) || class_db.get_builtin_class(actual).is_some() {
        return false;
    }

    // If actual type is a user-defined class, this doesn't apply
    if all_file_symbols
        .iter()
        .any(|fs| fs.class_name.as_deref() == Some(actual))
    {
        return false;
    }

    // Actual is unknown - allow if declared is a known engine class
    class_db.class_exists(declared)
}
