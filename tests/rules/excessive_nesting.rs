use crate::common::*;

#[test]
fn style_excessive_nesting() {
    let output = run_rule("style_function_too_long.gd", &["style/excessive-nesting"]);
    assert!(
        has_message(&output, "excessive-nesting") || has_message(&output, "nesting"),
        "Should flag deeply nested function.\nOutput:\n{}",
        output
    );
}
