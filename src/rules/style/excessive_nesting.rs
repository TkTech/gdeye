use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, OptionType, Rule, RuleContext, RuleOption, Severity};

const RULE_ID: &str = "style/excessive-nesting";

/// Default maximum nesting depth.
const DEFAULT_MAX_NESTING_DEPTH: usize = 5;

/// Recursively measure the maximum nesting depth of control structures.
fn measure_max_depth(node: tree_sitter::Node, current_depth: usize) -> usize {
    let nesting_kinds = [
        "if_statement",
        "for_statement",
        "while_statement",
        "match_statement",
    ];

    let mut max = current_depth;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let child_depth = if nesting_kinds.contains(&child.kind()) {
            measure_max_depth(child, current_depth + 1)
        } else {
            measure_max_depth(child, current_depth)
        };
        if child_depth > max {
            max = child_depth;
        }
    }

    max
}

pub struct ExcessiveNesting;

impl Rule for ExcessiveNesting {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Function has excessive nesting depth"
    }

    fn default_severity(&self) -> Severity {
        Severity::Info
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn options(&self) -> Vec<RuleOption> {
        vec![RuleOption {
            name: "max_depth",
            description: "Maximum nesting depth of control structures allowed",
            default: "5",
            value_type: OptionType::Integer,
        }]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let max_depth = ctx
            .config
            .rule_option(RULE_ID, "max_depth")
            .and_then(|v| v.as_integer())
            .unwrap_or(DEFAULT_MAX_NESTING_DEPTH as i64) as usize;

        let mut diagnostics = Vec::new();
        check_nesting_depth(ctx.parsed, max_depth, &mut diagnostics);
        diagnostics
    }
}

fn check_nesting_depth(parsed: &ParsedFile, max_depth: usize, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let functions = parser::find_nodes_by_kind(root, "function_definition");

    for func in functions {
        let func_name = match func.child_by_field_name("name") {
            Some(n) => parsed.node_text(n).to_string(),
            None => continue,
        };

        if let Some(body) = func.child_by_field_name("body") {
            let depth = measure_max_depth(body, 0);
            if depth > max_depth {
                diagnostics.push(Diagnostic::new(
                    RULE_ID,
                    Severity::Info,
                    format!(
                        "Function `{}` has nesting depth {} (maximum: {}).",
                        func_name, depth, max_depth
                    ),
                    func.start_position().row + 1,
                ));
            }
        }
    }
}
