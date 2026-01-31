#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

pub fn run_gdeye(fixture: &str) -> String {
    // Disable style rules and unused-function by default so correctness/perf tests aren't affected
    run_gdeye_with_args(
        fixture,
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
            "--disable",
            "style/unnecessary-pass",
            "--disable",
            "style/standalone-expression",
            "--disable",
            "style/no-else-return",
            "--disable",
            "correctness/unused-function",
        ],
    )
}

pub fn run_gdeye_with_args(fixture: &str, extra_args: &[&str]) -> String {
    let binary = env!("CARGO_BIN_EXE_gdeye");
    let output = Command::new(binary)
        .args(extra_args)
        .arg(fixture_path(fixture))
        .output()
        .expect("Failed to run gdeye");
    let raw = String::from_utf8_lossy(&output.stderr).to_string();
    strip_ansi(&raw)
}

pub fn run_gdeye_stdout(fixture: &str, extra_args: &[&str]) -> String {
    let binary = env!("CARGO_BIN_EXE_gdeye");
    let output = Command::new(binary)
        .args(extra_args)
        .arg(fixture_path(fixture))
        .output()
        .expect("Failed to run gdeye");
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn run_gdeye_subcommand(args: &[&str]) -> (String, String, bool) {
    let binary = env!("CARGO_BIN_EXE_gdeye");
    let output = Command::new(binary)
        .args(args)
        .output()
        .expect("Failed to run gdeye");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

pub fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            result.push(ch);
        }
    }
    result
}

pub fn count_rule(output: &str, rule: &str) -> usize {
    output.lines().filter(|l| l.contains(rule)).count()
}

pub fn has_message(output: &str, msg: &str) -> bool {
    output.contains(msg)
}

pub fn run_gdeye_style(fixture: &str) -> String {
    run_gdeye_with_args(fixture, &[])
}
