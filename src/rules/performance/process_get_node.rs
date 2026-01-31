use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};
use super::{get_call_name, PROCESS_FUNCTIONS};

const RULE_ID: &str = "perf/process-get-node";

pub struct ProcessGetNode;

impl Rule for ProcessGetNode {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "get_node() call inside _process/_physics_process (cache with @onready)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "performance"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let root = ctx.parsed.root_node();
        let functions = parser::find_nodes_by_kind(root, "function_definition");

        for func in functions {
            let func_name = match func.child_by_field_name("name") {
                Some(n) => ctx.parsed.node_text(n),
                None => continue,
            };

            if !PROCESS_FUNCTIONS.contains(&func_name) {
                continue;
            }

            if let Some(body) = func.child_by_field_name("body") {
                check_get_node_recursive(body, ctx.parsed, func_name, &mut diagnostics);
            }
        }

        diagnostics
    }
}

fn check_get_node_recursive(
    node: Node,
    parsed: &ParsedFile,
    func_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if node.kind() == "call" {
        let call_text = get_call_name(node, parsed);
        if call_text == "get_node" || call_text == "get_node_or_null" {
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                    "`{}()` called inside `{}`. Cache the node with `@onready var node = $Path`.",
                    call_text, func_name
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

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_get_node_recursive(child, parsed, func_name, diagnostics);
    }
}
