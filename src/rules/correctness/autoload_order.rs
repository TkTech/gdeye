use std::collections::HashSet;
use std::path::Path;

use indexmap::IndexMap;
use tree_sitter::Node;

use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/autoload-order";

pub struct AutoloadOrder;

impl Rule for AutoloadOrder {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Autoload references another autoload that loads later"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Skip if no autoloads defined
        if ctx.project_info.autoloads.is_empty() {
            return diagnostics;
        }

        // Check if current file is an autoload
        let current_autoload = find_autoload_name_for_path(
            ctx.path,
            &ctx.project_info.autoloads,
            ctx.all_file_symbols,
        );

        if current_autoload.is_none() {
            return diagnostics;
        }
        let current_autoload = current_autoload.unwrap();

        // Build autoload order (IndexMap preserves insertion order)
        let autoload_order: Vec<&String> = ctx.project_info.autoloads.keys().collect();

        let current_index = match autoload_order.iter().position(|&n| n == &current_autoload) {
            Some(i) => i,
            None => return diagnostics,
        };

        // Find all autoload references in this file
        check_autoload_references(
            ctx.parsed,
            &current_autoload,
            current_index,
            &autoload_order,
            &mut diagnostics,
        );

        diagnostics
    }
}

/// Find the autoload name for a given file path.
fn find_autoload_name_for_path(
    path: &Path,
    autoloads: &IndexMap<String, String>,
    all_file_symbols: &[FileSymbols],
) -> Option<String> {
    // First try to match by filename
    let filename = path.file_name()?.to_string_lossy();

    for (name, res_path) in autoloads {
        // res://scripts/GameManager.gd -> GameManager.gd
        if let Some(autoload_filename) = res_path.rsplit('/').next() {
            if autoload_filename == filename {
                return Some(name.clone());
            }
        }
    }

    // Try to match by class_name
    let file_sym = all_file_symbols.iter().find(|fs| fs.path == path)?;
    if let Some(ref class_name) = file_sym.class_name {
        if autoloads.contains_key(class_name) {
            return Some(class_name.clone());
        }
    }

    None
}

/// Check for references to autoloads that load after the current one.
fn check_autoload_references(
    parsed: &ParsedFile,
    current_autoload: &str,
    current_index: usize,
    autoload_order: &[&String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();

    // Track which dependencies we've already warned about to avoid noise
    let mut warned_dependencies: HashSet<String> = HashSet::new();

    // Find all identifier references
    let identifiers = parser::find_nodes_by_kind(root, "identifier");

    for ident in identifiers {
        let name = parsed.node_text(ident);

        // Skip self-reference
        if name == current_autoload {
            continue;
        }

        // Skip if we've already warned about this dependency
        if warned_dependencies.contains(name) {
            continue;
        }

        // Check if this is a reference to an autoload
        if let Some(ref_index) = autoload_order.iter().position(|&n| n == name) {
            // Check if the referenced autoload loads after current
            if ref_index > current_index {
                // Check if this is in a function body (not just a type annotation)
                if is_runtime_reference(ident) {
                    warned_dependencies.insert(name.to_string());
                    let start = ident.start_position();
                    let end = ident.end_position();
                    diagnostics.push(
                        Diagnostic::new(
                            RULE_ID,
                            Severity::Warning,
                            format!(
                                "Autoload `{}` references `{}` which loads later in project settings.",
                                current_autoload, name
                            ),
                            start.row + 1,
                        )
                        .span(start.column, end.row + 1, end.column)
                        .with_note(
                            "This may cause null reference errors at startup. Reorder autoloads in Project Settings.",
                        ),
                    );
                }
            }
        }
    }
}

/// Check if an identifier is a runtime reference (not a type annotation or preload arg).
fn is_runtime_reference(node: Node) -> bool {
    let mut current = node;
    while let Some(parent) = current.parent() {
        match parent.kind() {
            // Skip type annotations
            "type" | "return_type" | "type_cast" => return false,
            // Skip string arguments (preload paths)
            "string" => return false,
            // Found a statement context - it's a runtime reference
            "expression_statement"
            | "assignment"
            | "augmented_assignment"
            | "call"
            | "attribute"
            | "subscript"
            | "if_statement"
            | "while_statement"
            | "for_statement"
            | "return_statement" => return true,
            _ => {}
        }
        current = parent;
    }
    false
}
