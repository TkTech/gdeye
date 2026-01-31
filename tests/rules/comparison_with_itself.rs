use crate::common::*;

#[test]
fn correctness_comparison_self_equal_flagged() {
    let output = run_gdeye("correctness_comparison_self.gd");
    assert!(
        has_message(&output, "`x == x` is always true"),
        "Should flag x == x.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_comparison_self_not_equal_flagged() {
    let output = run_gdeye("correctness_comparison_self.gd");
    assert!(
        has_message(&output, "`x != x` is always false"),
        "Should flag x != x.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_comparison_self_different_vars_not_flagged() {
    let output = run_gdeye("correctness_comparison_self.gd");
    assert!(
        !has_message(&output, "`x == y`"),
        "Should NOT flag comparisons between different variables.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_comparison_self_complex_expr_flagged() {
    let output = run_gdeye("correctness_comparison_self.gd");
    assert!(
        has_message(&output, "`x + y == x + y` is always true"),
        "Should flag complex expression compared with itself.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_comparison_self_correct_count() {
    let output = run_gdeye("correctness_comparison_self.gd");
    let count = count_rule(&output, "correctness/comparison-with-itself");
    assert_eq!(
        count, 4,
        "Should find exactly 4 comparison-with-itself warnings.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_comparison_self_function_calls_not_flagged() {
    let output = run_gdeye("correctness_comparison_self.gd");
    // Function calls should NOT be flagged - they may return different values
    assert!(
        !has_message(&output, "get_value()"),
        "Should NOT flag function call comparisons like get_value() == get_value().\nOutput:\n{}",
        output
    );
}
