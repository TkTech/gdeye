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
    // var x = 1 is dead because both if and else branches overwrite x before use
    assert!(
        has_message(&output, "`x` is never read"),
        "Should flag var x = 1 when all branches overwrite before use.\nOutput:\n{}",
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
    // Expected dead assignments:
    // 1. var x = 1 in dead_in_if_else (all branches overwrite)
    // 2. var counter = 0 in dead_reassignment (immediately overwritten)
    // 3. counter = 1 in dead_reassignment (all branches overwrite)
    // 4. var value = 100 in dead_conditional_always_overwritten (all branches overwrite)
    assert_eq!(
        count, 4,
        "Should find exactly 4 dead assignments.\nOutput:\n{}",
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

// Tests for conditional assignment fallback pattern (default + conditional override)

#[test]
fn conditional_fallback_pattern_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    // Pattern: var x = default; if cond: x = other; use(x)
    // The default value IS used when condition is false
    assert!(
        !has_message(&output, "`cam_pos`"),
        "Should NOT flag conditional fallback pattern (default + optional override).\nOutput:\n{}",
        output
    );
}

#[test]
fn conditional_assignment_used_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    // Pattern: var x = get_value(); if x == bad: x = fallback; use(x)
    // The initial value IS read in the condition check
    assert!(
        !has_message(&output, "`fleet_pos`"),
        "Should NOT flag when initial value is read in condition.\nOutput:\n{}",
        output
    );
}

#[test]
fn conditional_always_overwritten_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    // Pattern: var x = initial; if cond: x = a else: x = b; use(x)
    // The initial value is NEVER read - all paths overwrite
    assert!(
        has_message(&output, "`value`")
            || has_message(&output, "dead_conditional_always_overwritten"),
        "Should flag when initial value is overwritten in all branches.\nOutput:\n{}",
        output
    );
}

// Regression tests for break-in-loop pattern (was causing false positives)

#[test]
fn break_in_search_loop_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    // Pattern: var x = default; for item in items: if cond: x = value; break; use(x)
    // The assignment before break IS used after the loop exits
    assert!(
        !has_message(&output, "`found`"),
        "Should NOT flag variable assigned before break in search loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn break_with_multiple_vars_not_flagged() {
    let output = run_gdeye("correctness_dead_assignment.gd");
    // Pattern: multiple vars assigned before break, all used after loop
    assert!(
        !has_message(&output, "`status`") || !output.contains("not_dead_break_with_multiple_vars"),
        "Should NOT flag variables assigned before break when used after loop.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "`status_color`")
            || !output.contains("not_dead_break_with_multiple_vars"),
        "Should NOT flag status_color assigned before break when used after loop.\nOutput:\n{}",
        output
    );
}
