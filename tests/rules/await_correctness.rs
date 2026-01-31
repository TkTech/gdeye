use crate::common::*;

#[test]
fn await_in_process() {
    let output = run_gdeye("correctness_await_correctness.gd");
    assert!(
        has_message(&output, "Await inside `_process`"),
        "Should detect await inside _process.\nOutput:\n{}",
        output
    );
}

#[test]
fn await_in_physics_process() {
    let output = run_gdeye("correctness_await_correctness.gd");
    assert!(
        has_message(&output, "Await inside `_physics_process`"),
        "Should detect await inside _physics_process.\nOutput:\n{}",
        output
    );
}

#[test]
fn await_on_non_coroutine() {
    let output = run_gdeye("correctness_await_correctness.gd");
    assert!(
        has_message(&output, "not a coroutine"),
        "Should detect await on non-coroutine.\nOutput:\n{}",
        output
    );
}

#[test]
fn await_correctness_count() {
    let output = run_gdeye("correctness_await_correctness.gd");
    let count = count_rule(&output, "correctness/await-correctness");
    assert!(
        count >= 3,
        "Expected at least 3 await-correctness warnings. Got {}.\nOutput:\n{}",
        count,
        output
    );
}
