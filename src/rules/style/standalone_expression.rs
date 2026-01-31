use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "style/standalone-expression";

pub struct StandaloneExpression;

impl Rule for StandaloneExpression {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Expression statement with no side effect"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_standalone_expressions(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_standalone_expressions(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let expr_stmts = parser::find_nodes_by_kind(root, "expression_statement");

    for stmt in expr_stmts {
        let mut cursor = stmt.walk();
        let expr = match stmt.children(&mut cursor).find(|c| c.is_named()) {
            Some(e) => e,
            None => continue,
        };

        if is_side_effect_free(&expr, parsed) {
            let text = parsed.node_text(expr);
            // Truncate display for long expressions
            let display = if text.len() > 40 {
                format!("{}...", &text[..37])
            } else {
                text.to_string()
            };
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Expression `{}` has no side effect and its value is unused.",
                        display
                    ),
                    stmt.start_position().row + 1,
                )
                .span(
                    stmt.start_position().column,
                    stmt.end_position().row + 1,
                    stmt.end_position().column,
                ),
            );
        }
    }
}

/// Check if an expression node is side-effect-free (no calls, assignments, or awaits).
fn is_side_effect_free(node: &tree_sitter::Node, parsed: &ParsedFile) -> bool {
    match node.kind() {
        // These are always pure
        "integer" | "float" | "string" | "true" | "false" | "null" | "identifier" | "name"
        | "array" | "dictionary" => true,

        // Arithmetic/comparison/logical — pure if operands are pure
        "binary_operator" | "unary_operator" => {
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            if children.len() >= 3 {
                let op = parsed.node_text(children[1]);
                // `as` and `is` are pure type checks/casts
                if matches!(op, "as" | "is") {
                    return true;
                }
            }
            children
                .iter()
                .filter(|c| c.is_named())
                .all(|c| is_side_effect_free(c, parsed))
        }

        // Parenthesized expression — check inner
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            children
                .iter()
                .filter(|c| c.is_named())
                .all(|c| is_side_effect_free(c, parsed))
        }

        // Attribute access without a call (e.g., `obj.property`) is pure
        "attribute" => {
            // Check if this attribute has a call child (attribute_call)
            let mut cursor = node.walk();
            let has_call = node
                .children(&mut cursor)
                .any(|c| c.kind() == "attribute_call");
            !has_call
        }

        // Subscript access (e.g., `arr[0]`) is pure
        "subscript" => true,

        // Ternary — pure if all branches are pure
        "conditional_expression" | "ternary_expression" => {
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            children
                .iter()
                .filter(|c| c.is_named())
                .all(|c| is_side_effect_free(c, parsed))
        }

        // Everything else (calls, assignments, await, yield) — not pure
        _ => false,
    }
}
