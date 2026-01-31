use crate::common::*;

#[test]
fn duplicated_preload() {
    let output = run_gdeye("correctness_duplicated_load.gd");
    assert!(
        has_message(&output, "loaded multiple times"),
        "Should detect duplicated preload.\nOutput:\n{}",
        output
    );
}

#[test]
fn duplicated_load_call() {
    let output = run_gdeye("correctness_duplicated_load.gd");
    assert!(
        has_message(&output, "res://scripts/helper.gd"),
        "Should detect duplicated load call.\nOutput:\n{}",
        output
    );
}

#[test]
fn duplicated_load_count() {
    let output = run_gdeye("correctness_duplicated_load.gd");
    let count = count_rule(&output, "correctness/duplicated-load");
    assert_eq!(
        count, 2,
        "Expected 2 duplicated-load warnings. Got {}.\nOutput:\n{}",
        count, output
    );
}
