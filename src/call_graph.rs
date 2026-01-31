//! Call graph construction and parameter type inference.
//!
//! This module builds a call graph from all parsed files and uses it to infer
//! parameter types from call site arguments.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::classdb::ClassDb;
use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;
use crate::types::{resolve_expr_type, ExprTypeContext};

/// A call site represents a single function call in the source code.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// File where the call occurs.
    #[allow(dead_code)] // Useful for debugging and future cross-file analysis
    pub caller_file: PathBuf,
    /// Function containing the call (None if at file level).
    #[allow(dead_code)] // Useful for debugging and call chain analysis
    pub caller_func: Option<String>,
    /// Line number of the call.
    #[allow(dead_code)] // Useful for diagnostics and debugging
    pub line: usize,
    /// Types of arguments passed at this call site.
    /// None if the type couldn't be resolved.
    pub arg_types: Vec<Option<String>>,
}

/// Information about a function that can be called.
#[derive(Debug, Clone)]
pub struct CallTarget {
    /// File where the function is defined.
    #[allow(dead_code)] // Useful for debugging
    pub file: PathBuf,
    /// Function name.
    #[allow(dead_code)] // Useful for debugging
    pub name: String,
    /// Number of parameters.
    #[allow(dead_code)] // May be used for arity checking in future
    pub param_count: usize,
    /// All call sites that invoke this function.
    pub call_sites: Vec<CallSite>,
}

/// The complete call graph for a project.
#[derive(Debug, Default)]
pub struct CallGraph {
    /// Map from (file_path, function_name) to call target info.
    pub targets: HashMap<(PathBuf, String), CallTarget>,
    /// Map from function name to all files that define it (for ambiguous calls).
    pub functions_by_name: HashMap<String, Vec<PathBuf>>,
    /// Forward edges: (caller_file, caller_func) -> set of (callee_file, callee_func).
    pub calls: HashMap<(PathBuf, String), HashSet<(PathBuf, String)>>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the call graph from all parsed files and their symbols.
    pub fn build(
        files: &[(PathBuf, ParsedFile)],
        all_symbols: &[FileSymbols],
        class_db: &ClassDb,
    ) -> Self {
        // Delegate to build_from_refs by converting owned to borrowed
        let refs: Vec<_> = files.iter().map(|(p, f)| (p.clone(), f)).collect();
        Self::build_from_refs(&refs, all_symbols, class_db)
    }

    /// Build the call graph from references to parsed files (avoids re-parsing).
    ///
    /// This is useful when you already have borrowed references to ParsedFile
    /// and don't want to re-parse them.
    pub fn build_from_refs(
        files: &[(PathBuf, &ParsedFile)],
        all_symbols: &[FileSymbols],
        class_db: &ClassDb,
    ) -> Self {
        let mut graph = CallGraph::new();

        // First pass: register all function definitions as call targets
        for (file_idx, symbols) in all_symbols.iter().enumerate() {
            let file_path = &files[file_idx].0;

            for func in &symbols.functions {
                let key = (file_path.clone(), func.name.clone());
                graph.targets.insert(
                    key,
                    CallTarget {
                        file: file_path.clone(),
                        name: func.name.clone(),
                        param_count: func.parameters.len(),
                        call_sites: Vec::new(),
                    },
                );

                // Track function names for lookup
                graph
                    .functions_by_name
                    .entry(func.name.clone())
                    .or_default()
                    .push(file_path.clone());
            }
        }

        // Second pass: find all call sites and resolve argument types
        for (file_idx, (file_path, parsed)) in files.iter().enumerate() {
            let symbols = &all_symbols[file_idx];
            collect_calls_from_file(file_path, parsed, symbols, class_db, &mut graph);
        }

        graph
    }

    /// Get all call sites for a function in a specific file.
    pub fn get_call_sites(&self, file: &Path, func_name: &str) -> Option<&[CallSite]> {
        self.targets
            .get(&(file.to_path_buf(), func_name.to_string()))
            .map(|t| t.call_sites.as_slice())
    }

    /// Compute functions reachable from entry points via forward traversal.
    ///
    /// Returns a set of (file_path, function_name) pairs for all functions
    /// that can be reached by following call edges from the entry points.
    pub fn compute_reachability(
        &self,
        entry_points: &HashSet<(PathBuf, String)>,
    ) -> HashSet<(PathBuf, String)> {
        let mut reachable = HashSet::new();
        let mut worklist: Vec<_> = entry_points.iter().cloned().collect();

        while let Some(current) = worklist.pop() {
            if !reachable.insert(current.clone()) {
                continue; // Already visited
            }
            // Add all functions called by current
            if let Some(callees) = self.calls.get(&current) {
                for callee in callees {
                    if !reachable.contains(callee) {
                        worklist.push(callee.clone());
                    }
                }
            }
        }
        reachable
    }
}

/// Collect all function calls from a single file.
fn collect_calls_from_file(
    file_path: &Path,
    parsed: &ParsedFile,
    symbols: &FileSymbols,
    class_db: &ClassDb,
    graph: &mut CallGraph,
) {
    let root = parsed.root_node();

    // Find all call nodes
    let calls = parser::find_nodes_by_kind(root, "call");

    for call_node in calls {
        // Get the function name being called
        let func_name = match call_node.child(0) {
            Some(n) if n.kind() == "identifier" || n.kind() == "name" => {
                parsed.node_text(n).to_string()
            }
            _ => continue, // Skip method calls for now (obj.method())
        };

        // Skip built-in utility functions (print, sin, etc.)
        if class_db.is_utility_function(&func_name) {
            continue;
        }

        // Find the containing function (caller)
        let caller_func = find_containing_function(call_node, parsed);

        // Get argument types
        let arg_types = resolve_call_arg_types(call_node, parsed, symbols, class_db);

        let call_site = CallSite {
            caller_file: file_path.to_path_buf(),
            caller_func,
            line: call_node.start_position().row + 1,
            arg_types,
        };

        // Find the target function - first check same file, then by name
        let target_key = if graph
            .targets
            .contains_key(&(file_path.to_path_buf(), func_name.clone()))
        {
            Some((file_path.to_path_buf(), func_name.clone()))
        } else if let Some(defining_files) = graph.functions_by_name.get(&func_name) {
            // For now, if there's exactly one definition, use it
            // TODO: Use class hierarchy to resolve ambiguous calls
            if defining_files.len() == 1 {
                Some((defining_files[0].clone(), func_name.clone()))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(key) = target_key {
            if let Some(target) = graph.targets.get_mut(&key) {
                target.call_sites.push(call_site.clone());
            }

            // Record forward edge if caller is known
            if let Some(ref caller_name) = call_site.caller_func {
                let caller_key = (file_path.to_path_buf(), caller_name.clone());
                graph.calls.entry(caller_key).or_default().insert(key);
            }
        }
    }
}

/// Find the function containing a node, if any.
fn find_containing_function(node: tree_sitter::Node, parsed: &ParsedFile) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "function_definition" || parent.kind() == "constructor_definition" {
            return parent
                .child_by_field_name("name")
                .map(|n| parsed.node_text(n).to_string());
        }
        current = parent.parent();
    }
    None
}

/// Resolve the types of arguments in a function call.
fn resolve_call_arg_types(
    call_node: tree_sitter::Node,
    parsed: &ParsedFile,
    symbols: &FileSymbols,
    class_db: &ClassDb,
) -> Vec<Option<String>> {
    let mut arg_types = Vec::new();

    // Find the arguments node
    let args_node = call_node
        .children(&mut call_node.walk())
        .find(|c| c.kind() == "arguments");

    let args_node = match args_node {
        Some(n) => n,
        None => return arg_types,
    };

    // Find the containing function for context
    let containing_func = find_containing_function(call_node, parsed);
    let func_decl = containing_func
        .as_ref()
        .and_then(|name| symbols.functions.iter().find(|f| &f.name == name));

    // Build local vars list
    let local_vars: Vec<(String, Option<String>)> = func_decl
        .map(|f| {
            f.local_vars
                .iter()
                .map(|v| (v.name.clone(), v.inferred_type.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Build params list
    let params: Vec<(String, Option<String>)> = func_decl
        .map(|f| {
            f.parameters
                .iter()
                .map(|p| (p.name.clone(), p.type_annotation.clone()))
                .collect()
        })
        .unwrap_or_default();

    // Build member vars list
    let member_vars: Vec<(String, Option<String>)> = symbols
        .variables
        .iter()
        .filter(|v| matches!(v.scope, crate::symbols::Scope::File))
        .map(|v| {
            let effective_type = v
                .type_annotation
                .clone()
                .or_else(|| v.inferred_type.clone())
                .or_else(|| v.initializer_type.clone());
            (v.name.clone(), effective_type)
        })
        .collect();

    // Build local function returns
    let local_func_returns: Vec<(String, Option<String>)> = symbols
        .functions
        .iter()
        .map(|f| {
            let ret = f
                .return_type
                .clone()
                .or_else(|| f.inferred_return_type.clone());
            (f.name.clone(), ret)
        })
        .collect();

    // Get extends class
    let extends_class = symbols.extends.as_deref().unwrap_or("");

    let ctx = ExprTypeContext {
        extends_class,
        local_vars: &local_vars,
        params: &params,
        member_vars: &member_vars,
        local_func_returns: &local_func_returns,
        class_db,
        type_refinements: None,
    };

    // Iterate over argument expressions
    let mut cursor = args_node.walk();
    for child in args_node.children(&mut cursor) {
        if !child.is_named() || child.kind() == "comment" {
            continue;
        }

        // Use the type resolver to get the argument type
        let arg_type = resolve_expr_type(child, parsed, &ctx);
        arg_types.push(arg_type);
    }

    arg_types
}

/// Infer parameter types from call sites and update FileSymbols.
pub fn infer_parameter_types(
    all_symbols: &mut [FileSymbols],
    files: &[(PathBuf, ParsedFile)],
    call_graph: &CallGraph,
    class_db: &ClassDb,
) {
    for (file_idx, symbols) in all_symbols.iter_mut().enumerate() {
        let file_path = &files[file_idx].0;

        for func in &mut symbols.functions {
            let call_sites = match call_graph.get_call_sites(file_path, &func.name) {
                Some(sites) if !sites.is_empty() => sites,
                _ => continue,
            };

            // For each parameter position, collect all argument types from call sites
            for (param_idx, param) in func.parameters.iter_mut().enumerate() {
                // Skip if already has explicit type annotation
                if param.type_annotation.is_some() {
                    continue;
                }

                // Collect all types for this parameter position
                let mut types: Vec<&str> = Vec::new();
                for site in call_sites {
                    if let Some(Some(arg_type)) = site.arg_types.get(param_idx) {
                        types.push(arg_type.as_str());
                    }
                }

                if types.is_empty() {
                    continue;
                }

                // Unify types
                let unified = unify_types(&types, class_db);
                if let Some(unified_type) = unified {
                    param.inferred_type = Some(unified_type);
                }
            }
        }
    }
}

/// Unify multiple types into a single type, if compatible.
fn unify_types(types: &[&str], class_db: &ClassDb) -> Option<String> {
    if types.is_empty() {
        return None;
    }

    // Check if all types are the same
    let first = types[0];
    if types.iter().all(|t| *t == first) {
        return Some(first.to_string());
    }

    // Handle numeric promotion: int + float -> float
    let has_int = types.contains(&"int");
    let has_float = types.contains(&"float");
    let all_numeric = types.iter().all(|t| *t == "int" || *t == "float");

    if all_numeric && has_int && has_float {
        return Some("float".to_string());
    }

    // Try finding common base class using ClassDB
    if let Some(common) = class_db.common_ancestor_of_all(types) {
        // Only use if more specific than Object/RefCounted (too general to be useful)
        if common != "Object" && common != "RefCounted" {
            return Some(common.to_string());
        }
    }

    None
}

/// Collect entry points (functions considered externally reachable).
///
/// Entry points include:
/// - Functions starting with `_` (callbacks/overrides)
/// - Functions marked as `used` by cross-file analysis
/// - Functions in files with a class_name (exported classes)
/// - RPC-annotated functions
pub fn collect_entry_points(
    all_symbols: &[FileSymbols],
    files: &[(PathBuf, ParsedFile)],
) -> HashSet<(PathBuf, String)> {
    // Delegate to collect_entry_points_from_refs by converting owned to borrowed
    let refs: Vec<_> = files.iter().map(|(p, f)| (p.clone(), f)).collect();
    collect_entry_points_from_refs(all_symbols, &refs)
}

/// Collect entry points from borrowed ParsedFile references (avoids re-parsing).
pub fn collect_entry_points_from_refs(
    all_symbols: &[FileSymbols],
    files: &[(PathBuf, &ParsedFile)],
) -> HashSet<(PathBuf, String)> {
    let mut entry_points = HashSet::new();

    for (file_idx, symbols) in all_symbols.iter().enumerate() {
        let file_path = &files[file_idx].0;
        let source = files[file_idx].1.source();
        let rpc_functions = collect_rpc_functions(source);

        for func in &symbols.functions {
            let is_entry = func.name.starts_with('_')    // Callbacks/overrides
                || func.used                              // Marked by cross-file analysis
                || symbols.class_name.is_some()           // Exported class
                || rpc_functions.contains(&func.name.as_str()); // RPC functions

            if is_entry {
                entry_points.insert((file_path.clone(), func.name.clone()));
            }
        }
    }
    entry_points
}

/// Collect function names that are annotated with @rpc.
fn collect_rpc_functions(source: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("@rpc") {
            // Look for the next `func` declaration
            for next_line in &lines[i + 1..] {
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with("func ") {
                    if let Some(name) = next_trimmed
                        .strip_prefix("func ")
                        .and_then(|s| s.split('(').next())
                    {
                        result.push(name.trim());
                    }
                    break;
                }
                // Skip empty lines and comments between @rpc and func
                if !next_trimmed.is_empty()
                    && !next_trimmed.starts_with('#')
                    && !next_trimmed.starts_with('@')
                {
                    break;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_same_types() {
        let db = ClassDb::empty();
        assert_eq!(
            unify_types(&["int", "int", "int"], &db),
            Some("int".to_string())
        );
        assert_eq!(
            unify_types(&["String", "String"], &db),
            Some("String".to_string())
        );
    }

    #[test]
    fn unify_int_float() {
        let db = ClassDb::empty();
        assert_eq!(
            unify_types(&["int", "float"], &db),
            Some("float".to_string())
        );
        assert_eq!(
            unify_types(&["float", "int", "int"], &db),
            Some("float".to_string())
        );
    }

    #[test]
    fn unify_incompatible() {
        let db = ClassDb::empty();
        assert_eq!(unify_types(&["int", "String"], &db), None);
    }

    #[test]
    fn unify_empty() {
        let db = ClassDb::empty();
        assert_eq!(unify_types(&[], &db), None);
    }

    #[test]
    fn unify_with_common_ancestor() {
        // Load bundled ClassDB to test inheritance-based unification
        let db = ClassDb::from_bundled(None).expect("Failed to load bundled classdb");

        // Button and Label both inherit from Control
        assert_eq!(
            unify_types(&["Button", "Label"], &db),
            Some("Control".to_string())
        );

        // Node2D and Node3D both inherit from Node
        assert_eq!(
            unify_types(&["Node2D", "Node3D"], &db),
            Some("Node".to_string())
        );

        // Sprite2D and Label both inherit from CanvasItem
        assert_eq!(
            unify_types(&["Sprite2D", "Label"], &db),
            Some("CanvasItem".to_string())
        );
    }

    #[test]
    fn unify_returns_none_for_object_ancestor() {
        let db = ClassDb::from_bundled(None).expect("Failed to load bundled classdb");

        // Node and Resource both ultimately inherit from Object, but that's too general
        assert_eq!(unify_types(&["Node", "Resource"], &db), None);
    }
}
