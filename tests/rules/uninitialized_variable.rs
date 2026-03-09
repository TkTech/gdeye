use crate::common::*;

#[test]
fn uninitialized_variable_detected() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    assert!(
        has_message(&output, "may be used before initialization"),
        "Should detect uninitialized variable use.\nOutput:\n{}",
        output
    );
}

#[test]
fn uninitialized_variable_specific_case() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // The function test_uninitialized_use should have a warning for `x`
    assert!(
        has_message(&output, "`x` may be used"),
        "Should detect `x` used before initialization.\nOutput:\n{}",
        output
    );
}

#[test]
fn initialized_variable_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // Check that test_initialized function doesn't produce a warning
    // by ensuring the warning count is limited
    let warning_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("uninitialized-variable"))
        .collect();

    // Should have some warnings but not for the test_initialized function
    for line in &warning_lines {
        assert!(
            !line.contains("test_initialized"),
            "Should not flag initialized variable in test_initialized.\nLine: {}",
            line
        );
    }
}

#[test]
fn underscore_prefix_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    assert!(
        !has_message(&output, "`_unused`"),
        "Should not flag underscore-prefixed variables.\nOutput:\n{}",
        output
    );
}

#[test]
fn uninitialized_variable_count() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // Should flag at least test_uninitialized_use
    let count = count_rule(&output, "correctness/uninitialized-variable");
    assert!(
        count >= 1,
        "Expected at least 1 uninitialized-variable warning. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn match_early_return_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // test_match_early_return has a match with default case that returns,
    // so `result` is always initialized when used.
    // The function is at lines 48-59, so check that no warning appears in that range.
    // Note: test_conditional_uninitialized at line 14 SHOULD warn about result.
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| {
            l.contains("uninitialized-variable")
                && l.contains("result")
                && (l.contains(":58:") || l.contains(":59:"))
        })
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable in match with early-return default case.\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}

#[test]
fn if_early_return_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // test_if_early_return has if-elif-else with return in else,
    // so `category` is always initialized when used
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("uninitialized-variable") && l.contains("category"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable in if-elif with early-return else.\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}

#[test]
fn loop_continue_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // test_loop_continue has continue in else branch,
    // so `status` is always initialized when print is reached
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("uninitialized-variable") && l.contains("status"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable when else branch has continue.\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}

#[test]
fn loop_guard_flag_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // test_loop_guard_flag uses a boolean flag to ensure the variable is initialized
    // on the first loop iteration. Subsequent iterations skip the init but the
    // variable is already set from the first pass. Should NOT warn.
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("uninitialized-variable") && l.contains("cached_value"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable protected by a guard flag in a loop.\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}

#[test]
fn correlated_condition_not_flagged() {
    let output = run_rule(
        "correctness_uninitialized_variable.gd",
        &["correctness/uninitialized-variable"],
    );
    // test_correlated_condition uses the same boolean condition to guard both
    // the assignment and the use of the variable. If the condition is false,
    // neither the assignment nor the use is reached. Should NOT warn.
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("uninitialized-variable") && l.contains("special_value"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag variable when same condition guards both assignment and use.\nMatching lines: {:?}\nOutput:\n{}",
        lines,
        output
    );
}
