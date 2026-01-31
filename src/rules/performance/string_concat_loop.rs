use tree_sitter::Node;

use crate::parser;
use crate::types::{resolve_expr_type, ExprTypeContext};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "perf/string-concat-in-loop";

pub struct StringConcatLoop;

impl Rule for StringConcatLoop {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "String concatenation inside loop creates many intermediate strings"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "performance"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_string_concat_in_loops(ctx, &mut diagnostics);
        diagnostics
    }
}

fn check_string_concat_in_loops(ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    let root = ctx.parsed.root_node();

    // Find all for and while loops
    let for_loops = parser::find_nodes_by_kind(root, "for_statement");
    let while_loops = parser::find_nodes_by_kind(root, "while_statement");

    for loop_node in for_loops.into_iter().chain(while_loops) {
        if let Some(body) = loop_node.child_by_field_name("body") {
            check_body_for_string_concat(body, ctx, diagnostics);
        }
    }
}

fn check_body_for_string_concat(node: Node, ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    // Check for augmented assignment with += on strings
    if node.kind() == "augmented_assignment" {
        check_augmented_assignment(node, ctx, diagnostics);
    }

    // Check for assignment with binary + operation on strings
    if node.kind() == "assignment" {
        check_assignment_with_concat(node, ctx, diagnostics);
    }

    // Recurse into children (but stop at nested function definitions/lambdas)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition" || child.kind() == "lambda" {
            continue;
        }
        check_body_for_string_concat(child, ctx, diagnostics);
    }
}

fn check_augmented_assignment(node: Node, ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.len() < 3 {
        return;
    }

    // Check if operator is +=
    let operator = ctx.parsed.node_text(children[1]);
    if operator != "+=" {
        return;
    }

    let lhs = children[0];
    let rhs = children[2];

    // Check if the LHS or RHS is a string type
    // (LHS could be typed String, or RHS could be a string literal)
    if is_string_type(lhs, ctx) || is_string_type(rhs, ctx) {
        emit_diagnostic(node, ctx, diagnostics);
    }
}

fn check_assignment_with_concat(node: Node, ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.len() < 3 {
        return;
    }

    // Check if RHS is a binary operation with +
    let rhs = children[2];
    if rhs.kind() != "binary_operator" {
        return;
    }

    // Check if the binary operator is +
    let mut rhs_cursor = rhs.walk();
    let rhs_children: Vec<_> = rhs.children(&mut rhs_cursor).collect();
    if rhs_children.len() < 3 {
        return;
    }

    let op = ctx.parsed.node_text(rhs_children[1]);
    if op != "+" {
        return;
    }

    // Check if either operand is a string
    if is_string_type(rhs_children[0], ctx) || is_string_type(rhs_children[2], ctx) {
        emit_diagnostic(node, ctx, diagnostics);
    }
}

fn is_string_type(node: Node, ctx: &RuleContext) -> bool {
    // Check if the node is a string literal
    if node.kind() == "string" {
        return true;
    }

    // Build type context for resolution
    let extends_class = ctx.file_sym.extends.as_deref().unwrap_or("RefCounted");

    // Collect all local variables from all functions
    let mut all_local_vars: Vec<(String, Option<String>)> = Vec::new();
    for func in &ctx.file_sym.functions {
        for var in &func.local_vars {
            all_local_vars.push((var.name.clone(), var.inferred_type.clone()));
        }
    }

    let member_vars: Vec<(String, Option<String>)> = ctx
        .file_sym
        .variables
        .iter()
        .map(|v| (v.name.clone(), v.inferred_type.clone()))
        .collect();

    let local_func_returns: Vec<(String, Option<String>)> = ctx
        .file_sym
        .functions
        .iter()
        .map(|f| {
            (
                f.name.clone(),
                f.return_type.clone().or(f.inferred_return_type.clone()),
            )
        })
        .collect();

    let type_ctx = ExprTypeContext {
        extends_class,
        local_vars: &all_local_vars,
        params: &[],
        member_vars: &member_vars,
        local_func_returns: &local_func_returns,
        class_db: ctx.class_db,
        type_refinements: None,
    };

    // Try to resolve the expression type
    if let Some(ty) = resolve_expr_type(node, ctx.parsed, &type_ctx) {
        return ty == "String";
    }

    false
}

fn emit_diagnostic(node: Node, _ctx: &RuleContext, diagnostics: &mut Vec<Diagnostic>) {
    let line = node.start_position().row + 1;

    // Check if we already reported at this line to avoid duplicates
    if diagnostics
        .iter()
        .any(|d| d.rule == RULE_ID && d.line == line)
    {
        return;
    }

    diagnostics.push(
        Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            "String concatenation in loop. Consider using Array.append() with join(), or PackedStringArray.",
            line,
        )
        .span(
            node.start_position().column,
            node.end_position().row + 1,
            node.end_position().column,
        )
        .with_note("Each concatenation creates a new String, causing repeated allocations."),
    );
}
