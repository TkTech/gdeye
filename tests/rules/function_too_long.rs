use crate::common::*;

#[test]
fn style_function_too_long() {
    let output = run_rule("style_function_too_long.gd", &["style/function-too-long"]);
    assert!(
        has_message(&output, "function-too-long") || has_message(&output, "exceeds"),
        "Should flag very long function.\nOutput:\n{}",
        output
    );
}
