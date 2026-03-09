use crate::common::*;

#[test]
fn correctness_unused_parameter() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        has_message(&output, "Parameter `unused_param` in function `add`"),
        "Should detect unused parameter.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_used_parameters_not_flagged() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        !has_message(&output, "Parameter `a`"),
        "Should not flag used parameter `a`.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "Parameter `b`"),
        "Should not flag used parameter `b`.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_underscore_parameter_not_flagged() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        !has_message(&output, "`_context`"),
        "Should not flag _prefixed parameter.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_callback_parameters_not_flagged() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        !has_message(&output, "in function `_process`"),
        "Should not flag _process callback parameter.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "in function `_input`"),
        "Should not flag _input callback parameter.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_typed_unused_parameter() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        has_message(&output, "Parameter `unused` in function `typed_unused`"),
        "Should detect unused typed parameter.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_typed_used_parameters_not_flagged() {
    let output = run_rule(
        "correctness_unused_parameter.gd",
        &["correctness/unused-parameter"],
    );
    assert!(
        !has_message(&output, "Parameter `name` in function `typed_unused`"),
        "Should not flag used typed parameter.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "Parameter `value` in function `typed_unused`"),
        "Should not flag used typed parameter.\nOutput:\n{}",
        output
    );
}
