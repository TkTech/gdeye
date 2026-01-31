use crate::common::*;

#[test]
fn correctness_unused_signal() {
    let output = run_gdeye("correctness_unused_signal.gd");
    assert!(
        has_message(&output, "Signal `unused_signal`"),
        "Should detect unused signal.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_emitted_signal_not_flagged() {
    let output = run_gdeye("correctness_unused_signal.gd");
    assert!(
        !has_message(&output, "`used_signal`"),
        "Should not flag emitted signal.\nOutput:\n{}",
        output
    );
}

#[test]
fn correctness_connected_signal_not_flagged() {
    let output = run_gdeye("correctness_unused_signal.gd");
    assert!(
        !has_message(&output, "`connected_signal`"),
        "Should not flag connected signal.\nOutput:\n{}",
        output
    );
}

#[test]
fn cross_file_signal_emitted_locally_not_flagged() {
    // player.gd emits `hit` and `died` signals via signal.emit() syntax
    let output = run_gdeye_with_args(
        "cross_file_usage",
        &[
            "--disable",
            "style/naming-convention",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/function-too-long",
            "--disable",
            "style/excessive-nesting",
        ],
    );
    let unused_signal_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unused-signal"))
        .collect();
    for line in &unused_signal_lines {
        assert!(
            !line.contains("`hit`"),
            "Should NOT flag `hit` -- emitted via hit.emit().\nLines:\n{}",
            unused_signal_lines.join("\n")
        );
        assert!(
            !line.contains("`died`"),
            "Should NOT flag `died` -- emitted via died.emit().\nLines:\n{}",
            unused_signal_lines.join("\n")
        );
    }
}

#[test]
fn cross_file_signal_connected_externally_not_flagged() {
    // game.gd connects to player.hit via player.hit.connect() syntax
    let output = run_gdeye_with_args(
        "cross_file_usage",
        &[
            "--disable",
            "style/naming-convention",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/function-too-long",
            "--disable",
            "style/excessive-nesting",
        ],
    );
    // The hit signal should be marked as used because game.gd connects to it
    let unused_signal_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unused-signal"))
        .collect();
    for line in &unused_signal_lines {
        assert!(
            !line.contains("`hit`"),
            "Should NOT flag `hit` -- connected cross-file via player.hit.connect().\nLines:\n{}",
            unused_signal_lines.join("\n")
        );
    }
}
