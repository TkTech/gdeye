use crate::common::*;

// Tests for allocations in process functions

#[test]
fn perf_allocation_array_literal_in_process() {
    let output = run_gdeye("perf_process_allocation.gd");
    assert!(
        has_message(&output, "Array literal allocation inside `_process`"),
        "Should detect array literal in _process.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_dictionary_literal_in_process() {
    let output = run_gdeye("perf_process_allocation.gd");
    assert!(
        has_message(&output, "Dictionary literal allocation inside `_process`"),
        "Should detect dictionary literal in _process.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_constructor_in_physics_process() {
    let output = run_gdeye("perf_process_allocation.gd");
    assert!(
        has_message(&output, "`Array()` allocation inside `_physics_process`"),
        "Should detect Array() constructor in _physics_process.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_not_in_regular_function() {
    let output = run_gdeye("perf_process_allocation.gd");
    assert!(
        !has_message(&output, "inside `some_function`"),
        "Should not flag allocations in regular functions.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_in_process_correct_count() {
    let output = run_gdeye("perf_process_allocation.gd");
    let count = count_rule(&output, "perf/allocation");
    assert_eq!(
        count, 3,
        "Expected 3 allocation warnings in process functions (array in _process, dict in _process, Array() in _physics_process). Got {}.\nOutput:\n{}",
        count, output
    );
}

// Tests for allocations in loops

#[test]
fn perf_allocation_in_loop_array() {
    let output = run_gdeye("perf_allocation_in_loop.gd");
    assert!(
        has_message(&output, "`Array()` allocation inside `for` loop"),
        "Should detect Array() in for loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_in_loop_dictionary() {
    let output = run_gdeye("perf_allocation_in_loop.gd");
    assert!(
        has_message(&output, "`Dictionary()` allocation inside `while` loop"),
        "Should detect Dictionary() in while loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_allocation_in_loop_correct_count() {
    let output = run_gdeye("perf_allocation_in_loop.gd");
    let count = count_rule(&output, "perf/allocation");
    // Should detect: Array(), Dictionary(), array literal [1,2,3,4,5,6], dict literal
    assert!(
        count >= 4,
        "Expected at least 4 allocation warnings in loops. Got {}.\nOutput:\n{}",
        count,
        output
    );
}
