use std::path::{Path, PathBuf};

use ariadne::{Color, Label, Report, ReportKind, Source};
use serde_json::{json, Value};

use crate::analysis::SeverityCounts;
use crate::fix::FixCounts;
use crate::rules::{self, Diagnostic, Severity};
use crate::util::LineIndex;

/// Emit diagnostics to stderr using ariadne for pretty-printed source spans.
pub fn emit_diagnostics(path: &Path, source: &str, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        return;
    }

    let path_str = path.display().to_string();
    let line_index = LineIndex::new(source);

    for diag in diagnostics {
        let kind = match diag.severity {
            Severity::Error => ReportKind::Error,
            Severity::Warning => ReportKind::Warning,
            Severity::Info => ReportKind::Advice,
        };

        let color = match diag.severity {
            Severity::Error => Color::Red,
            Severity::Warning => Color::Yellow,
            Severity::Info => Color::Cyan,
        };

        // Convert line/col to byte offset using pre-computed index
        let offset = line_index.line_col_to_offset(diag.line, diag.col);
        let end_offset =
            if diag.end_line > 0 && (diag.end_line != diag.line || diag.end_col != diag.col) {
                line_index.line_col_to_offset(diag.end_line, diag.end_col)
            } else {
                // Point to end of line if no span
                source[offset..]
                    .find('\n')
                    .map(|i| offset + i)
                    .unwrap_or(source.len())
            };

        let span = offset..end_offset.max(offset + 1);

        let mut builder = Report::build(kind, (&path_str, span.clone()))
            .with_message(format!("[{}] {}", diag.rule, diag.message))
            .with_label(
                Label::new((&path_str, span))
                    .with_message(&diag.message)
                    .with_color(color),
            );

        for label in &diag.labels {
            let l_offset = line_index.line_col_to_offset(label.line, label.col);
            let l_end = if label.end_line > 0
                && (label.end_line != label.line || label.end_col != label.col)
            {
                line_index.line_col_to_offset(label.end_line, label.end_col)
            } else {
                source[l_offset..]
                    .find('\n')
                    .map(|i| l_offset + i)
                    .unwrap_or(source.len())
            };
            let l_span = l_offset..l_end.max(l_offset + 1);
            builder = builder.with_label(
                Label::new((&path_str, l_span))
                    .with_message(&label.message)
                    .with_color(Color::Blue),
            );
        }

        if let Some(ref note) = diag.note {
            builder = builder.with_note(note);
        }

        let report = builder.finish();

        report
            .eprint((&path_str, Source::from(source)))
            .unwrap_or_else(|_| {
                // Fallback if ariadne fails
                eprintln!(
                    "{}:{}:{}: {} [{}] {}",
                    path_str, diag.line, diag.col, diag.severity, diag.rule, diag.message
                );
            });
    }
}

/// Emit diagnostics in compact one-line-per-issue format to stderr.
/// Format: path:line:col: severity [rule] message
pub fn emit_compact(path: &Path, diagnostics: &[Diagnostic]) {
    let path_str = path.display().to_string();
    for diag in diagnostics {
        eprintln!(
            "{}:{}:{}: {} [{}] {}",
            path_str, diag.line, diag.col, diag.severity, diag.rule, diag.message
        );
    }
}

/// Emit a summary line showing counts by severity and fixable issues.
pub fn emit_summary(counts: &SeverityCounts, fix_counts: &FixCounts) {
    let mut parts = Vec::new();
    if counts.errors > 0 {
        parts.push(format!(
            "{} error{}",
            counts.errors,
            if counts.errors == 1 { "" } else { "s" }
        ));
    }
    if counts.warnings > 0 {
        parts.push(format!(
            "{} warning{}",
            counts.warnings,
            if counts.warnings == 1 { "" } else { "s" }
        ));
    }
    if counts.infos > 0 {
        parts.push(format!("{} info", counts.infos));
    }
    if !parts.is_empty() {
        eprintln!("\nFound {}", parts.join(", "));
    }

    // Show fix availability
    if fix_counts.has_any() {
        let mut fix_parts = Vec::new();
        if fix_counts.safe > 0 {
            fix_parts.push(format!("{} auto-fixable (--fix)", fix_counts.safe));
        }
        if fix_counts.unsafe_ > 0 {
            fix_parts.push(format!("{} with --fix --unsafe", fix_counts.unsafe_));
        }
        eprintln!("  {}", fix_parts.join(", "));
    }
}

/// Emit all diagnostics as a JSON array to stdout.
pub fn emit_json(file_diagnostics: &[(PathBuf, Vec<Diagnostic>)]) {
    let mut results: Vec<Value> = Vec::new();

    for (path, diagnostics) in file_diagnostics {
        let path_str = path.display().to_string();
        for diag in diagnostics {
            results.push(json!({
                "file": path_str,
                "rule": diag.rule,
                "severity": format!("{}", diag.severity),
                "message": diag.message,
                "line": diag.line,
                "column": diag.col,
                "endLine": diag.end_line,
                "endColumn": diag.end_col,
            }));
        }
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string())
    );
}

/// Emit all diagnostics in SARIF v2.1.0 format to stdout.
pub fn emit_sarif(file_diagnostics: &[(PathBuf, Vec<Diagnostic>)]) {
    let mut results: Vec<Value> = Vec::new();

    for (path, diagnostics) in file_diagnostics {
        let path_str = path.display().to_string();
        for diag in diagnostics {
            results.push(json!({
                "ruleId": diag.rule,
                "level": sarif_level(diag.severity),
                "message": {
                    "text": diag.message
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": path_str
                        },
                        "region": {
                            "startLine": diag.line,
                            "startColumn": diag.col + 1, // SARIF uses 1-based columns
                            "endLine": diag.end_line,
                            "endColumn": diag.end_col + 1,
                        }
                    }
                }]
            }));
        }
    }

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "gdeye",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/tktech/gdeye",
                    "rules": sarif_rule_descriptors()
                }
            },
            "results": results
        }]
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&sarif).unwrap_or_else(|_| "{}".to_string())
    );
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

fn sarif_rule_descriptors() -> Vec<Value> {
    rules::all_rules()
        .iter()
        .map(|r| {
            json!({
                "id": r.id(),
                "shortDescription": { "text": r.description() }
            })
        })
        .collect()
}
