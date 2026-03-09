use crate::common::*;

#[test]
fn perf_process_get_node_in_process() {
    let output = run_rule("perf_process_get_node.gd", &["perf/process-get-node"]);
    assert!(
        has_message(&output, "`get_node()` called inside `_process`"),
        "Should detect get_node in _process.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_process_get_node_or_null() {
    let output = run_rule("perf_process_get_node.gd", &["perf/process-get-node"]);
    assert!(
        has_message(&output, "`get_node_or_null()` called inside `_process`"),
        "Should detect get_node_or_null in _process.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_process_get_node_in_input() {
    let output = run_rule("perf_process_get_node.gd", &["perf/process-get-node"]);
    assert!(
        has_message(&output, "called inside `_input`"),
        "Should detect get_node in _input.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_process_get_node_not_in_regular_function() {
    let output = run_rule("perf_process_get_node.gd", &["perf/process-get-node"]);
    assert!(
        !has_message(&output, "inside `some_function`"),
        "Should not flag get_node in regular functions.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_process_get_node_correct_count() {
    let output = run_rule("perf_process_get_node.gd", &["perf/process-get-node"]);
    let count = count_rule(&output, "perf/process-get-node");
    assert_eq!(
        count, 3,
        "Expected 3 process-get-node warnings. Got {}.\nOutput:\n{}",
        count, output
    );
}
