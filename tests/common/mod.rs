#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Run gdeye with only the specified rule(s) enabled.
/// This is the preferred helper for rule tests — it isolates the test from
/// other rules so adding new rules never causes unrelated test failures.
pub fn run_rule(fixture: &str, rules: &[&str]) -> String {
    let mut args: Vec<&str> = Vec::new();
    for rule in rules {
        args.push("--rule");
        args.push(rule);
    }
    run_gdeye_with_args(fixture, &args)
}

/// Run gdeye with only the specified rule(s) enabled, returning stdout (for JSON/SARIF).
pub fn run_rule_stdout(fixture: &str, rules: &[&str], extra_args: &[&str]) -> String {
    let mut args: Vec<&str> = Vec::new();
    for rule in rules {
        args.push("--rule");
        args.push(rule);
    }
    args.extend_from_slice(extra_args);
    run_gdeye_stdout(fixture, &args)
}

/// Run gdeye with all rules enabled. Prefer `run_rule` for single-rule tests.
pub fn run_gdeye(fixture: &str) -> String {
    run_gdeye_with_args(fixture, &[])
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
