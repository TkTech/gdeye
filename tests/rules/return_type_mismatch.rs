use crate::common::*;

#[test]
fn return_type_mismatch_wrong_literal() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        has_message(
            &output,
            "returns `String` but declared return type is `int`"
        ),
        "Should flag returning a string literal in an -> int function.\nOutput:\n{}",
        output
    );
}

#[test]
fn return_type_mismatch_wrong_call() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        has_message(
            &output,
            "returns `Viewport` but declared return type is `String`"
        ),
        "Should flag returning get_viewport() in a -> String function.\nOutput:\n{}",
        output
    );
}

#[test]
fn return_type_mismatch_compatible_numeric_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "declared return type is `float`"),
        "Should NOT flag returning int in a -> float function (numeric compat).\nOutput:\n{}",
        output
    );
}

#[test]
fn return_type_mismatch_correct_count() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    let count = count_rule(&output, "correctness/return-type-mismatch");
    assert_eq!(
        count, 2,
        "Should find exactly 2 return-type-mismatch warnings.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_ternary_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "ternary_return"),
        "Should NOT flag ternary_return since return type matches.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_paren_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "paren_return"),
        "Should NOT flag paren_return since return type matches.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_concat_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "concat_return"),
        "Should NOT flag concat_return since return type matches.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_inferred_type_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "inferred_type_new"),
        "Should NOT flag := inferred type variables.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "inferred_type_literal"),
        "Should NOT flag := inferred type with literals.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_cast_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "cast_return"),
        "Should NOT flag return with explicit `as` cast.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_lambda_not_flagged() {
    let output = run_gdeye("correctness_return_type_mismatch.gd");
    assert!(
        !has_message(&output, "uses_lambda_sort"),
        "Should NOT flag return statements inside lambdas as belonging to enclosing function.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_user_subclass_not_flagged() {
    let output = run_gdeye("return_type_subclass");
    assert!(
        !has_message(&output, "get_multiplayer_peer"),
        "Should NOT flag returning a user-defined subclass (SteamMultiplayerPeer) for parent type (MultiplayerPeer).\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_return_type_mismatch_user_subclass_still_flags_real_mismatch() {
    let output = run_gdeye("return_type_subclass");
    assert!(
        has_message(
            &output,
            "returns `String` but declared return type is `int`"
        ),
        "Should still flag genuine return type mismatches in the same project.\nOutput:\n{}",
        output
    );
}
