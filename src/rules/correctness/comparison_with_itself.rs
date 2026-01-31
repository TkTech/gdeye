use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/comparison-with-itself";

pub struct ComparisonWithItself;

impl Rule for ComparisonWithItself {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Expression compared with itself is always true or always false"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_comparison_with_itself(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

/// Check if a node contains any function or method calls.
/// Function calls can have side effects or return different values on each invocation,
/// so comparing `get_value() == get_value()` may not be a bug.
fn contains_call(node: tree_sitter::Node) -> bool {
    // Check if this node is a call
    if node.kind() == "call" || node.kind() == "attribute_call" {
        return true;
    }
    // Check children recursively
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_call(child) {
            return true;
        }
    }
    false
}

fn check_comparison_with_itself(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let bin_ops = parser::find_nodes_by_kind(root, "binary_operator");

    for node in bin_ops {
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        if children.len() < 3 {
            continue;
        }

        let op = parsed.node_text(children[1]);
        if !matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=" | "is") {
            continue;
        }

        let left = children[0];
        let right = children[2];

        // Skip comparisons involving function/method calls - they may return different values
        if contains_call(left) || contains_call(right) {
            continue;
        }

        let left_text = parsed.node_text(left);
        let right_text = parsed.node_text(right);

        // Only flag if the expressions are identical and non-trivial
        // (not just a single character, to avoid false positives on typos)
        if left_text == right_text && !left_text.is_empty() {
            let always = match op {
                "==" | "<=" | ">=" | "is" => "always true",
                "!=" | "<" | ">" => "always false",
                _ => "suspicious",
            };
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Comparison `{} {} {}` is {} (comparing expression with itself).",
                        left_text, op, right_text, always
                    ),
                    node.start_position().row + 1,
                )
                .span(
                    node.start_position().column,
                    node.end_position().row + 1,
                    node.end_position().column,
                ),
            );
        }
    }
}
