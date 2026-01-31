use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/self-assignment";

pub struct SelfAssignment;

impl Rule for SelfAssignment {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Variable is assigned to itself"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_self_assignment(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_self_assignment(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let assignments = parser::find_nodes_by_kind(root, "assignment");

    for node in assignments {
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        if children.len() < 3 {
            continue;
        }

        // Check operator is plain `=` (skip +=, -=, etc.)
        let op = parsed.node_text(children[1]);
        if op != "=" {
            continue;
        }

        let lhs = parsed.node_text(children[0]);
        let rhs = parsed.node_text(children[2]);

        if lhs == rhs && !lhs.is_empty() {
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!("Self-assignment: `{}` is assigned to itself.", lhs),
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
