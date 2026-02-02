use crate::common::*;

#[test]
fn uninitialized_variable_detected() {
    let output = run_gdeye("correctness_uninitialized_variable.gd");
    assert!(
        has_message(&output, "may be used before initialization"),
        "Should detect uninitialized variable use.\nOutput:\n{}",
        output
    );
}

#[test]
fn uninitialized_variable_specific_case() {
    let output = run_gdeye("correctness_uninitialized_variable.gd");
    // The function test_uninitialized_use should have a warning for `x`
    assert!(
        has_message(&output, "`x` may be used"),
        "Should detect `x` used before initialization.\nOutput:\n{}",
        output
    );
}

#[test]
fn initialized_variable_not_flagged() {
    let output = run_gdeye("correctness_uninitialized_variable.gd");
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
    let output = run_gdeye("correctness_uninitialized_variable.gd");
    assert!(
        !has_message(&output, "`_unused`"),
        "Should not flag underscore-prefixed variables.\nOutput:\n{}",
        output
    );
}

#[test]
fn uninitialized_variable_count() {
    let output = run_gdeye("correctness_uninitialized_variable.gd");
    let count = count_rule(&output, "correctness/uninitialized-variable");
    // Should flag at least test_uninitialized_use
    assert!(
        count >= 1,
        "Expected at least 1 uninitialized-variable warning. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn match_early_return_not_flagged() {
    let output = run_gdeye("correctness_uninitialized_variable.gd");
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
    let output = run_gdeye("correctness_uninitialized_variable.gd");
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
    let output = run_gdeye("correctness_uninitialized_variable.gd");
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
