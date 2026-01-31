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
