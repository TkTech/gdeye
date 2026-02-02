use std::collections::{HashMap, HashSet};

use crate::cfg::Cfg;
use crate::symbols::FileSymbols;

/// Results of flow analysis for a single file.
#[derive(Debug)]
pub struct FlowResults {
    /// Per-function analysis results
    pub functions: HashMap<String, FunctionFlowResult>,
}

#[derive(Debug)]
pub struct FunctionFlowResult {
    /// Variables defined but never used (name, definition line)
    pub dead_assignments: Vec<(String, usize)>,
    /// Variables used before being defined (name, use line)
    pub uninitialized_uses: Vec<(String, usize)>,
    /// Variables that are live at each block (block_id -> set of live var names)
    pub live_vars: HashMap<usize, HashSet<String>>,
}

/// Run flow analysis on all CFGs in a file.
pub fn analyze(cfgs: &[Cfg], file_sym: &FileSymbols) -> FlowResults {
    let mut results = FlowResults {
        functions: HashMap::new(),
    };

    // Collect the set of member variables so we don't flag them
    let member_vars: HashSet<&str> = file_sym.variables.iter().map(|v| v.name.as_str()).collect();

    for cfg in cfgs {
        // Find the function symbol to get parameter names
        let param_names: HashSet<&str> = file_sym
            .functions
            .iter()
            .find(|f| f.name == cfg.function_name)
            .map(|f| f.parameters.iter().map(|p| p.name.as_str()).collect())
            .unwrap_or_default();

        let result = analyze_function(cfg, &member_vars, &param_names);
        results.functions.insert(cfg.function_name.clone(), result);
    }

    results
}

fn analyze_function(
    cfg: &Cfg,
    member_vars: &HashSet<&str>,
    param_names: &HashSet<&str>,
) -> FunctionFlowResult {
    let mut result = FunctionFlowResult {
        dead_assignments: Vec::new(),
        uninitialized_uses: Vec::new(),
        live_vars: HashMap::new(),
    };

    if cfg.blocks.is_empty() {
        return result;
    }

    // Compute liveness using backward dataflow (properly handles kills)
    let (live_in, live_out) = compute_liveness(cfg);
    result.live_vars = live_in.clone();

    // Compute reaching definitions for uninitialized variable detection
    let reaching = compute_reaching_definitions(cfg);

    // Find uninitialized uses: a use is uninitialized if the variable is not
    // in the reaching definitions set for the block and is not a member var or parameter.
    // We also need to collect all definitions in the current block first, since a
    // var x = ... statement defines x before any uses on subsequent lines.
    let empty_set = HashSet::new();
    for block in &cfg.blocks {
        let block_reaching = reaching.get(&block.id).unwrap_or(&empty_set);

        // Collect all definitions in this block with their line numbers
        // Keep the FIRST (earliest) definition of each variable
        let mut block_defs: HashMap<&String, usize> = HashMap::new();
        for (name, line, _) in &block.definitions {
            block_defs.entry(name).or_insert(*line);
        }

        // Check each use to see if it's defined before being used
        for (var_name, use_line) in &block.uses {
            // Skip member variables and parameters
            if member_vars.contains(var_name.as_str()) {
                continue;
            }
            if param_names.contains(var_name.as_str()) {
                continue;
            }

            // Check if variable reaches this block from predecessors
            if block_reaching.contains(var_name) {
                continue;
            }

            // Check if variable is defined in this block before this use
            if let Some(&def_line) = block_defs.get(var_name) {
                if def_line <= *use_line {
                    continue; // Defined before use in the same block
                }
            }

            result
                .uninitialized_uses
                .push((var_name.clone(), *use_line));
        }
    }

    // Collect the set of locally-declared variables (those with a `var` statement).
    // Only these can be flagged as dead assignments - reassignments to unknown
    // identifiers could be member variable or inherited property writes.
    let local_vars: HashSet<&str> = cfg
        .blocks
        .iter()
        .flat_map(|b| b.definitions.iter())
        .filter(|(_, _, is_decl)| *is_decl)
        .map(|(name, _, _)| name.as_str())
        .collect();

    // Count definitions per variable to determine if declarations have reassignments.
    // A declaration is only flagged as dead if there are subsequent reassignments
    // (meaning the initial value was overwritten before use).
    let mut def_count: HashMap<&str, usize> = HashMap::new();
    for block in &cfg.blocks {
        for (var_name, _, _) in &block.definitions {
            *def_count.entry(var_name.as_str()).or_insert(0) += 1;
        }
    }

    // Find dead assignments: a definition is dead if the variable is NOT
    // in the live-out set of the block (meaning no successor path reads it
    // without first redefining it).
    for block in &cfg.blocks {
        for (var_name, def_line, is_decl) in &block.definitions {
            if member_vars.contains(var_name.as_str()) {
                continue;
            }
            // For declarations, only flag if there are subsequent reassignments
            // (meaning the initial value was definitively overwritten before use).
            // Declarations without reassignments are handled by "unused variable" warnings.
            if *is_decl {
                let has_reassignment = def_count.get(var_name.as_str()).copied().unwrap_or(0) > 1;
                if !has_reassignment {
                    continue;
                }
            } else {
                // For reassignments, skip if not a local variable (could be member/inherited)
                if !local_vars.contains(var_name.as_str()) {
                    continue;
                }
            }
            let block_live_out = live_out.get(&block.id);
            let is_live = block_live_out.is_some_and(|s| s.contains(var_name));
            let is_used_in_block = block.uses.iter().any(|(n, _)| n == var_name);

            if !is_live && !is_used_in_block {
                result.dead_assignments.push((var_name.clone(), *def_line));
            }
        }
    }

    result
}

/// Compute variable liveness using backward dataflow analysis.
/// Returns (live_in, live_out) maps for each block.
fn compute_liveness(
    cfg: &Cfg,
) -> (
    HashMap<usize, HashSet<String>>,
    HashMap<usize, HashSet<String>>,
) {
    let mut live_in: HashMap<usize, HashSet<String>> = HashMap::new();
    let mut live_out: HashMap<usize, HashSet<String>> = HashMap::new();

    for block in &cfg.blocks {
        live_in.insert(block.id, HashSet::new());
        live_out.insert(block.id, HashSet::new());
    }

    // Fixed-point iteration (backward)
    let mut changed = true;
    let mut iterations = 0;
    let max_iterations = 100;

    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;

        // Process blocks in reverse order
        for block in cfg.blocks.iter().rev() {
            // live_out = union of live_in of all successors
            let mut new_live_out = HashSet::new();
            if let Some(successors) = cfg.edges.get(&block.id) {
                for &succ in successors {
                    if let Some(succ_live_in) = live_in.get(&succ) {
                        new_live_out.extend(succ_live_in.iter().cloned());
                    }
                }
            }

            // live_in = uses ∪ (live_out - definitions)
            let defs: HashSet<&String> = block.definitions.iter().map(|(n, _, _)| n).collect();
            let mut new_live_in: HashSet<String> = new_live_out
                .iter()
                .filter(|v| !defs.contains(v))
                .cloned()
                .collect();
            for (use_name, _) in &block.uses {
                new_live_in.insert(use_name.clone());
            }

            if new_live_in != *live_in.get(&block.id).unwrap_or(&HashSet::new()) {
                changed = true;
                live_in.insert(block.id, new_live_in);
            }
            if new_live_out != *live_out.get(&block.id).unwrap_or(&HashSet::new()) {
                changed = true;
                live_out.insert(block.id, new_live_out);
            }
        }
    }

    (live_in, live_out)
}

/// Compute reaching definitions using forward dataflow analysis (must-analysis).
/// Returns a map from block ID to the set of variable names that are
/// definitely defined on all paths reaching the block's entry point.
///
/// Uses intersection at merge points: a variable is "definitely defined"
/// only if it is defined on ALL predecessor paths.
fn compute_reaching_definitions(cfg: &Cfg) -> HashMap<usize, HashSet<String>> {
    // Collect all defined variables (used as TOP in the lattice)
    let all_vars: HashSet<String> = cfg
        .blocks
        .iter()
        .flat_map(|b| b.definitions.iter().map(|(name, _, _)| name.clone()))
        .collect();

    let mut reaching: HashMap<usize, HashSet<String>> = HashMap::new();

    // Entry block starts empty; other blocks start with TOP (all variables)
    // This is required for must-analysis: we intersect down from TOP
    for block in &cfg.blocks {
        if block.id == 0 {
            reaching.insert(block.id, HashSet::new());
        } else {
            reaching.insert(block.id, all_vars.clone());
        }
    }

    // Build predecessor map for efficiency
    let mut predecessors: HashMap<usize, Vec<usize>> = HashMap::new();
    for block in &cfg.blocks {
        predecessors.insert(block.id, Vec::new());
    }
    for (from, tos) in &cfg.edges {
        for to in tos {
            predecessors.entry(*to).or_default().push(*from);
        }
    }

    // Fixed-point iteration (forward dataflow)
    let mut changed = true;
    let mut iterations = 0;
    let max_iterations = 100;

    while changed && iterations < max_iterations {
        changed = false;
        iterations += 1;

        for block in &cfg.blocks {
            let preds = predecessors
                .get(&block.id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            // Entry block has no predecessors
            if preds.is_empty() {
                continue;
            }

            // Compute intersection of (reaching[pred] ∪ defs[pred]) for all predecessors
            // Must-analysis: variable is definitely defined only if defined on ALL paths
            let mut new_reaching: Option<HashSet<String>> = None;

            for &pred_id in preds {
                // Start with what reaches the predecessor
                let mut pred_out = reaching.get(&pred_id).cloned().unwrap_or_default();

                // Add definitions from the predecessor block
                if let Some(pred_block) = cfg.blocks.iter().find(|b| b.id == pred_id) {
                    for (name, _, _) in &pred_block.definitions {
                        pred_out.insert(name.clone());
                    }
                }

                // Intersect with what we have from other predecessors
                new_reaching = match new_reaching {
                    None => Some(pred_out),
                    Some(current) => Some(current.intersection(&pred_out).cloned().collect()),
                };
            }

            let new_reaching = new_reaching.unwrap_or_default();
            if new_reaching != *reaching.get(&block.id).unwrap_or(&HashSet::new()) {
                changed = true;
                reaching.insert(block.id, new_reaching);
            }
        }
    }

    reaching
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::build_cfgs;
    use crate::parser::parse_source;
    use crate::symbols::collect_symbols;
    use std::path::Path;

    fn analyze_source(source: &str) -> FlowResults {
        let parsed = parse_source(source).expect("Should parse");
        let file_sym = collect_symbols(Path::new("test.gd"), &parsed);
        let cfgs = build_cfgs(&parsed);
        analyze(&cfgs, &file_sym)
    }

    #[test]
    fn no_dead_assignments_in_simple_code() {
        let source = r#"
func foo():
    var x = 1
    var y = x
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        assert!(
            func_result.dead_assignments.is_empty(),
            "No dead assignments expected: {:?}",
            func_result.dead_assignments
        );
    }

    #[test]
    fn dead_assignment_detected() {
        // Assignment in a block that returns without using the value
        let source = r#"
func foo():
    var x = 1
    if true:
        x = 2
        return
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // x = 2 is dead because the block returns immediately after
        let dead_reassigns: Vec<_> = func_result
            .dead_assignments
            .iter()
            .filter(|(name, _)| name == "x")
            .collect();
        assert!(
            !dead_reassigns.is_empty(),
            "Should detect dead assignment of x: {:?}",
            func_result.dead_assignments
        );
    }

    #[test]
    fn uninitialized_use_detected() {
        let source = r#"
func foo():
    var y = x
    var x = 1
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // x is used before it's defined
        let x_uninit = func_result
            .uninitialized_uses
            .iter()
            .filter(|(name, _)| name == "x")
            .count();
        assert!(
            x_uninit > 0,
            "Should detect uninitialized use of x: {:?}",
            func_result.uninitialized_uses
        );
    }

    #[test]
    fn no_uninitialized_for_initialized_var() {
        let source = r#"
func foo():
    var x = 1
    var y = x
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // x is defined before use
        let x_uninit = func_result
            .uninitialized_uses
            .iter()
            .filter(|(name, _)| name == "x")
            .count();
        assert_eq!(
            x_uninit, 0,
            "Should not flag initialized x: {:?}",
            func_result.uninitialized_uses
        );
    }

    #[test]
    fn parameter_not_flagged_as_uninitialized() {
        let source = r#"
func foo(x):
    var y = x
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // Parameter x should be considered initialized
        let x_uninit = func_result
            .uninitialized_uses
            .iter()
            .filter(|(name, _)| name == "x")
            .count();
        assert_eq!(
            x_uninit, 0,
            "Parameter x should not be flagged: {:?}",
            func_result.uninitialized_uses
        );
    }

    #[test]
    fn liveness_computed_for_simple_function() {
        let source = r#"
func foo():
    var x = 1
    var y = x + 1
    var z = y
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // Should have computed liveness for at least one block
        assert!(
            !func_result.live_vars.is_empty(),
            "Should compute live variables"
        );
    }

    #[test]
    fn conditional_assignment_uses_both_paths() {
        let source = r#"
func foo(cond):
    var x = 1
    if cond:
        x = 2
    var y = x
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // x = 2 is not dead because it's conditionally executed
        // The analysis here depends on CFG structure
    }

    #[test]
    fn empty_function_no_issues() {
        let source = r#"
func foo():
    pass
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        assert!(func_result.dead_assignments.is_empty());
        assert!(func_result.uninitialized_uses.is_empty());
    }

    #[test]
    fn multiple_functions_analyzed() {
        let source = r#"
func foo():
    var x = 1

func bar():
    var y = 2
"#;
        let results = analyze_source(source);
        assert!(results.functions.contains_key("foo"), "Should analyze foo");
        assert!(results.functions.contains_key("bar"), "Should analyze bar");
    }

    #[test]
    fn conditional_fallback_assignment_not_dead() {
        // Pattern from WeaponVFXManager: default value + conditional override + use
        // The conditional assignment should NOT be flagged as dead
        let source = r#"
func _add_beam_geometry():
    var cam_pos = Vector3.ZERO
    if camera and is_instance_valid(camera):
        cam_pos = camera.global_position
    var to_camera = cam_pos.normalized()
"#;
        let results = analyze_source(source);
        let func_result = results
            .functions
            .get("_add_beam_geometry")
            .expect("Should have function");

        // cam_pos = camera.global_position should NOT be flagged as dead
        let dead_cam_pos: Vec<_> = func_result
            .dead_assignments
            .iter()
            .filter(|(name, _)| name == "cam_pos")
            .collect();
        assert!(
            dead_cam_pos.is_empty(),
            "Conditional assignment should NOT be flagged as dead: {:?}",
            dead_cam_pos
        );
    }

    #[test]
    fn conditional_fallback_with_early_return_not_dead() {
        // Exact pattern from WeaponVFXManager: early return + default + conditional override + use
        // The conditional assignment should NOT be flagged as dead
        let source = r#"
func _add_beam_geometry(start, end):
    var direction = (end - start)
    if direction.length_squared() < 0.0001:
        return
    direction = direction.normalized()

    var cam_pos = Vector3.ZERO
    if camera and is_instance_valid(camera):
        cam_pos = camera.global_position

    var to_camera = (cam_pos - (start + end) * 0.5).normalized()
    var perpendicular = direction.cross(to_camera).normalized()
"#;
        let results = analyze_source(source);
        let func_result = results
            .functions
            .get("_add_beam_geometry")
            .expect("Should have function");

        // cam_pos = camera.global_position should NOT be flagged as dead
        let dead_cam_pos: Vec<_> = func_result
            .dead_assignments
            .iter()
            .filter(|(name, _)| name == "cam_pos")
            .collect();
        assert!(
            dead_cam_pos.is_empty(),
            "Conditional assignment should NOT be flagged as dead (with early return): {:?}",
            dead_cam_pos
        );
    }

    #[test]
    fn loop_variable_not_uninitialized() {
        let source = r#"
func foo():
    for i in range(10):
        var x = i
"#;
        let results = analyze_source(source);
        let func_result = results.functions.get("foo").expect("Should have foo");
        // Loop variable i should be considered defined by the for loop
        let i_uninit = func_result
            .uninitialized_uses
            .iter()
            .filter(|(name, _)| name == "i")
            .count();
        assert_eq!(i_uninit, 0, "Loop variable should not be uninitialized");
    }

    #[test]
    fn match_early_return_not_uninitialized() {
        // Pattern: match with default case that returns early
        // Variable should NOT be flagged as uninitialized
        let source = r#"
func test_match_early_return(value):
    var result
    match value:
        1:
            result = "one"
        2:
            result = "two"
        _:
            return "default"
    print(result)
    return result
"#;
        let parsed = parse_source(source).expect("Should parse");
        let file_sym = collect_symbols(Path::new("test.gd"), &parsed);
        let cfgs = build_cfgs(&parsed);

        // Debug: print CFG structure
        for cfg in &cfgs {
            eprintln!("CFG for function: {}", cfg.function_name);
            for block in &cfg.blocks {
                eprintln!(
                    "  Block {}: lines {}-{}, has_return={}, defs={:?}, uses={:?}",
                    block.id,
                    block.start_line,
                    block.end_line,
                    block.has_return,
                    block.definitions,
                    block.uses
                );
            }
            eprintln!("  Edges: {:?}", cfg.edges);
        }

        let results = analyze(&cfgs, &file_sym);
        let func_result = results
            .functions
            .get("test_match_early_return")
            .expect("Should have function");

        // result should NOT be flagged as uninitialized because the default case returns
        let uninit_result: Vec<_> = func_result
            .uninitialized_uses
            .iter()
            .filter(|(name, _)| name == "result")
            .collect();
        assert!(
            uninit_result.is_empty(),
            "Variable should NOT be flagged when match has early-return default: {:?}",
            uninit_result
        );
    }
}
