mod common;

use common::*;
use std::process::Command;

// --- Configuration: --disable ---

#[test]
fn cli_disable_suppresses_rule() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &["--disable", "correctness/dead-store"],
    );
    assert_eq!(
        count_rule(&output, "correctness/dead-store"),
        0,
        "Disabled rule should produce no diagnostics.\nOutput:\n{}",
        output
    );
}

#[test]
fn cli_disable_does_not_affect_other_rules() {
    let output = run_gdeye_with_args(
        "perf_process_allocation.gd",
        &["--disable", "correctness/dead-store"],
    );
    assert!(
        count_rule(&output, "perf/allocation") > 0,
        "Other rules should still fire.\nOutput:\n{}",
        output
    );
}

#[test]
fn cli_disable_multiple_rules() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &[
            "--disable",
            "correctness/dead-store",
            "--disable",
            "correctness/unused-signal",
        ],
    );
    assert_eq!(count_rule(&output, "correctness/dead-store"), 0);
    assert_eq!(count_rule(&output, "correctness/unused-signal"), 0);
}

// --- Suppression comments ---

#[test]
fn suppression_unsuppressed_still_warns() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        has_message(&output, "`unused_no_suppress`"),
        "Unsuppressed variable should still warn.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_ignore_next_line_blanket() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_blanket_next`"),
        "gdeye:ignore-next-line should suppress.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_ignore_same_line_specific_rule() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_specific_same`"),
        "gdeye:ignore with specific rule should suppress same line.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_ignore_next_line_specific_rule() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_specific_next`"),
        "gdeye:ignore-next-line with specific rule should suppress.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_wrong_rule_does_not_suppress() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        has_message(&output, "`unused_wrong_rule`"),
        "gdeye:ignore-next-line with wrong rule ID should not suppress.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_ignore_same_line_blanket() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_inline_blanket`"),
        "gdeye:ignore on same line should suppress.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_signal_same_line() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_signal_suppressed`"),
        "gdeye:ignore should suppress signal warning.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "`unused_signal_not_suppressed`"),
        "Unsuppressed signal should still warn.\nOutput:\n{}",
        output
    );
}

#[test]
fn suppression_parameter_same_line() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        !has_message(&output, "`unused_param`"),
        "gdeye:ignore should suppress parameter warning.\nOutput:\n{}",
        output
    );
    assert!(
        has_message(&output, "`unused_param2`"),
        "Unsuppressed parameter should still warn.\nOutput:\n{}",
        output
    );
}

// --- Output format: JSON ---

#[test]
fn format_json_outputs_array() {
    let output = run_gdeye_stdout(
        "correctness_unused_parameter.gd",
        &[
            "--format",
            "json",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/naming-convention",
            "--disable",
            "correctness/unused-function",
            "--disable",
            "correctness/dead-store",
        ],
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("JSON output should be valid JSON");
    assert!(parsed.is_array(), "JSON output should be an array");
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2, "Should have 2 diagnostics");
}

#[test]
fn format_json_contains_rule_and_message() {
    let output = run_gdeye_stdout("correctness_unused_parameter.gd", &["--format", "json"]);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let first = &parsed[0];
    assert_eq!(first["rule"], "correctness/unused-parameter");
    assert_eq!(first["severity"], "info");
    assert!(first["message"].as_str().unwrap().contains("unused_param"));
    assert!(first["line"].as_u64().unwrap() > 0);
}

#[test]
fn format_json_no_stderr_diagnostics() {
    let output = run_gdeye_with_args("correctness_unused_parameter.gd", &["--format", "json"]);
    assert!(
        !output.contains("Warning:"),
        "JSON format should not emit text diagnostics to stderr.\nStderr:\n{}",
        output
    );
}

// --- Output format: SARIF ---

#[test]
fn format_sarif_valid_structure() {
    let output = run_gdeye_stdout("correctness_unused_parameter.gd", &["--format", "sarif"]);
    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("SARIF output should be valid JSON");
    assert_eq!(parsed["version"], "2.1.0");
    assert!(parsed["runs"].is_array());
    let run = &parsed["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "gdeye");
    assert!(run["tool"]["driver"]["rules"].is_array());
}

#[test]
fn format_sarif_contains_results() {
    let output = run_gdeye_stdout(
        "correctness_unused_parameter.gd",
        &[
            "--format",
            "sarif",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/naming-convention",
            "--disable",
            "correctness/unused-function",
            "--disable",
            "correctness/dead-store",
        ],
    );
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let results = parsed["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["ruleId"], "correctness/unused-parameter");
    assert_eq!(results[0]["level"], "note");
    assert!(
        results[0]["locations"][0]["physicalLocation"]["region"]["startLine"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn format_sarif_rules_match_registered() {
    let output = run_gdeye_stdout("correctness_unused_parameter.gd", &["--format", "sarif"]);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let rules = parsed["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap();
    assert!(rules.len() >= 5, "SARIF should list all registered rules");
}

// --- Compact format tests ---

#[test]
fn format_compact_one_line_per_issue() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &[
            "--format",
            "compact",
            "--disable",
            "style/untyped-parameter",
            "--disable",
            "style/untyped-return",
            "--disable",
            "style/naming-convention",
            "--disable",
            "correctness/unused-function",
        ],
    );
    let lines: Vec<&str> = output
        .lines()
        .filter(|l| !l.starts_with("Checked "))
        .collect();
    assert_eq!(
        lines.len(),
        2,
        "Expected 2 lines of compact output (unused_member, unused_local), got {}:\n{}",
        lines.len(),
        output
    );
    for line in &lines {
        assert!(
            line.contains(": warning ["),
            "Each line should contain severity and rule bracket: {}",
            line
        );
    }
}

#[test]
fn format_compact_includes_path_line_col() {
    let output = run_gdeye_with_args("correctness_unused_variable.gd", &["--format", "compact"]);
    let first_line = output.lines().next().unwrap();
    assert!(
        first_line.contains("correctness_unused_variable.gd:4:0: warning [correctness/dead-store]"),
        "Compact line should have path:line:col: severity [rule] format.\nGot: {}",
        first_line
    );
}

#[test]
fn format_compact_no_summary() {
    let output = run_gdeye_with_args("correctness_unused_variable.gd", &["--format", "compact"]);
    assert!(
        !output.contains("Found"),
        "Compact format should not include a summary line.\nOutput:\n{}",
        output
    );
}

// --- Text format summary tests ---

#[test]
fn format_text_summary_shows_counts() {
    let output = run_gdeye("correctness_unused_variable.gd");
    assert!(
        output.contains("Found 2 warnings"),
        "Text output should end with a summary line.\nOutput:\n{}",
        output
    );
}

#[test]
fn format_text_summary_mixed_severities() {
    let output = run_gdeye("suppression_comments.gd");
    assert!(
        output.contains("Found") && output.contains("warning") && output.contains("info"),
        "Text summary should include both warning and info counts.\nOutput:\n{}",
        output
    );
}

// --- ClassDB mode tests ---

#[test]
fn target_version_flag_works() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &["--target-version", "4.5"],
    );
    assert!(
        output.contains("unused_local"),
        "--target-version 4.5 should still produce diagnostics.\nOutput:\n{}",
        output
    );
}

#[test]
fn target_version_exact_flag_works() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &["--target-version", "4.5.1"],
    );
    assert!(
        output.contains("unused_local"),
        "--target-version 4.5.1 should still produce diagnostics.\nOutput:\n{}",
        output
    );
}

// --- Autoload injection ---

#[test]
fn autoload_usage_no_false_positives() {
    let output = run_gdeye("autoload_usage.gd");
    assert!(
        !has_message(&output, "GameManager"),
        "Autoload names should not produce warnings.\nOutput:\n{}",
        output
    );
    assert!(
        !has_message(&output, "EventBus"),
        "Autoload names should not produce warnings.\nOutput:\n{}",
        output
    );
}

#[test]
fn autoload_unused_local_still_warns() {
    let output = run_gdeye("autoload_usage.gd");
    assert!(
        has_message(&output, "`unused_var`"),
        "Unused local variable should still be flagged with autoloads present.\nOutput:\n{}",
        output
    );
}

// --- Parse error handling tests ---

#[test]
fn parse_error_reports_error_message() {
    let output = run_gdeye("parse_error_project");
    assert!(
        has_message(&output, "Failed to parse") && has_message(&output, "binary.gd"),
        "Should report parse failure for binary file.\nOutput:\n{}",
        output
    );
}

#[test]
fn parse_error_continues_analysis() {
    let output = run_gdeye("parse_error_project");
    assert!(
        has_message(&output, "unused_in_valid_file"),
        "Should still analyze valid.gd despite binary.gd parse failure.\nOutput:\n{}",
        output
    );
}

// --- CLI subcommand and format tests ---

#[test]
fn subcommand_rules_lists_rules() {
    let (stdout, _, success) = run_gdeye_subcommand(&["rules"]);
    assert!(success, "rules subcommand should succeed");
    assert!(stdout.contains("perf/allocation"));
    assert!(stdout.contains("correctness/dead-store"));
    assert!(stdout.contains("style/naming-convention"));
}

#[test]
fn subcommand_dump_ast() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (stdout, _, success) = run_gdeye_subcommand(&["dump-ast", fixture.to_str().unwrap()]);
    assert!(success, "dump-ast should succeed");
    assert!(stdout.contains("source"));
    assert!(stdout.contains("function_definition"));
}

#[test]
fn output_format_compact() {
    let output = run_gdeye_with_args(
        "correctness_unused_variable.gd",
        &[
            "--format",
            "compact",
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
    assert!(
        output.contains("correctness/dead-store"),
        "Compact output should contain the rule.\nOutput:\n{}",
        output
    );
}

#[test]
fn output_format_json() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (stdout, _, _) = run_gdeye_subcommand(&[
        "--format",
        "json",
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
        fixture.to_str().unwrap(),
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "JSON output should be valid JSON: {}\nOutput:\n{}",
            e, stdout
        )
    });
    assert!(parsed.is_array());
}

#[test]
fn output_format_sarif() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (stdout, _, _) = run_gdeye_subcommand(&[
        "--format",
        "sarif",
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
        fixture.to_str().unwrap(),
    ]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "SARIF output should be valid JSON: {}\nOutput:\n{}",
            e, stdout
        )
    });
    assert!(parsed.get("$schema").is_some() || parsed.get("runs").is_some());
}

#[test]
fn fix_flag_removes_unused_var() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let source = std::fs::read_to_string(&fixture).unwrap();

    let tmp_dir = std::env::temp_dir().join("gdeye_fix_test");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let tmp_file = tmp_dir.join("test_fix.gd");
    std::fs::write(&tmp_file, &source).unwrap();

    let binary = env!("CARGO_BIN_EXE_gdeye");
    // Use --unsafe to apply destructive fixes like removing unused variables
    let output = Command::new(binary)
        .args([
            "--fix",
            "--unsafe",
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
        ])
        .arg(&tmp_file)
        .output()
        .expect("Failed to run gdeye --fix --unsafe");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Fixed"),
        "Should report fixes applied.\nStderr:\n{}",
        stderr
    );

    let fixed_source = std::fs::read_to_string(&tmp_file).unwrap();
    assert!(
        !fixed_source.contains("unused_var"),
        "Fixed source should not contain the unused variable.\nFixed:\n{}",
        fixed_source
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn check_subcommand_works() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, stderr, _) = run_gdeye_subcommand(&[
        "check",
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
        fixture.to_str().unwrap(),
    ]);
    assert!(
        stderr.contains("dead-store"),
        "check subcommand should find dead stores.\nStderr:\n{}",
        stderr
    );
}

#[test]
fn no_gd_files_reports_message() {
    let tmp_dir = std::env::temp_dir().join("gdeye_empty_test");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let (_, stderr, success) = run_gdeye_subcommand(&[tmp_dir.to_str().unwrap()]);
    assert!(success, "Should exit 0 when no .gd files found");
    assert!(
        stderr.contains("No .gd files found"),
        "Should report no files found.\nStderr:\n{}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn target_version_cli_flag() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, stderr, _) = run_gdeye_subcommand(&[
        "--target-version",
        "4.5",
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
        fixture.to_str().unwrap(),
    ]);
    assert!(
        !stderr.contains("Error"),
        "Should not produce errors with --target-version 4.5.\nStderr:\n{}",
        stderr
    );
}

#[test]
fn invalid_target_version_exits_with_error() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, stderr, success) =
        run_gdeye_subcommand(&["--target-version", "99.99", fixture.to_str().unwrap()]);
    assert!(!success, "Invalid version should exit non-zero");
    assert!(
        stderr.contains("No bundled ClassDB matching version"),
        "Should report version not found.\nStderr:\n{}",
        stderr
    );
}

// --- Comprehensive pattern tests (CFG, symbols, loops) ---

#[test]
fn comprehensive_patterns_no_panics() {
    let output = run_gdeye_with_args("comprehensive_patterns.gd", &[]);
    assert!(
        !output.contains("panicked") && !output.contains("thread"),
        "Should not panic on comprehensive patterns.\nOutput:\n{}",
        output
    );
}

#[test]
fn comprehensive_unused_signal() {
    let output = run_gdeye("comprehensive_patterns.gd");
    assert!(
        has_message(&output, "unused_signal_test"),
        "Should flag unused signal.\nOutput:\n{}",
        output
    );
}

#[test]
fn comprehensive_shadowed_variable() {
    let output = run_gdeye("comprehensive_patterns.gd");
    assert!(
        has_message(&output, "shadow_target") && has_message(&output, "shadows"),
        "Should flag shadowed variable.\nOutput:\n{}",
        output
    );
}

#[test]
fn comprehensive_unreachable_after_break() {
    let output = run_gdeye("comprehensive_patterns.gd");
    assert!(
        count_rule(&output, "correctness/unreachable-code") >= 2,
        "Should flag unreachable code after break and continue.\nOutput:\n{}",
        output
    );
}

// --- --fail-on tests ---

#[test]
fn fail_on_error_exits_zero_for_warnings() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, _, success) = run_gdeye_subcommand(&["--fail-on", "error", fixture.to_str().unwrap()]);
    assert!(
        success,
        "--fail-on error should exit 0 when only warnings are present"
    );
}

#[test]
fn fail_on_warning_exits_nonzero_for_warnings() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, _, success) =
        run_gdeye_subcommand(&["--fail-on", "warning", fixture.to_str().unwrap()]);
    assert!(
        !success,
        "--fail-on warning should exit 1 when warnings are present"
    );
}

#[test]
fn fail_on_info_exits_nonzero_for_any_diagnostic() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, _, success) = run_gdeye_subcommand(&["--fail-on", "info", fixture.to_str().unwrap()]);
    assert!(
        !success,
        "--fail-on info should exit 1 when any diagnostics are present"
    );
}

#[test]
fn fail_on_default_exits_nonzero_for_warnings() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, _, success) = run_gdeye_subcommand(&[fixture.to_str().unwrap()]);
    assert!(
        !success,
        "default (no --fail-on) should exit 1 when diagnostics are present"
    );
}

#[test]
fn fail_on_error_still_reports_warnings() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, stderr, success) =
        run_gdeye_subcommand(&["--fail-on", "error", fixture.to_str().unwrap()]);
    assert!(success, "should exit 0");
    assert!(
        stderr.contains("unused"),
        "--fail-on error should still report warnings in output.\nStderr:\n{}",
        stderr
    );
}

#[test]
fn fail_on_check_subcommand() {
    let fixture = fixture_path("correctness_unused_variable.gd");
    let (_, _, success) =
        run_gdeye_subcommand(&["check", "--fail-on", "error", fixture.to_str().unwrap()]);
    assert!(
        success,
        "--fail-on error via check subcommand should exit 0 for warnings"
    );
}

// --- Cross-file usage detection tests ---

#[test]
fn cross_file_unused_member_flagged() {
    let output = run_gdeye("cross_file_usage");
    assert!(
        has_message(&output, "`unused_stat`"),
        "Should flag genuinely unused member variable `unused_stat`.\nOutput:\n{}",
        output
    );
}

#[test]
fn cross_file_used_member_not_flagged() {
    let output = run_gdeye("cross_file_usage");
    let unused_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("dead-store"))
        .collect();
    for line in &unused_lines {
        assert!(
            !line.contains("`health`"),
            "Should NOT flag `health` -- used via cross-file attribute access.\nLines:\n{}",
            unused_lines.join("\n")
        );
    }
}

#[test]
fn cross_file_scene_signal_not_flagged() {
    let output = run_gdeye("cross_file_usage");
    let signal_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("unused-signal"))
        .collect();
    for line in &signal_lines {
        assert!(
            !line.contains("`died`"),
            "Should NOT flag `died` signal -- connected in scene file.\nLines:\n{}",
            signal_lines.join("\n")
        );
    }
}

#[test]
fn cross_file_scene_property_marks_used() {
    let output = run_gdeye("cross_file_usage");
    let unused_lines: Vec<&str> = output
        .lines()
        .filter(|l| l.contains("dead-store"))
        .collect();
    for line in &unused_lines {
        assert!(
            !line.contains("`speed`"),
            "Should NOT flag `speed` -- set as property in scene file.\nLines:\n{}",
            unused_lines.join("\n")
        );
    }
}
