use crate::common::*;

#[test]
fn shadowed_variable_member() {
    let output = run_gdeye("correctness_shadowed_variable.gd");
    assert!(
        output.contains("shadows a member variable"),
        "Should detect member variable shadowing.\nOutput:\n{}",
        output
    );
}

#[test]
fn shadowed_variable_parameter() {
    let output = run_gdeye("correctness_shadowed_variable.gd");
    assert!(
        output.contains("shadows a parameter"),
        "Should detect parameter shadowing.\nOutput:\n{}",
        output
    );
}

#[test]
fn shadowed_variable_correct_count() {
    let output = run_gdeye("correctness_shadowed_variable.gd");
    let count = output.matches("shadowed-variable").count();
    assert_eq!(
        count, 3,
        "Should find exactly 3 shadowed variables (health, speed, value).\nOutput:\n{}",
        output
    );
}
