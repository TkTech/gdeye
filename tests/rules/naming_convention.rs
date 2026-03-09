use crate::common::*;

#[test]
fn style_naming_convention_bad_class() {
    let output = run_rule("style_rules.gd", &["style/naming-convention"]);
    assert!(
        has_message(&output, "myBadClassName"),
        "Should flag non-PascalCase class_name.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_naming_convention_bad_inner_class() {
    let output = run_rule("style_rules.gd", &["style/naming-convention"]);
    assert!(
        has_message(&output, "inner_bad_class"),
        "Should flag non-PascalCase inner class.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_naming_convention_bad_function() {
    let output = run_rule("style_rules.gd", &["style/naming-convention"]);
    assert!(
        has_message(&output, "MyFunction"),
        "Should flag non-snake_case function.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "camelCase"),
        "Should flag camelCase function.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_naming_convention_good_not_flagged() {
    let output = run_rule("style_rules.gd", &["style/naming-convention"]);
    let naming_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("naming-convention"))
        .collect();
    for line in &naming_lines {
        assert!(
            !line.contains("good_function"),
            "Should not flag good_function"
        );
        assert!(
            !line.contains("GoodInnerClass"),
            "Should not flag GoodInnerClass"
        );
        assert!(!line.contains("good_var"), "Should not flag good_var");
    }
}

#[test]
fn style_naming_convention_bad_variable() {
    let output = run_rule("style_rules.gd", &["style/naming-convention"]);
    assert!(
        has_message(&output, "BadVar"),
        "Should flag non-snake_case variable.\nOutput:\n{}",
        output
    );
}
