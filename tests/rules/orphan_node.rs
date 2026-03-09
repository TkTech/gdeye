use crate::common::*;

#[test]
fn orphan_node_detected() {
    let output = run_rule("correctness_orphan_node.gd", &["correctness/orphan-node"]);
    assert!(
        has_message(&output, "never added to the scene tree"),
        "Should detect orphan node.\nOutput:\n{}",
        output
    );
}

#[test]
fn orphan_node_safe_not_flagged() {
    let output = run_rule("correctness_orphan_node.gd", &["correctness/orphan-node"]);
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
    let output = run_rule("correctness_orphan_node.gd", &["correctness/orphan-node"]);
    let count = count_rule(&output, "correctness/orphan-node");
    assert!(
        count >= 1,
        "Expected at least 1 orphan-node warning. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn orphan_node_alias_not_flagged() {
    let output = run_rule("correctness_orphan_node.gd", &["correctness/orphan-node"]);
    // Node assigned to another variable then added via that alias
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("orphan-node") && l.contains("test_assigned_then_added"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag node added via alias variable.\nOutput:\n{}",
        output
    );
}

#[test]
fn orphan_node_member_alias_not_flagged() {
    let output = run_rule("correctness_orphan_node.gd", &["correctness/orphan-node"]);
    // Node assigned to member field then added via that field
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("orphan-node") && l.contains("test_assigned_to_member"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag node added via member field alias.\nOutput:\n{}",
        output
    );
}
