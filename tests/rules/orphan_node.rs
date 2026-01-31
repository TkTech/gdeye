use crate::common::*;

#[test]
fn orphan_node_detected() {
    let output = run_gdeye("correctness_orphan_node.gd");
    assert!(
        has_message(&output, "never added to the scene tree"),
        "Should detect orphan node.\nOutput:\n{}",
        output
    );
}

#[test]
fn orphan_node_safe_not_flagged() {
    let output = run_gdeye("correctness_orphan_node.gd");
    // test_safe uses add_child, should not be flagged
    // The line for test_safe is line 12
    let flagged_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("correctness/orphan-node") && l.contains(":13:"))
        .collect();
    assert!(
        flagged_lines.is_empty(),
        "Should not flag node that is add_child'd.\nOutput:\n{}",
        output
    );
}

#[test]
fn orphan_node_count() {
    let output = run_gdeye("correctness_orphan_node.gd");
    let count = count_rule(&output, "correctness/orphan-node");
    assert!(
        count >= 1,
        "Expected at least 1 orphan-node warning. Got {}.\nOutput:\n{}",
        count,
        output
    );
}
