use crate::common::*;

#[test]
fn style_standalone_expression_variable_flagged() {
    let output = run_gdeye_with_args("style_standalone_expression.gd", &[]);
    assert!(
        has_message(&output, "Expression `member` has no side effect"),
        "Should flag standalone variable reference.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_standalone_expression_arithmetic_flagged() {
    let output = run_gdeye_with_args("style_standalone_expression.gd", &[]);
    assert!(
        has_message(&output, "Expression `1 + 2` has no side effect"),
        "Should flag standalone arithmetic.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_standalone_expression_call_not_flagged() {
    let output = run_gdeye_with_args("style_standalone_expression.gd", &[]);
    assert!(
        !has_message(&output, "print"),
        "Should NOT flag function calls.\nOutput:\n{}",
        output
    );
}

#[test]
fn style_standalone_expression_correct_count() {
    let output = run_gdeye_with_args("style_standalone_expression.gd", &[]);
    let count = count_rule(&output, "style/standalone-expression");
    assert_eq!(
        count, 4,
        "Should find exactly 4 standalone expressions (member, 1+2, member==10, self.member).\nOutput:\n{}",
        output
    );
}
