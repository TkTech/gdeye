use crate::common::*;

#[test]
fn missing_return_no_return_statement() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        output.contains("get_value"),
        "Should flag get_value() which has no return.\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_partial_coverage() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        output.contains("conditional_return"),
        "Should flag conditional_return() which has if-without-else.\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_full_coverage_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !output.contains("full_coverage"),
        "Should NOT flag full_coverage() which has if/else both returning.\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_no_annotation_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !output.contains("no_return_type"),
        "Should NOT flag no_return_type() which has no return annotation.\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_correct_count() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    let count = output.matches("missing-return").count();
    assert_eq!(
        count, 4,
        "Should find exactly 4 missing-return issues (get_value, conditional_return, match_no_catchall, elif_missing_else).\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_nested_if_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !output.contains("nested_if"),
        "Should NOT flag nested_if() which has full nested if/else coverage.\nOutput:\n{}",
        output
    );
}

#[test]
fn missing_return_void_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !output.contains("void_return"),
        "Should NOT flag void_return() since -> void doesn't need a return.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_missing_return_match_no_catchall() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        has_message(&output, "match_no_catchall"),
        "Should flag missing return in match without catch-all.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_missing_return_match_all_return_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !has_message(&output, "match_all_return"),
        "Should NOT flag match_all_return which has catch-all returning.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_missing_return_elif_all_return_not_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        !has_message(&output, "elif_all_return"),
        "Should NOT flag elif_all_return which has all branches returning.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_missing_return_elif_missing_else_flagged() {
    let output = run_rule(
        "correctness_missing_return.gd",
        &["correctness/missing-return"],
    );
    assert!(
        has_message(&output, "elif_missing_else"),
        "Should flag elif_missing_else which has no else clause.\nOutput:\n{}",
        output
    );
}
