use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "style/untyped-return";

pub struct UntypedReturn;

impl Rule for UntypedReturn {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function has no return type annotation"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_untyped_return(ctx.parsed, ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

fn check_untyped_return(
    parsed: &ParsedFile,
    file_sym: &crate::symbols::FileSymbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let func_defs = parser::find_nodes_by_kind(root, "function_definition");

    for func_node in func_defs {
        // Get function name
        let func_name = match func_node.child_by_field_name("name") {
            Some(n) => parsed.node_text(n),
            None => continue,
        };

        // Find matching FuncDecl
        let func_decl = match file_sym.functions.iter().find(|f| f.name == func_name) {
            Some(f) => f,
            None => continue,
        };

        // Skip if has explicit return type
        if func_decl.return_type.is_some() {
            continue;
        }

        let mut diag = Diagnostic::new(
            RULE_ID,
            Severity::Info,
            format!("Function `{}` has no return type annotation.", func_name),
            func_decl.line,
        );

        // If we have an inferred return type, create a fix
        if let Some(ref inferred) = func_decl.inferred_return_type {
            if let Some(fix) = make_return_type_fix(func_node, inferred) {
                diag = diag.with_fix(fix);
            }
        }

        diagnostics.push(diag);
    }
}

/// Create a fix that adds `-> Type` after the parameter list.
fn make_return_type_fix(func_node: tree_sitter::Node, return_type: &str) -> Option<Fix> {
    // Find the closing paren of the parameters
    let mut cursor = func_node.walk();
    let mut insert_pos = None;

    for child in func_node.children(&mut cursor) {
        if child.kind() == "parameters" {
            // Insert right after the parameters node
            insert_pos = Some(child.end_byte());
            break;
        }
        // Handle case where there's a ")" directly
        if child.kind() == ")" {
            insert_pos = Some(child.end_byte());
        }
    }

    let insert_byte = insert_pos?;

    Some(Fix::new(
        format!("Add return type `{}`", return_type),
        vec![TextEdit {
            start_byte: insert_byte,
            end_byte: insert_byte,
            replacement: format!(" -> {}", return_type),
        }],
    ))
}
