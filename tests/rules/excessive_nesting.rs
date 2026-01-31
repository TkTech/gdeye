use crate::common::*;

#[test]
fn style_excessive_nesting() {
    let output = run_gdeye_with_args("style_function_too_long.gd", &[]);
    assert!(
        has_message(&output, "excessive-nesting") || has_message(&output, "nesting"),
        "Should flag deeply nested function.\nOutput:\n{}",
        output
    );
}
