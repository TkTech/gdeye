use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::helpers::is_statement_node;
use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "correctness/unreachable-code";

pub struct UnreachableCode;

impl Rule for UnreachableCode {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Code after return/break/continue is never executed"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_unreachable_code(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_unreachable_code(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let functions = parser::find_nodes_by_kind(root, "function_definition");

    for func in functions {
        if let Some(body) = func.child_by_field_name("body") {
            scan_block_for_unreachable(body, parsed, diagnostics);
        }
    }
}

/// Recursively scan a block node for statements after a terminator.
fn scan_block_for_unreachable(block: Node, parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let mut saw_terminator = false;
    let mut unreachable_start: Option<(usize, usize, usize)> = None; // (line, col, start_byte)
    let mut unreachable_end: Option<(usize, usize, usize)> = None; // (end_line, end_col, end_byte)

    let mut cursor = block.walk();
    for child in block.children(&mut cursor) {
        // Skip non-statement nodes (comments, whitespace, etc.)
        if !is_statement_node(child.kind()) {
            continue;
        }

        if saw_terminator {
            let line = child.start_position().row + 1;
            let col = child.start_position().column;
            let start_byte = child.start_byte();
            let end_line = child.end_position().row + 1;
            let end_col = child.end_position().column;
            let end_byte = child.end_byte();

            // Track the first unreachable statement
            if unreachable_start.is_none() {
                unreachable_start = Some((line, col, start_byte));
            }
            // Keep updating the end to capture all unreachable statements
            unreachable_end = Some((end_line, end_col, end_byte));
        } else if is_terminator(child.kind()) {
            saw_terminator = true;
        }

        // Recurse into nested blocks (if bodies, for bodies, etc.)
        recurse_into_child_blocks(child, parsed, diagnostics);
    }

    // Emit diagnostic with fix if we found unreachable code
    if let (Some((line, col, start_byte)), Some((end_line, end_col, end_byte))) =
        (unreachable_start, unreachable_end)
    {
        // Find the start of the line containing the unreachable code
        // to include the indentation/newline before it
        let source = parsed.source();
        let removal_start = find_line_start(source, start_byte);

        let fix = Fix::new_unsafe(
            "Remove unreachable code",
            vec![TextEdit {
                start_byte: removal_start,
                end_byte,
                replacement: String::new(),
            }],
        );

        diagnostics.push(
            Diagnostic::new(
                RULE_ID,
                Severity::Warning,
                "Unreachable code after terminating statement.",
                line,
            )
            .span(col, end_line, end_col)
            .with_fix(fix),
        );
    }
}

/// Find the start of the line containing the given byte offset.
fn find_line_start(source: &str, byte_offset: usize) -> usize {
    let bytes = source.as_bytes();
    // Walk backwards from the byte offset to find the newline
    let mut pos = byte_offset;
    while pos > 0 && bytes[pos - 1] != b'\n' {
        pos -= 1;
    }
    pos
}

/// Recurse into child nodes that contain statement blocks.
fn recurse_into_child_blocks(node: Node, parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "body" {
            scan_block_for_unreachable(child, parsed, diagnostics);
        } else {
            recurse_into_child_blocks(child, parsed, diagnostics);
        }
    }
}

fn is_terminator(kind: &str) -> bool {
    matches!(
        kind,
        "return_statement" | "break_statement" | "continue_statement"
    )
}
