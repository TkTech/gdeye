use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::classdb::ClassDb;
use crate::scene::{SceneFile, SceneNode};
use crate::symbols::FileSymbols;

use super::super::helpers::resource_matches_path;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/signal-signature-mismatch";

pub struct SignalSignatureMismatch;

impl Rule for SignalSignatureMismatch {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Signal handler has fewer parameters than the signal passes"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_signal_signature_mismatch(
            ctx.path,
            ctx.file_sym,
            ctx.all_file_symbols,
            ctx.scenes,
            ctx.class_db,
            &mut diagnostics,
        );
        diagnostics
    }
}

/// Check that signal handlers have the correct number of parameters.
fn check_signal_signature_mismatch(
    path: &Path,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    scenes: &HashMap<PathBuf, SceneFile>,
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find scenes where this file's script is attached to a node
    for scene in scenes.values() {
        // Find the node(s) that have this file's script
        let script_nodes: Vec<&SceneNode> = scene
            .nodes
            .iter()
            .filter(|node| {
                if let Some(ref sid) = node.script_id {
                    if let Some(ext) = scene.ext_resources.iter().find(|e| e.id == *sid) {
                        return resource_matches_path(&ext.path, path, None, &scene.path);
                    }
                }
                false
            })
            .collect();

        if script_nodes.is_empty() {
            continue;
        }

        for script_node in &script_nodes {
            // Compute the connection path for this node
            let conn_path = if script_node.parent.is_empty() {
                ".".to_string()
            } else {
                script_node.node_path.clone()
            };

            // Find connections where to_node == this node (handler is here)
            for conn in &scene.connections {
                if conn.to_node != conn_path {
                    continue;
                }

                // Find the handler function in this file
                let handler = match file_sym.functions.iter().find(|f| f.name == conn.method) {
                    Some(f) => f,
                    None => continue,
                };

                // Resolve the signal parameter count
                let signal_param_count = resolve_signal_param_count(
                    &conn.signal,
                    &conn.from_node,
                    scene,
                    all_file_symbols,
                    class_db,
                );

                let expected = match signal_param_count {
                    Some(count) => count,
                    None => continue, // Can't resolve signal, skip
                };

                let actual = handler.parameters.len();
                if actual < expected {
                    diagnostics.push(
                        Diagnostic::new(
                            RULE_ID,
                            Severity::Warning,
                            format!(
                            "Signal `{}` passes {} argument{} but handler `{}` accepts only {}.",
                            conn.signal,
                            expected,
                            if expected == 1 { "" } else { "s" },
                            conn.method,
                            actual,
                        ),
                            handler.line,
                        )
                        .span(0, handler.line, handler.name.len() + 5),
                    );
                }
            }
        }
    }
}

/// Resolve the parameter count of a signal from its source node.
fn resolve_signal_param_count(
    signal_name: &str,
    from_node_path: &str,
    scene: &SceneFile,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> Option<usize> {
    // Find the from_node in the scene
    let from_node = if from_node_path == "." {
        scene.nodes.iter().find(|n| n.parent.is_empty())
    } else {
        scene.nodes.iter().find(|n| n.node_path == from_node_path)
    }?;

    // First, check if the node has a user script with this signal declared
    if let Some(ref sid) = from_node.script_id {
        if let Some(ext) = scene.ext_resources.iter().find(|e| e.id == *sid) {
            // Find the file symbols for this script
            for file_sym in all_file_symbols {
                if resource_matches_path(&ext.path, &file_sym.path, None, &scene.path) {
                    if let Some(sig) = file_sym.signals.iter().find(|s| s.name == signal_name) {
                        return Some(sig.parameters.len());
                    }
                    break;
                }
            }
        }
    }

    // Fall back to ClassDB for built-in signals
    if !from_node.node_type.is_empty() {
        if let Some(sig) = class_db.get_signal(&from_node.node_type, signal_name) {
            return Some(sig.arguments.len());
        }
    }

    None
}
