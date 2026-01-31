use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "style/no-else-return";

pub struct NoElseReturn;

impl Rule for NoElseReturn {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Unnecessary else after return/break/continue"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_no_else_return(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_no_else_return(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let if_stmts = parser::find_nodes_by_kind(root, "if_statement");

    for if_stmt in if_stmts {
        let mut cursor = if_stmt.walk();
        let children: Vec<_> = if_stmt.children(&mut cursor).collect();

        // Find the if body and else clause
        let mut if_body = None;
        let mut else_node = None;

        for child in &children {
            if child.kind() == "body" && if_body.is_none() {
                if_body = Some(*child);
            }
            if child.kind() == "else_clause" {
                else_node = Some(*child);
            }
        }

        let if_body = match if_body {
            Some(b) => b,
            None => continue,
        };

        if else_node.is_none() {
            continue;
        }

        // Check if the last statement in the if body is return/break/continue
        if body_ends_with_jump(parsed, if_body) {
            let else_n = else_node.unwrap();
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Info,
                    "Unnecessary `else` after `return`/`break`/`continue` in if-body. \
                     Consider removing the `else` and un-indenting."
                        .to_string(),
                    else_n.start_position().row + 1,
                )
                .span(
                    else_n.start_position().column,
                    else_n.start_position().row + 1,
                    else_n.start_position().column + 4,
                ),
            );
        }
    }
}

fn body_ends_with_jump(_parsed: &ParsedFile, body: tree_sitter::Node) -> bool {
    let mut cursor = body.walk();
    let children: Vec<_> = body.children(&mut cursor).collect();

    // Find the last statement in the body
    let last_stmt = children.iter().rev().find(|c| {
        matches!(
            c.kind(),
            "return_statement"
                | "break_statement"
                | "continue_statement"
                | "expression_statement"
                | "variable_statement"
                | "if_statement"
                | "for_statement"
                | "while_statement"
                | "pass_statement"
        )
    });

    match last_stmt {
        Some(stmt) => matches!(
            stmt.kind(),
            "return_statement" | "break_statement" | "continue_statement"
        ),
        None => false,
    }
}
