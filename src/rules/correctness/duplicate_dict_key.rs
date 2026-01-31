use std::collections::HashMap;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/duplicate-dict-key";

pub struct DuplicateDictKey;

impl Rule for DuplicateDictKey {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Dictionary contains duplicate keys"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_duplicate_dict_keys(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

fn check_duplicate_dict_keys(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let dicts = parser::find_nodes_by_kind(root, "dictionary");

    for dict in dicts {
        let mut seen: HashMap<String, usize> = HashMap::new();
        let mut cursor = dict.walk();

        for child in dict.children(&mut cursor) {
            if child.kind() != "pair" {
                continue;
            }

            let mut pair_cursor = child.walk();
            let key_node = match child.children(&mut pair_cursor).next() {
                Some(n) => n,
                None => continue,
            };

            // Only compare simple literals and identifiers
            let kind = key_node.kind();
            if !matches!(kind, "string" | "integer" | "identifier") {
                continue;
            }

            let key_text = parsed.node_text(key_node).to_string();
            if key_text.is_empty() {
                continue;
            }

            let line = key_node.start_position().row + 1;
            if let Some(&first_line) = seen.get(&key_text) {
                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!(
                            "Duplicate dictionary key `{}` (first defined on line {}).",
                            key_text, first_line
                        ),
                        line,
                    )
                    .span(
                        key_node.start_position().column,
                        key_node.end_position().row + 1,
                        key_node.end_position().column,
                    ),
                );
            } else {
                seen.insert(key_text, line);
            }
        }
    }
}
