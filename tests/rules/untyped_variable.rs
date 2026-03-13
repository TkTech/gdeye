use crate::common::*;

#[test]
fn style_untyped_variable_flags_untyped() {
    let output = run_rule("style_rules.gd", &["style/untyped-variable"]);
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("untyped-variable"))
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("untyped_dict")),
        "Should flag untyped_dict.\nLines:\n{:?}",
        lines
    );
    assert!(
        lines.iter().any(|l| l.contains("untyped_array")),
        "Should flag untyped_array.\nLines:\n{:?}",
        lines
    );
    assert!(
        lines.iter().any(|l| l.contains("untyped_int")),
        "Should flag untyped_int.\nLines:\n{:?}",
        lines
    );
}

#[test]
fn style_untyped_variable_skips_typed() {
    let output = run_rule("style_rules.gd", &["style/untyped-variable"]);
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("untyped-variable"))
        .collect();
    assert!(
        !lines.iter().any(|l| l.contains("`typed_var`")),
        "Should NOT flag typed_var.\nLines:\n{:?}",
        lines
    );
    assert!(
        !lines.iter().any(|l| l.contains("`inferred_var`")),
        "Should NOT flag inferred_var.\nLines:\n{:?}",
        lines
    );
}

#[test]
fn style_untyped_variable_inferred_type_correct() {
    let output = run_rule("style_rules.gd", &["style/untyped-variable"]);
    assert!(
        has_message(&output, "inferred: `Dictionary`"),
        "Should infer Dictionary for untyped_dict.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "inferred: `Array`"),
        "Should infer Array for untyped_array.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "inferred: `int`"),
        "Should infer int for untyped_int.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_untyped_variable_flags_local() {
    let output = run_rule("style_rules.gd", &["style/untyped-variable"]);
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("untyped-variable"))
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("local_dict")),
        "Should flag local_dict.\nLines:\n{:?}",
        lines
    );
}
