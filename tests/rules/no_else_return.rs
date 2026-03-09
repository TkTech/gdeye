use crate::common::*;

#[test]
fn else_after_return() {
    let output = run_rule("style_no_else_return.gd", &["style/no-else-return"]);
    assert!(
        has_message(&output, "Unnecessary `else` after `return`"),
        "Should flag else after return.\nOutput:\n{}",
        output
    );
}

#[test]
fn no_else_return_count() {
    let output = run_rule("style_no_else_return.gd", &["style/no-else-return"]);
    let count = count_rule(&output, "style/no-else-return");
    assert!(
        count >= 1,
        "Expected at least 1 no-else-return info. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn no_return_not_flagged() {
    let output = run_rule("style_no_else_return.gd", &["style/no-else-return"]);
    // The function without a return in the if-body should not be flagged
    // There should be exactly 3 flags: with_else_return, with_else_break, with_else_continue
    let count = count_rule(&output, "style/no-else-return");
    assert!(
        count <= 3,
        "Should not flag else without return in if-body. Got {} flags.\nOutput:\n{}",
        count,
        output
    );
}
