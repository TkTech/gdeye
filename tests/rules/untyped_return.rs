use crate::common::*;

#[test]
fn style_untyped_return() {
    let output = run_rule("style_rules.gd", &["style/untyped-return"]);
    let return_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("untyped-return"))
        .collect();
    assert!(
        return_lines.iter().any(|l| l.contains("no_return")),
        "Should flag function with no return type.\nLines:\n{:?}",
        return_lines
    );
    assert!(
        !return_lines.iter().any(|l| l.contains("with_return")),
        "Should NOT flag function with return type.\nLines:\n{:?}",
        return_lines
    );
}
