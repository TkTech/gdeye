use std::collections::HashMap;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/duplicated-load";

pub struct DuplicatedLoad;

impl Rule for DuplicatedLoad {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Same resource loaded multiple times"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_duplicated_load(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_duplicated_load(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let calls = parser::find_nodes_by_kind(root, "call");

    let mut seen: HashMap<String, usize> = HashMap::new();

    for call in calls {
        let mut cursor = call.walk();
        let children: Vec<_> = call.children(&mut cursor).collect();

        if children.is_empty() {
            continue;
        }

        let func_name = parsed.node_text(children[0]);
        if func_name != "load" && func_name != "preload" {
            continue;
        }

        // Find the argument list
        let args_node = children.iter().find(|c| c.kind() == "arguments");
        let args_node = match args_node {
            Some(n) => *n,
            None => continue,
        };

        // Get the first argument (the path string)
        let mut args_cursor = args_node.walk();
        let arg = args_node
            .children(&mut args_cursor)
            .find(|c| c.kind() == "string");

        let arg = match arg {
            Some(n) => n,
            None => continue,
        };

        let path = parsed.node_text(arg).to_string();
        if path.is_empty() {
            continue;
        }

        let line = call.start_position().row + 1;
        if let Some(&first_line) = seen.get(&path) {
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Resource {} is loaded multiple times (first on line {}).",
                        path, first_line
                    ),
                    line,
                )
                .span(
                    call.start_position().column,
                    call.end_position().row + 1,
                    call.end_position().column,
                ),
            );
        } else {
            seen.insert(path, line);
        }
    }
}
