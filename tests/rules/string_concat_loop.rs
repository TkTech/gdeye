use crate::common::*;

#[test]
fn perf_string_concat_loop_for() {
    let output = run_rule(
        "perf_string_concat_loop.gd",
        &["perf/string-concat-in-loop"],
    );
    assert!(
        has_message(&output, "String concatenation in loop"),
        "Should detect string += in for loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn perf_string_concat_loop_correct_count() {
    let output = run_rule(
        "perf_string_concat_loop.gd",
        &["perf/string-concat-in-loop"],
    );
    let count = count_rule(&output, "perf/string-concat-in-loop");
    // Should detect: bad_string_concat_for, bad_string_concat_while,
    // bad_string_concat_assignment, bad_string_concat_with_variable
    assert!(
        count >= 4,
        "Expected at least 4 string-concat-in-loop warnings. Got {}.\nOutput:\n{}",
        count,
        output
    );
}
