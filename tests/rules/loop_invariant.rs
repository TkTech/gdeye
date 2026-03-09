use crate::common::*;

// Note: Array and dictionary allocation detection has been moved to perf/allocation.
// The loop-invariant rule now focuses on computation hoisting (complex expressions, etc.)

#[test]
fn perf_loop_invariant_function_call_not_flagged() {
    let output = run_rule(
        "perf_loop_invariant.gd",
        &["perf/loop-invariant", "perf/allocation"],
    );
    assert!(
        !has_message(&output, "expensive_calculation"),
        "Should NOT flag function calls (could have side effects).\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_loop_invariant_method_call_not_flagged() {
    let output = run_rule(
        "perf_loop_invariant.gd",
        &["perf/loop-invariant", "perf/allocation"],
    );
    assert!(
        !has_message(&output, "randf_range"),
        "Should NOT flag method calls (could have side effects).\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_loop_invariant_depends_on_loop_var_not_flagged() {
    let output = run_rule(
        "perf_loop_invariant.gd",
        &["perf/loop-invariant", "perf/allocation"],
    );
    // The array [i, i + 1, i + 2] should not be flagged
    assert!(
        !has_message(&output, "i + 1"),
        "Should NOT flag expressions that depend on loop variable.\nOutput:\n{}",
        output
    );
}

// Allocation detection tests - now handled by perf/allocation

#[test]
fn perf_allocation_dict_in_loop_flagged() {
    let output = run_rule(
        "perf_loop_invariant.gd",
        &["perf/loop-invariant", "perf/allocation"],
    );
    // Dictionary allocation is now caught by perf/allocation
    assert!(
        has_message(&output, "perf/allocation") || has_message(&output, "allocation"),
        "Dictionary in loop should be caught by allocation rule.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_array_in_loop_flagged() {
    let output = run_rule(
        "perf_loop_invariant.gd",
        &["perf/loop-invariant", "perf/allocation"],
    );
    // Array allocation is now caught by perf/allocation
    let has_allocation =
        has_message(&output, "perf/allocation") || has_message(&output, "Array literal allocation");
    assert!(
        has_allocation,
        "Array in loop should be caught by allocation rule.\nOutput:\n{}",
        output
    );
}
