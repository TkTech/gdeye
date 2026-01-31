use tree_sitter::Node;

use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;

use super::super::helpers::is_statement_node;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/missing-return";

pub struct MissingReturn;

impl Rule for MissingReturn {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function with return type has code paths without a return"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_missing_return(ctx.parsed, ctx.file_sym, &mut diagnostics);
        diagnostics
    }
}

fn check_missing_return(
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let functions = parser::find_nodes_by_kind(root, "function_definition");

    for func_node in functions {
        // Find the matching FuncDecl to check for return type
        let name = func_node
            .child_by_field_name("name")
            .map(|n| parsed.node_text(n).to_string());
        let name = match name {
            Some(n) => n,
            None => continue,
        };

        let return_type = file_sym
            .functions
            .iter()
            .find(|f| f.name == name)
            .and_then(|f| f.return_type.as_deref());

        match return_type {
            None | Some("void") => continue,
            Some(_) => {}
        }

        if let Some(body) = func_node.child_by_field_name("body") {
            if !body_definitely_returns(body, parsed) {
                let line = func_node.start_position().row + 1;
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Function `{}` declares a return type but not all code paths return a value.",
                        name
                    ),
                    line,
                ));
            }
        }
    }
}

/// Check if a block of statements definitely returns on all paths.
fn body_definitely_returns(body: Node, parsed: &ParsedFile) -> bool {
    let mut cursor = body.walk();
    let children: Vec<_> = body.children(&mut cursor).collect();

    // Find the last statement node
    let last_stmt = children.iter().rev().find(|c| is_statement_node(c.kind()));

    match last_stmt {
        None => false,
        Some(stmt) => statement_definitely_returns(*stmt, parsed),
    }
}

/// Check if a single statement definitely returns.
fn statement_definitely_returns(stmt: Node, parsed: &ParsedFile) -> bool {
    match stmt.kind() {
        "return_statement" => true,
        "if_statement" => if_definitely_returns(stmt, parsed),
        "match_statement" => match_definitely_returns(stmt, parsed),
        _ => false,
    }
}

/// Check if an if/elif/else chain definitely returns on all branches.
fn if_definitely_returns(node: Node, parsed: &ParsedFile) -> bool {
    let mut cursor = node.walk();
    let mut if_body_returns = false;
    let mut has_else = false;
    let mut else_returns = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "body" => {
                // The if-true branch body
                if_body_returns = body_definitely_returns(child, parsed);
            }
            "else_clause" => {
                // Only a true else_clause guarantees all paths are covered
                has_else = true;
                else_returns = else_clause_definitely_returns(child, parsed);
            }
            "elif_clause" => {
                // elif is another conditional branch - check if it terminates in else
                let (terminates, returns) = elif_chain_definitely_returns(child, parsed);
                has_else = terminates;
                else_returns = returns;
            }
            _ => {}
        }
    }

    has_else && if_body_returns && else_returns
}

/// Check if an elif chain terminates in an else clause and all branches return.
/// Returns (has_terminal_else, all_branches_return).
fn elif_chain_definitely_returns(node: Node, parsed: &ParsedFile) -> (bool, bool) {
    let mut cursor = node.walk();
    let mut body_returns = false;
    let mut has_else = false;
    let mut else_returns = false;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "body" => {
                body_returns = body_definitely_returns(child, parsed);
            }
            "else_clause" => {
                has_else = true;
                else_returns = else_clause_definitely_returns(child, parsed);
            }
            "elif_clause" => {
                // Nested elif - recurse
                let (terminates, returns) = elif_chain_definitely_returns(child, parsed);
                has_else = terminates;
                else_returns = returns;
            }
            _ => {}
        }
    }

    // This elif chain is complete only if it ends in else AND all branches return
    (has_else, body_returns && else_returns)
}

/// Check if an else clause definitely returns.
fn else_clause_definitely_returns(node: Node, parsed: &ParsedFile) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "body" => {
                return body_definitely_returns(child, parsed);
            }
            "if_statement" => {
                // else contains another if_statement (else: if ... pattern)
                return if_definitely_returns(child, parsed);
            }
            _ => {}
        }
    }
    false
}

/// Check if a match statement definitely returns on all branches.
fn match_definitely_returns(node: Node, parsed: &ParsedFile) -> bool {
    let mut has_catch_all = false;
    let mut all_return = true;
    let mut branch_count = 0;

    // Collect all branch nodes (may be direct children or inside match_body)
    let mut branches = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "match_branch" || child.kind() == "pattern_section" {
            branches.push(child);
        }
        if child.kind() == "match_body" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "match_branch" || inner.kind() == "pattern_section" {
                    branches.push(inner);
                }
            }
        }
    }

    for branch in &branches {
        branch_count += 1;

        // Check for catch-all pattern
        let mut pattern_cursor = branch.walk();
        for pat_child in branch.children(&mut pattern_cursor) {
            // Wildcard `_` appears as an identifier with text "_"
            if pat_child.kind() == "identifier" || pat_child.kind() == "name" {
                let text = parsed.node_text(pat_child);
                if text == "_" {
                    has_catch_all = true;
                }
            }
            // Some grammars use explicit wildcard nodes
            if pat_child.kind() == "_" || pat_child.kind() == "wildcard" {
                has_catch_all = true;
            }
            // Pattern wrapper nodes
            if pat_child.kind() == "pattern" || pat_child.kind() == "match_pattern" {
                let mut inner_cursor = pat_child.walk();
                for inner in pat_child.children(&mut inner_cursor) {
                    if (inner.kind() == "identifier" || inner.kind() == "name")
                        && parsed.node_text(inner) == "_"
                    {
                        has_catch_all = true;
                    }
                }
            }
        }

        // Check if the branch body returns
        if let Some(body) = branch.child_by_field_name("body") {
            if !body_definitely_returns(body, parsed) {
                all_return = false;
            }
        } else {
            // Try to find body node directly
            let mut body_cursor = branch.walk();
            let mut found_body = false;
            for c in branch.children(&mut body_cursor) {
                if c.kind() == "body" {
                    found_body = true;
                    if !body_definitely_returns(c, parsed) {
                        all_return = false;
                    }
                    break;
                }
            }
            if !found_body {
                all_return = false;
            }
        }
    }

    branch_count > 0 && has_catch_all && all_return
}
