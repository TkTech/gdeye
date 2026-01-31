use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/circular-preload";

pub struct CircularPreload;

impl Rule for CircularPreload {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Circular dependency detected in preload/load chain"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        // Build dependency graph from all files' preloads
        let graph = build_dependency_graph(ctx.all_file_symbols, ctx.path);

        // Only check for cycles involving the current file
        if let Some(cycles) = find_cycles_from(&graph, ctx.path) {
            for cycle in cycles {
                let cycle_str = cycle
                    .iter()
                    .map(|p| {
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(" → ");

                // Find the preload that starts the cycle in this file
                let line = ctx
                    .file_sym
                    .preloads
                    .iter()
                    .find(|p| {
                        cycle
                            .get(1)
                            .is_some_and(|next| res_path_matches(&p.res_path, next, ctx.path))
                    })
                    .map(|_| 1) // TODO: get actual line from preload
                    .unwrap_or(1);

                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!("Circular preload dependency: {}", cycle_str),
                        line,
                    )
                    .with_note(
                        "Circular dependencies can cause load failures or infinite loops at runtime.",
                    ),
                );
            }
        }

        diagnostics
    }
}

/// Build a dependency graph mapping each file path to its preload targets.
fn build_dependency_graph(
    all_file_symbols: &[crate::symbols::FileSymbols],
    current_path: &Path,
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut graph: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    // Build path lookup for res:// resolution
    let path_lookup: HashMap<String, PathBuf> = all_file_symbols
        .iter()
        .filter_map(|fs| {
            let filename = fs.path.file_name()?.to_string_lossy().to_string();
            Some((filename, fs.path.clone()))
        })
        .collect();

    for fs in all_file_symbols {
        let mut deps = Vec::new();
        for preload in &fs.preloads {
            // Try to resolve res:// path to actual file
            if let Some(resolved) = resolve_res_path(&preload.res_path, &path_lookup, current_path)
            {
                deps.push(resolved);
            }
        }
        if !deps.is_empty() {
            graph.insert(fs.path.clone(), deps);
        }
    }

    graph
}

/// Resolve a res:// path to an actual filesystem path.
fn resolve_res_path(
    res_path: &str,
    path_lookup: &HashMap<String, PathBuf>,
    _current_path: &Path,
) -> Option<PathBuf> {
    // Extract filename from res://path/to/file.gd
    let filename = res_path.rsplit('/').next()?;

    // Look up by filename (simple heuristic)
    path_lookup.get(filename).cloned()
}

/// Check if a res:// path matches a given filesystem path.
fn res_path_matches(res_path: &str, target: &Path, _current: &Path) -> bool {
    if let Some(filename) = res_path.rsplit('/').next() {
        if let Some(target_name) = target.file_name() {
            return filename == target_name.to_string_lossy();
        }
    }
    false
}

/// Find cycles in the dependency graph starting from a given path.
fn find_cycles_from(
    graph: &HashMap<PathBuf, Vec<PathBuf>>,
    start: &Path,
) -> Option<Vec<Vec<PathBuf>>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut path = Vec::new();

    fn dfs(
        node: &Path,
        graph: &HashMap<PathBuf, Vec<PathBuf>>,
        visited: &mut HashSet<PathBuf>,
        path: &mut Vec<PathBuf>,
        cycles: &mut Vec<Vec<PathBuf>>,
    ) {
        if path.contains(&node.to_path_buf()) {
            // Found a cycle - extract it
            if let Some(pos) = path.iter().position(|p| p == node) {
                let cycle: Vec<PathBuf> = path[pos..].to_vec();
                if !cycle.is_empty() {
                    let mut full_cycle = cycle;
                    full_cycle.push(node.to_path_buf()); // Close the cycle
                    cycles.push(full_cycle);
                }
            }
            return;
        }

        if visited.contains(&node.to_path_buf()) {
            return;
        }

        visited.insert(node.to_path_buf());
        path.push(node.to_path_buf());

        if let Some(deps) = graph.get(&node.to_path_buf()) {
            for dep in deps {
                dfs(dep, graph, visited, path, cycles);
            }
        }

        path.pop();
    }

    dfs(start, graph, &mut visited, &mut path, &mut cycles);

    if cycles.is_empty() {
        None
    } else {
        Some(cycles)
    }
}
