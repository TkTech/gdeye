use crate::common::*;

#[test]
fn chained_null_access() {
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    assert!(
        has_message(&output, "can return null"),
        "Should detect chained null access.\nOutput:\n{}",
        output
    );
}

#[test]
fn dollar_access_not_flagged() {
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    // $ is sugar for get_node() which throws on missing nodes, never returns null.
    assert!(
        !has_message(&output, "node access can return null"),
        "Should NOT flag $ access (get_node throws, doesn't return null).\nOutput:\n{}",
        output
    );
}

#[test]
fn get_node_not_flagged() {
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    // get_node() throws on missing nodes, never returns null.
    assert!(
        !has_message(&output, "`get_node()` can return null"),
        "Should NOT flag get_node() (throws, doesn't return null).\nOutput:\n{}",
        output
    );
}

#[test]
fn null_access_count() {
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    let count = count_rule(&output, "correctness/null-access");
    // Should flag: test_chained (3) only
    // Should NOT flag: test_dollar, test_safe, test_guarded_*, etc.
    assert_eq!(
        count, 3,
        "Expected exactly 3 null-access warnings (chained calls only). Got {}.\nOutput:\n{}",
        count, output
    );
}

#[test]
fn is_instance_valid_guard_not_flagged() {
    // Verify that is_instance_valid guard is recognized
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    // The test_guarded_is_instance_valid function should not have any warnings
    // because the access is guarded by is_instance_valid(node)
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("null-access") && l.contains("test_guarded_is_instance_valid"))
        .collect();
    assert!(
        lines.is_empty(),
        "is_instance_valid guard should suppress null-access warning.\nLines:\n{}",
        lines.join("\n")
    );
}

#[test]
fn null_comparison_guard_not_flagged() {
    // Verify that != null guard is recognized
    let output = run_rule("correctness_null_access.gd", &["correctness/null-access"]);
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("null-access") && l.contains("test_guarded_null_comparison"))
        .collect();
    assert!(
        lines.is_empty(),
        "!= null guard should suppress null-access warning.\nLines:\n{}",
        lines.join("\n")
    );
}
