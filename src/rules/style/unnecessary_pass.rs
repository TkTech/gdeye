use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "style/unnecessary-pass";

pub struct UnnecessaryPass;

impl Rule for UnnecessaryPass {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Redundant `pass` statement in a body with other statements"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_unnecessary_pass(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_unnecessary_pass(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let pass_nodes = parser::find_nodes_by_kind(root, "pass_statement");

    for pass_node in pass_nodes {
        let parent = match pass_node.parent() {
            Some(p) => p,
            None => continue,
        };

        // Count named siblings (other statements in the same body, excluding comments)
        let mut cursor = parent.walk();
        let sibling_count = parent
            .children(&mut cursor)
            .filter(|c| c.is_named() && c.id() != pass_node.id() && c.kind() != "comment")
            .count();

        if sibling_count > 0 {
            let fix = make_line_removal_fix(
                parsed.source(),
                pass_node.start_byte(),
                pass_node.end_byte(),
            );
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Info,
                    "Unnecessary `pass` statement (body has other statements).",
                    pass_node.start_position().row + 1,
                )
                .span(
                    pass_node.start_position().column,
                    pass_node.end_position().row + 1,
                    pass_node.end_position().column,
                )
                .with_fix(fix),
            );
        }
    }
}

/// Create a fix that removes a statement including its full line.
fn make_line_removal_fix(source: &str, start_byte: usize, end_byte: usize) -> Fix {
    let bytes = source.as_bytes();

    // Extend backward to start of line (consume leading whitespace)
    let mut remove_start = start_byte;
    while remove_start > 0 && bytes[remove_start - 1] != b'\n' {
        remove_start -= 1;
    }

    // Extend forward to include the newline
    let mut remove_end = end_byte;
    while remove_end < bytes.len() && bytes[remove_end] != b'\n' {
        remove_end += 1;
    }
    if remove_end < bytes.len() && bytes[remove_end] == b'\n' {
        remove_end += 1;
    }

    Fix::new(
        "Remove unnecessary `pass`",
        vec![TextEdit {
            start_byte: remove_start,
            end_byte: remove_end,
            replacement: String::new(),
        }],
    )
}
