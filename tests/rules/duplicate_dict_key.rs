use crate::common::*;

#[test]
fn duplicate_string_key() {
    let output = run_rule(
        "correctness_duplicate_dict_key.gd",
        &["correctness/duplicate-dict-key"],
    );
    assert!(
        has_message(&output, "Duplicate dictionary key"),
        "Should flag duplicate string key.\nOutput:\n{}",
        output
    );
}

#[test]
fn duplicate_integer_key() {
    let output = run_rule(
        "correctness_duplicate_dict_key.gd",
        &["correctness/duplicate-dict-key"],
    );
    assert!(
        has_message(&output, "first defined on line"),
        "Should reference first definition.\nOutput:\n{}",
        output
    );
}

#[test]
fn duplicate_dict_key_count() {
    let output = run_rule(
        "correctness_duplicate_dict_key.gd",
        &["correctness/duplicate-dict-key"],
    );
    let count = count_rule(&output, "correctness/duplicate-dict-key");
    assert_eq!(
        count, 3,
        "Expected 3 duplicate key warnings. Got {}.\nOutput:\n{}",
        count, output
    );
}
