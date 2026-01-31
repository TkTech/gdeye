use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::parser::ParsedFile;
use crate::scene::{SceneFile, SceneNode};

use super::super::helpers::resource_matches_path;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/broken-node-path";

pub struct BrokenNodePath;

impl Rule for BrokenNodePath {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Node path does not match any node in the attached scene tree"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_broken_node_paths(ctx.path, ctx.parsed, ctx.scenes, &mut diagnostics);
        diagnostics
    }
}

fn check_broken_node_paths(
    script_path: &Path,
    parsed: &ParsedFile,
    scenes: &HashMap<PathBuf, SceneFile>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if scenes.is_empty() {
        return;
    }

    // Find scenes that attach this script and determine the node path context
    let contexts = find_script_scene_contexts(script_path, scenes);
    if contexts.is_empty() {
        return;
    }

    // Collect all node path references from the AST
    let root = parsed.root_node();
    let mut refs = Vec::new();
    collect_node_path_refs(root, parsed, &mut refs);

    for node_ref in &refs {
        // Skip paths we can't statically validate
        if node_ref.path.contains("..")
            || node_ref.path.starts_with('/')
            || node_ref.path.starts_with('%')
            || node_ref.path.is_empty()
        {
            continue;
        }

        // Check against all scene contexts — only flag if broken in ALL scenes
        let valid_in_any = contexts
            .iter()
            .any(|ctx| ctx.valid_paths.contains(&node_ref.path));

        if !valid_in_any {
            diagnostics.push(
                Diagnostic::new(
                    RULE_ID,
                    Severity::Warning,
                    format!(
                        "Node path `{}` does not match any node in the scene tree.",
                        node_ref.path
                    ),
                    node_ref.line,
                )
                .span(node_ref.col, node_ref.line, node_ref.end_col),
            );
        }
    }
}

struct SceneContext {
    /// Set of valid relative node paths from the script's node
    valid_paths: HashSet<String>,
}

/// Find all scenes where this script is attached and build the valid path sets.
fn find_script_scene_contexts(
    script_path: &Path,
    scenes: &HashMap<PathBuf, SceneFile>,
) -> Vec<SceneContext> {
    let mut contexts = Vec::new();

    // Canonicalize the script path for comparison
    let script_canonical = script_path.canonicalize().ok();

    for scene in scenes.values() {
        // Find ext_resource entries that point to this script
        let script_res_ids: Vec<&str> = scene
            .ext_resources
            .iter()
            .filter(|r| {
                r.resource_type == "Script"
                    && resource_matches_path(
                        &r.path,
                        script_path,
                        script_canonical.as_deref(),
                        &scene.path,
                    )
            })
            .map(|r| r.id.as_str())
            .collect();

        if script_res_ids.is_empty() {
            continue;
        }

        // Find nodes that have this script attached
        for node in &scene.nodes {
            if let Some(ref sid) = node.script_id {
                if script_res_ids.contains(&sid.as_str()) {
                    // Build the set of valid descendant paths relative to this node
                    let valid_paths = build_valid_paths(&node.node_path, &scene.nodes);
                    contexts.push(SceneContext { valid_paths });
                }
            }
        }
    }

    contexts
}

/// Build the set of valid node paths relative to a given node.
fn build_valid_paths(script_node_path: &str, all_nodes: &[SceneNode]) -> HashSet<String> {
    let mut paths = HashSet::new();

    // Determine if the script is on the root node.
    let is_root = all_nodes
        .iter()
        .any(|n| n.node_path == script_node_path && n.parent.is_empty());

    if is_root {
        // Script is on root: all other nodes are valid, using their node_path directly
        for node in all_nodes {
            if node.node_path != script_node_path && !node.node_path.is_empty() {
                paths.insert(node.node_path.clone());
            }
        }
    } else {
        // Script is on a non-root node: find descendants by parent chain
        for node in all_nodes {
            if node.node_path == script_node_path {
                continue;
            }
            let is_descendant = node.parent == script_node_path
                || node.parent.starts_with(&format!("{}/", script_node_path));

            if is_descendant {
                // Compute relative path by stripping the script node path prefix
                let prefix = format!("{}/", script_node_path);
                let relative = if node.node_path.starts_with(&prefix) {
                    node.node_path[prefix.len()..].to_string()
                } else {
                    node.node_path.clone()
                };
                if !relative.is_empty() {
                    paths.insert(relative);
                }
            }
        }
    }

    paths
}

struct NodePathRef {
    path: String,
    line: usize,
    col: usize,
    end_col: usize,
}

/// Recursively collect all node path references from the AST.
fn collect_node_path_refs(node: Node, parsed: &ParsedFile, refs: &mut Vec<NodePathRef>) {
    match node.kind() {
        "get_node" => {
            // $NodePath or $"NodePath" syntax
            let text = parsed.node_text(node);
            let path = extract_dollar_path(text);
            if !path.is_empty() {
                refs.push(NodePathRef {
                    path,
                    line: node.start_position().row + 1,
                    col: node.start_position().column,
                    end_col: node.end_position().column,
                });
            }
        }
        "call" => {
            // get_node("path") or get_node_or_null("path")
            if let Some(func) = node.child(0) {
                let func_name = parsed.node_text(func);
                if func_name == "get_node" || func_name == "get_node_or_null" {
                    // Find the arguments node (may not be a named field)
                    let mut call_cursor = node.walk();
                    for call_child in node.children(&mut call_cursor) {
                        if call_child.kind() == "arguments" {
                            let mut arg_cursor = call_child.walk();
                            for arg in call_child.children(&mut arg_cursor) {
                                if arg.kind() == "string" {
                                    let path = unquote_string(parsed.node_text(arg));
                                    if !path.is_empty() {
                                        refs.push(NodePathRef {
                                            path,
                                            line: node.start_position().row + 1,
                                            col: node.start_position().column,
                                            end_col: node.end_position().column,
                                        });
                                    }
                                    break;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_path_refs(child, parsed, refs);
    }
}

/// Extract the node path from `$NodePath` or `$"NodePath"` syntax.
fn extract_dollar_path(text: &str) -> String {
    let path = text.strip_prefix('$').unwrap_or(text);
    // Handle quoted form: $"Some/Path"
    if path.starts_with('"') && path.ends_with('"') {
        path[1..path.len() - 1].to_string()
    } else {
        path.to_string()
    }
}

/// Remove surrounding quotes from a string literal.
fn unquote_string(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
