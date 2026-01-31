use crate::common::*;

#[test]
fn style_function_too_long() {
    let output = run_gdeye_with_args("style_function_too_long.gd", &[]);
    assert!(
        has_message(&output, "function-too-long") || has_message(&output, "exceeds"),
        "Should flag very long function.\nOutput:\n{}",
        output
    );
}
