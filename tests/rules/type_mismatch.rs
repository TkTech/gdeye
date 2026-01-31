use crate::common::*;

#[test]
fn correctness_type_mismatch_literal() {
    let output = run_gdeye("correctness_type_mismatch.gd");
    assert!(
        has_message(&output, "type-mismatch") && has_message(&output, "`x`"),
        "Should flag type mismatch: String var with int literal.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_type_mismatch_call() {
    let output = run_gdeye("correctness_type_mismatch.gd");
    assert!(
        has_message(&output, "type-mismatch") && has_message(&output, "`vp`"),
        "Should flag type mismatch: String var with get_viewport() call.\nOutput:\n{}",
        output
    );
}
