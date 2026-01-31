use std::path::Path;

use crate::classdb::ClassDb;
use crate::symbols::{FileSymbols, VarDecl};

/// Functions that conventionally have unused parameters (Godot callbacks).
pub(crate) const CALLBACK_FUNCTIONS: &[&str] = &[
    "_process",
    "_physics_process",
    "_input",
    "_unhandled_input",
    "_unhandled_key_input",
    "_ready",
    "_enter_tree",
    "_exit_tree",
    "_notification",
    "_draw",
    "_gui_input",
];

/// Check if `child` is a subclass of `parent` by walking user-defined class
/// inheritance chains (via class_name/extends in FileSymbols).
pub(crate) fn is_user_subclass_of(
    child: &str,
    parent: &str,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> bool {
    if child == parent {
        return true;
    }

    let mut current = child.to_string();
    let mut depth = 0;
    loop {
        depth += 1;
        if depth > 20 {
            return false; // Guard against cycles
        }

        // Find the user class with this class_name
        let file_sym = all_file_symbols
            .iter()
            .find(|fs| fs.class_name.as_deref() == Some(current.as_str()));

        let extends = match file_sym {
            Some(fs) => fs
                .extends
                .clone()
                .unwrap_or_else(|| "RefCounted".to_string()),
            None => return false, // Not a user class
        };

        if extends == parent {
            return true;
        }

        // Check if the extends class is a subclass of parent in the engine DB
        if class_db.is_subclass_of(&extends, parent) {
            return true;
        }

        // Continue walking up user-defined hierarchy
        current = extends;
    }
}

/// Check if a type name is an enum defined in any of the file symbols.
pub(crate) fn is_user_enum(type_name: &str, all_file_symbols: &[FileSymbols]) -> bool {
    all_file_symbols
        .iter()
        .any(|fs| fs.enums.iter().any(|e| e.name == type_name))
}

/// Compute the effective type of a variable, filtering out `:=` inference operator.
pub(crate) fn effective_var_type(v: &VarDecl) -> Option<String> {
    v.inferred_type
        .as_ref()
        .filter(|t| !t.is_empty() && *t != ":=")
        .cloned()
        .or_else(|| {
            v.type_annotation
                .as_ref()
                .filter(|a| !a.is_empty() && *a != ":=")
                .cloned()
        })
        .or_else(|| v.initializer_type.clone())
}

/// Check if a tree-sitter node kind represents a statement.
pub(crate) fn is_statement_node(kind: &str) -> bool {
    matches!(
        kind,
        "variable_statement"
            | "expression_statement"
            | "return_statement"
            | "break_statement"
            | "continue_statement"
            | "if_statement"
            | "for_statement"
            | "while_statement"
            | "match_statement"
            | "pass_statement"
            | "assert_statement"
    )
}

/// Check if a res:// resource path matches a filesystem script path.
pub(crate) fn resource_matches_path(
    res_path: &str,
    script_path: &Path,
    script_canonical: Option<&Path>,
    scene_path: &Path,
) -> bool {
    // Strip res:// prefix
    let relative = res_path.strip_prefix("res://").unwrap_or(res_path);

    // Try matching against the script's path suffix
    let script_str = script_path.to_string_lossy();
    if script_str.ends_with(relative) {
        return true;
    }

    // Try resolving relative to the scene file's project root
    if let Some(project_root) = find_project_root_from_scene(scene_path) {
        let resolved = project_root.join(relative);
        if let Some(canonical) = script_canonical {
            if let Ok(res_canonical) = resolved.canonicalize() {
                return canonical == res_canonical;
            }
        }
        // Fallback: compare paths directly
        if resolved == script_path {
            return true;
        }
    }

    false
}

/// Find the project root by searching upward from a scene file for project.godot.
fn find_project_root_from_scene(scene_path: &Path) -> Option<std::path::PathBuf> {
    let mut dir = scene_path.parent()?.to_path_buf();
    loop {
        if dir.join("project.godot").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
