use crate::common::*;

#[test]
fn await_in_for_loop() {
    let output = run_rule(
        "correctness_await_in_loop.gd",
        &["correctness/await-in-loop"],
    );
    assert!(
        has_message(&output, "Await inside `for` loop"),
        "Should detect await inside for loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn await_in_while_loop() {
    let output = run_rule(
        "correctness_await_in_loop.gd",
        &["correctness/await-in-loop"],
    );
    assert!(
        has_message(&output, "Await inside `while` loop"),
        "Should detect await inside while loop.\nOutput:\n{}",
        output
    );
}

#[test]
fn await_in_loop_count() {
    let output = run_rule(
        "correctness_await_in_loop.gd",
        &["correctness/await-in-loop"],
    );
    let count = count_rule(&output, "correctness/await-in-loop");
    assert_eq!(
        count, 2,
        "Expected 2 await-in-loop warnings. Got {}.\nOutput:\n{}",
        count, output
    );
}
