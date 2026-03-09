use crate::common::*;

#[test]
fn style_unnecessary_pass_flagged() {
    let output = run_rule("style_unnecessary_pass.gd", &["style/unnecessary-pass"]);
    assert!(
        has_message(&output, "Unnecessary `pass`"),
        "Should flag pass in body with other statements.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_unnecessary_pass_only_pass_not_flagged() {
    let output = run_rule("style_unnecessary_pass.gd", &["style/unnecessary-pass"]);
    // Line 11 is the only_pass function -- should not be flagged
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unnecessary-pass"))
        .collect();
    for line in &lines {
        assert!(
            !line.contains(":11:"),
            "Should NOT flag pass when it's the only statement.\nLines:\n{:?}",
            lines
        );
    }
}

#[test]
fn style_unnecessary_pass_correct_count() {
    let output = run_rule("style_unnecessary_pass.gd", &["style/unnecessary-pass"]);
    let count = count_rule(&output, "style/unnecessary-pass");
    assert_eq!(
        count, 2,
        "Should find exactly 2 unnecessary pass statements.\nOutput:\n{}",
        output
    );
}
