use crate::common::*;

#[test]
fn shadowed_variable_member() {
    let output = run_rule(
        "correctness_shadowed_variable.gd",
        &["correctness/shadowed-variable"],
    );
    assert!(
        output.contains("shadows a member variable"),
        "Should detect member variable shadowing.\nOutput:\n{}",
        output
    );
}

#[test]
fn shadowed_variable_parameter() {
    let output = run_rule(
        "correctness_shadowed_variable.gd",
        &["correctness/shadowed-variable"],
    );
    assert!(
        output.contains("shadows a parameter"),
        "Should detect parameter shadowing.\nOutput:\n{}",
        output
    );
}

#[test]
fn shadowed_variable_correct_count() {
    let output = run_rule(
        "correctness_shadowed_variable.gd",
        &["correctness/shadowed-variable"],
    );
    let count = output.matches("shadowed-variable").count();
    assert_eq!(
        count, 3,
        "Should find exactly 3 shadowed variables (health, speed, value).\nOutput:\n{}",
        output
    );
}

#[test]
fn shadowed_variable_static_func_not_flagged() {
    let output = run_rule(
        "correctness_shadowed_variable.gd",
        &["correctness/shadowed-variable"],
    );
    // Static functions cannot access self, so locals cannot shadow members
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("shadowed-variable") && l.contains("static_no_shadow"))
        .collect();
    assert!(
        lines.is_empty(),
        "Should NOT flag locals in static func as shadowing members.\nLines:\n{:?}",
        lines
    );
}
