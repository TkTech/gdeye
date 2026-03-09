use crate::common::*;

#[test]
fn style_untyped_parameter() {
    let output = run_rule("style_rules.gd", &["style/untyped-parameter"]);
    let untyped_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("untyped-parameter"))
        .collect();
    assert!(
        untyped_lines.iter().any(|l| l.contains("untyped_param")),
        "Should flag untyped parameters in untyped_param.\nLines:\n{:?}",
        untyped_lines
    );
    // typed_param should not be flagged (use backtick-delimited name to avoid substring match)
    assert!(
        !untyped_lines.iter().any(|l| l.contains("`typed_param`")),
        "Should NOT flag typed parameters.\nLines:\n{:?}",
        untyped_lines
    );
}
