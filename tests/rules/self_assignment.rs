use crate::common::*;

#[test]
fn self_assignment_detected() {
    let output = run_rule(
        "correctness_self_assignment.gd",
        &["correctness/self-assignment"],
    );
    assert!(
        has_message(&output, "Self-assignment"),
        "Should detect self-assignment.\nOutput:\n{}",
        output
    );
}

#[test]
fn self_assignment_count() {
    let output = run_rule(
        "correctness_self_assignment.gd",
        &["correctness/self-assignment"],
    );
    let count = count_rule(&output, "correctness/self-assignment");
    assert!(
        count >= 2,
        "Expected at least 2 self-assignment warnings (x = x, health = health). Got {}.\nOutput:\n{}",
        count, output
    );
}

#[test]
fn augmented_assignment_not_flagged() {
    let output = run_rule(
        "correctness_self_assignment.gd",
        &["correctness/self-assignment"],
    );
    // += and -= should not be flagged
    assert!(
        !has_message(&output, "augmented"),
        "Should not flag augmented assignments.\nOutput:\n{}",
        output
    );
}
