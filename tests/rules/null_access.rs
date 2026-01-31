use crate::common::*;

#[test]
fn chained_null_access() {
    let output = run_gdeye("correctness_null_access.gd");
    assert!(
        has_message(&output, "can return null"),
        "Should detect chained null access.\nOutput:\n{}",
        output
    );
}

#[test]
fn dollar_null_access() {
    let output = run_gdeye("correctness_null_access.gd");
    assert!(
        has_message(&output, "node access can return null"),
        "Should detect $ null access.\nOutput:\n{}",
        output
    );
}

#[test]
fn null_access_count() {
    let output = run_gdeye("correctness_null_access.gd");
    let count = count_rule(&output, "correctness/null-access");
    // Should flag: test_chained (3), test_dollar (2)
    // Should NOT flag: test_safe, test_guarded_*, etc.
    assert!(
        count >= 3,
        "Expected at least 3 null-access warnings. Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn guarded_access_not_flagged() {
    let output = run_gdeye("correctness_null_access.gd");
    // Count warnings - guarded accesses should reduce total
    let count = count_rule(&output, "correctness/null-access");
    // The guarded functions test_guarded_dollar, test_guarded_has_node,
    // test_guarded_is_instance_valid, test_guarded_null_comparison, test_guarded_boolean_and
    // should NOT add any warnings (their accesses are protected by if guards)
    // Only test_chained (3) and test_dollar (2) should be flagged = 5 total
    assert!(
        count <= 6,
        "Expected at most 6 null-access warnings (guarded accesses shouldn't be flagged). Got {}.\nOutput:\n{}",
        count,
        output
    );
}

#[test]
fn is_instance_valid_guard_not_flagged() {
    // Verify that is_instance_valid guard is recognized
    let output = run_gdeye("correctness_null_access.gd");
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
    let output = run_gdeye("correctness_null_access.gd");
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
