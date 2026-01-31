use crate::common::*;

// Tests for unused member variables

#[test]
fn correctness_unused_member_variable_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        has_message(&output, "`unused_member`"),
        "Should flag unused member variables.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_used_member_variable_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`used_member`"),
        "Should not flag used member variable.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_exported_variable_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`exported_value`"),
        "Should not flag @export variables.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_underscore_prefixed_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`_intentionally_unused`"),
        "Should not flag _prefixed variables.\nOutput:\n{}",
        output
    );
}

// Tests for unused local variables

#[test]
fn correctness_unused_local_variable() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        has_message(&output, "`unused_local`"),
        "Should detect unused local variable.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_used_local_variable_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`used_local`"),
        "Should not flag used local variable.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_variable_used_in_condition_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`flag`"),
        "Should not flag variable used in if condition.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_variable_used_in_return_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`result`"),
        "Should not flag variable used in return.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_variable_used_in_for_not_flagged() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        !has_message(&output, "`items`"),
        "Should not flag variable used in for loop iterator.\nOutput:\n{}",
        output
    );
}

// Tests for match statement variable usage

#[test]
fn correctness_match_body_uses_variable() {
    let output = run_gdeye("correctness_match_usage.gd");
    assert!(
        !has_message(&output, "`threshold`"),
        "Should not flag variable used in match body returns.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_match_subject_uses_variable() {
    let output = run_gdeye("correctness_match_usage.gd");
    assert!(
        !has_message(&output, "`subject`"),
        "Should not flag variable used as match subject.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_match_genuinely_unused_variable() {
    let output = run_gdeye("correctness_match_usage.gd");
    assert!(
        has_message(&output, "`unused_in_match`"),
        "Should flag variable genuinely unused in match.\nOutput:\n{}",
        output
    );
}

// Tests for dead assignments (value never read)

#[test]
fn dead_assignment_if_else_both_branches() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    assert!(
        !has_message(
            &output,
            "Variable `x` is assigned but its value is never read"
        ),
        "Should NOT flag var x = 1 (declaration dead assignments are suppressed).\nOutput:\n{}",
        output
    );
}

#[test]
fn dead_assignment_reassignment_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    assert!(
        has_message(&output, "Value assigned to `counter` is never read"),
        "Should flag counter = 1 in dead_reassignment (non-declaration dead assignment).\nOutput:\n{}",
        output
    );
}

#[test]
fn dead_assignment_conditional_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    assert!(
        !has_message(&output, "`w`"),
        "Should NOT flag var w = 1 when only one branch reassigns (no else).\nOutput:\n{}",
        output
    );
}

#[test]
fn dead_assignment_used_before_reassign_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    assert!(
        !has_message(&output, "`z`"),
        "Should NOT flag var z = 5 when z is used before reassignment.\nOutput:\n{}",
        output
    );
}

#[test]
fn dead_assignment_elif_condition_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    assert!(
        !has_message(&output, "`chance`"),
        "Should NOT flag var chance when it is used in an elif condition.\nOutput:\n{}",
        output
    );
}

#[test]
fn dead_assignment_correct_count() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    let count = count_rule(&output, "correctness/dead-store");
    assert_eq!(
        count, 1,
        "Should find exactly 1 dead assignment (counter = 1 in dead_reassignment).\nOutput:\n{}",
        output
    );
}

// Tests for useless assignments (from useless_assignment.rs)

#[test]
fn useless_assignment_detected() {
    let output = run_gdeye("correctness_useless_assignment.gd");
    assert!(
        has_message(&output, "is never read"),
        "Should detect useless assignment.\nOutput:\n{}",
        output
    );
}

#[test]
fn useless_assignment_count() {
    let output = run_gdeye("correctness_useless_assignment.gd");
    let count = count_rule(&output, "correctness/dead-store");
    // Expected: 1 from test_useless (counter = 1), plus warnings from test_match_unused
    assert!(
        count >= 1,
        "Expected at least 1 dead-store warning. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn useless_assignment_match_then_method_call_not_flagged() {
    let output = run_gdeye("correctness_useless_assignment.gd");
    // mode_idx is used in some_object.select(mode_idx), should NOT be flagged
    assert!(
        !output.contains("mode_idx"),
        "Should NOT flag variable assigned in match and used in method call.\nOutput:\n{}",
        output
    );
}

#[test]
fn useless_assignment_match_then_function_call_not_flagged() {
    let output = run_gdeye("correctness_useless_assignment.gd");
    // Check that test_match_then_function_call doesn't produce warnings for 'result'
    // by verifying the function name isn't mentioned in dead-store context
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("dead-store") && l.contains("result"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable assigned in match and used in print().\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}
