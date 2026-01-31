use crate::common::*;

#[test]
fn signal_mismatch_wrong_param_count() {
    let output = run_gdeye("signal_mismatch_project");
    assert!(
        has_message(&output, "signal-signature-mismatch"),
        "Should flag handler with wrong parameter count.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "_on_health_changed_wrong"),
        "Should specifically flag _on_health_changed_wrong.\nOutput:\n{}",
        output
    );
}

#[test]
fn signal_mismatch_correct_not_flagged() {
    let output = run_gdeye("signal_mismatch_project");
    let mismatch_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("signal-signature-mismatch"))
        .collect();
    for line in &mismatch_lines {
        assert!(!line.contains("_on_died"), "Should NOT flag _on_died");
        assert!(
            !line.contains("_on_damage_taken"),
            "Should NOT flag _on_damage_taken"
        );
        assert!(
            !line.contains("_on_health_changed_ok"),
            "Should NOT flag _on_health_changed_ok"
        );
    }
}

#[test]
fn signal_mismatch_correct_count() {
    let output = run_gdeye("signal_mismatch_project");
    let count = output.matches("signal-signature-mismatch").count();
    assert_eq!(
        count, 1,
        "Should find exactly 1 signal signature mismatch.\nOutput:\n{}",
        output
    );
}
