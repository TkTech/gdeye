use crate::common::*;

#[test]
fn invalid_action_detected() {
    let output = run_rule(
        "correctness_invalid_input_action.gd",
        &["correctness/invalid-input-action"],
    );
    assert!(
        has_message(&output, "not defined in project.godot"),
        "Should detect invalid input action.\nOutput:\n{}",
        output
    );
}

#[test]
fn valid_action_not_flagged() {
    let output = run_rule(
        "correctness_invalid_input_action.gd",
        &["correctness/invalid-input-action"],
    );
    // move_left and jump are defined in project.godot, should not be flagged
    assert!(
        !has_message(&output, "`move_left` is not defined"),
        "Should not flag valid action 'move_left'.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "`jump` is not defined"),
        "Should not flag valid action 'jump'.\nOutput:\n{}",
        output
    );
}

#[test]
fn builtin_action_not_flagged() {
    let output = run_rule(
        "correctness_invalid_input_action.gd",
        &["correctness/invalid-input-action"],
    );
    assert!(
        !has_message(&output, "`ui_accept` is not defined"),
        "Should not flag built-in UI action.\nOutput:\n{}",
        output
    );
}

#[test]
fn invalid_input_action_count() {
    let output = run_rule(
        "correctness_invalid_input_action.gd",
        &["correctness/invalid-input-action"],
    );
    let count = count_rule(&output, "correctness/invalid-input-action");
    assert!(
        count >= 2,
        "Expected at least 2 invalid-input-action warnings. Got {}.\nOutput:\n{}",
        count,
        output
    );
}
