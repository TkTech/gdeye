mod correctness;
pub(crate) mod helpers;
mod performance;
mod style;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::call_graph::CallGraph;
use crate::cfg::Cfg;
use crate::classdb::ClassDb;
use crate::config::Config;
use crate::flow::FlowResults;
use crate::parser::ParsedFile;
use crate::project::ProjectInfo;
use crate::scene::SceneFile;
use crate::symbols::FileSymbols;

/// A single text replacement to apply as part of a fix.
#[derive(Debug, Clone)]
pub struct TextEdit {
    pub start_byte: usize,
    pub end_byte: usize,
    pub replacement: String,
}

/// An auto-fix that can be applied to resolve a diagnostic.
#[derive(Debug, Clone)]
pub struct Fix {
    #[allow(dead_code)] // For display in UI/CLI
    pub description: String,
    pub edits: Vec<TextEdit>,
    /// Whether this fix requires --unsafe to apply (e.g., removing code entirely).
    pub is_unsafe: bool,
}

impl Fix {
    /// Create a new safe fix.
    pub fn new(description: impl Into<String>, edits: Vec<TextEdit>) -> Self {
        Self {
            description: description.into(),
            edits,
            is_unsafe: false,
        }
    }

    /// Create an unsafe fix (requires --unsafe flag to apply).
    pub fn new_unsafe(description: impl Into<String>, edits: Vec<TextEdit>) -> Self {
        Self {
            description: description.into(),
            edits,
            is_unsafe: true,
        }
    }
}

/// A secondary label to display on the diagnostic.
#[derive(Debug, Clone)]
pub struct DiagLabel {
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// A diagnostic produced by a lint rule.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub end_line: usize,
    pub end_col: usize,
    pub fix: Option<Fix>,
    pub labels: Vec<DiagLabel>,
    pub note: Option<String>,
}

impl Diagnostic {
    pub fn new(
        rule: &'static str,
        severity: Severity,
        message: impl Into<String>,
        line: usize,
    ) -> Self {
        Diagnostic {
            rule,
            severity,
            message: message.into(),
            line,
            col: 0,
            end_line: line,
            end_col: 0,
            fix: None,
            labels: vec![],
            note: None,
        }
    }

    pub fn span(mut self, col: usize, end_line: usize, end_col: usize) -> Self {
        self.col = col;
        self.end_line = end_line;
        self.end_col = end_col;
        self
    }

    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn with_label(mut self, label: DiagLabel) -> Self {
        self.labels.push(label);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// Context passed to each rule's `check` method.
pub struct RuleContext<'a> {
    pub path: &'a Path,
    pub parsed: &'a ParsedFile,
    pub file_sym: &'a FileSymbols,
    pub all_file_symbols: &'a [FileSymbols],
    #[allow(dead_code)] // Available for rules that need CFG access
    pub cfgs: &'a [Cfg],
    pub flow_results: &'a FlowResults,
    pub scenes: &'a HashMap<PathBuf, SceneFile>,
    pub class_db: &'a ClassDb,
    pub config: &'a Config,
    pub project_info: &'a ProjectInfo,
    pub call_graph: &'a CallGraph,
    /// Functions reachable from entry points. Used for dead code detection.
    pub reachable_functions: &'a Arc<HashSet<(PathBuf, String)>>,
}

/// Describes a configurable option for a rule.
#[derive(Debug, Clone)]
pub struct RuleOption {
    /// The option key as used in gdeye.toml (e.g., "max_length").
    pub name: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// String representation of the default value.
    pub default: &'static str,
    /// The expected value type.
    pub value_type: OptionType,
}

/// The type of a rule option value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    Integer,
    #[allow(dead_code)] // For future rule options
    String,
    #[allow(dead_code)] // For future rule options
    Boolean,
}

impl std::fmt::Display for OptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionType::Integer => write!(f, "integer"),
            OptionType::String => write!(f, "string"),
            OptionType::Boolean => write!(f, "boolean"),
        }
    }
}

/// Trait implemented by each lint rule.
pub trait Rule {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn category(&self) -> &'static str;
    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic>;

    /// Return the configurable options for this rule.
    fn options(&self) -> Vec<RuleOption> {
        vec![]
    }
}

/// Return all registered rule instances.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        // Performance rules
        Box::new(performance::Allocation),
        Box::new(performance::ProcessGetNode),
        Box::new(performance::LoopInvariant),
        Box::new(performance::StringConcatLoop),
        // Correctness rules
        Box::new(correctness::DeadStore),
        Box::new(correctness::UnusedParameter),
        Box::new(correctness::UnusedSignal),
        Box::new(correctness::UnusedFunction),
        Box::new(correctness::UnreachableCode),
        Box::new(correctness::ShadowedVariable),
        Box::new(correctness::MissingReturn),
        Box::new(correctness::BrokenNodePath),
        Box::new(correctness::SignalSignatureMismatch),
        Box::new(correctness::TypeMismatch),
        Box::new(correctness::ReturnTypeMismatch),
        Box::new(correctness::ComparisonWithItself),
        Box::new(correctness::DuplicateDictKey),
        Box::new(correctness::SelfAssignment),
        Box::new(correctness::AwaitInLoop),
        Box::new(correctness::DuplicatedLoad),
        Box::new(correctness::UninitializedVariable),
        Box::new(correctness::PrivateAccess),
        Box::new(correctness::AwaitCorrectness),
        Box::new(correctness::InvalidInputAction),
        Box::new(correctness::NullAccess),
        Box::new(correctness::OrphanNode),
        Box::new(correctness::CircularPreload),
        Box::new(correctness::AutoloadOrder),
        Box::new(correctness::MatchExhaustiveness),
        // Style rules
        Box::new(style::NamingConvention),
        Box::new(style::UntypedParameter),
        Box::new(style::UntypedReturn),
        Box::new(style::FunctionTooLong),
        Box::new(style::ExcessiveNesting),
        Box::new(style::UnnecessaryPass),
        Box::new(style::StandaloneExpression),
        Box::new(style::NoElseReturn),
        Box::new(style::OnreadyHoist),
        Box::new(style::UntypedVariable),
    ]
}

/// Run all lint rules on a file, filtering by config.
pub fn run_all(ctx: &RuleContext) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for rule in all_rules() {
        if !ctx.config.is_rule_enabled(rule.id()) {
            continue;
        }
        diagnostics.extend(rule.check(ctx));
    }

    // Filter disabled rules and apply severity overrides
    diagnostics.retain_mut(
        |d| match ctx.config.effective_severity(d.rule, d.severity) {
            None => false,
            Some(sev) => {
                d.severity = sev;
                true
            }
        },
    );

    // Filter suppressed diagnostics via inline comments
    let suppressions = parse_suppressions(ctx.parsed.source());
    diagnostics.retain(|d| !suppressions.is_suppressed(d.line, d.rule));

    // Sort by line number
    diagnostics.sort_by_key(|d| (d.line, d.col));

    diagnostics
}

/// A suppression entry parsed from a comment.
#[derive(Debug)]
struct Suppression {
    /// The line number this suppression applies to (1-based).
    line: usize,
    /// The rule ID to suppress, or None for all rules.
    rule: Option<String>,
}

/// Collection of parsed suppressions from source comments.
#[derive(Debug)]
struct Suppressions {
    entries: Vec<Suppression>,
    /// Lines where all rules are suppressed.
    blanket_lines: HashSet<usize>,
}

impl Suppressions {
    fn is_suppressed(&self, line: usize, rule: &str) -> bool {
        if self.blanket_lines.contains(&line) {
            return true;
        }
        self.entries
            .iter()
            .any(|s| s.line == line && s.rule.as_deref() == Some(rule))
    }
}

/// Parse suppression comments from source text.
///
/// Supported formats:
///   `# gdeye:ignore` — suppress all rules on this line
///   `# gdeye:ignore rule-id` — suppress specific rule on this line
///   `# gdeye:ignore-next-line` — suppress all rules on the next line
///   `# gdeye:ignore-next-line rule-id` — suppress specific rule on the next line
fn parse_suppressions(source: &str) -> Suppressions {
    let mut entries = Vec::new();
    let mut blanket_lines = HashSet::new();

    for (idx, line_text) in source.lines().enumerate() {
        let line_num = idx + 1; // 1-based

        // Find a comment on this line
        let comment_start = match line_text.find('#') {
            Some(pos) => pos,
            None => continue,
        };

        let comment = line_text[comment_start + 1..].trim();

        if let Some(rest) = comment.strip_prefix("gdeye:ignore-next-line") {
            let rest = rest.trim();
            let target_line = line_num + 1;
            if rest.is_empty() {
                blanket_lines.insert(target_line);
            } else {
                entries.push(Suppression {
                    line: target_line,
                    rule: Some(rest.to_string()),
                });
            }
        } else if let Some(rest) = comment.strip_prefix("gdeye:ignore") {
            let rest = rest.trim();
            if rest.is_empty() {
                blanket_lines.insert(line_num);
            } else {
                entries.push(Suppression {
                    line: line_num,
                    rule: Some(rest.to_string()),
                });
            }
        }
    }

    Suppressions {
        entries,
        blanket_lines,
    }
}

/// Print all available rules, optionally filtered to a single rule with detail.
pub fn print_rules(filter: Option<&str>) {
    let rules = all_rules();

    if let Some(rule_id) = filter {
        let rule = rules.iter().find(|r| r.id() == rule_id);
        match rule {
            Some(r) => print_rule_detail(r.as_ref()),
            None => {
                eprintln!("Unknown rule: {}", rule_id);
                std::process::exit(1);
            }
        }
        return;
    }

    let max_id_len = rules.iter().map(|r| r.id().len()).max().unwrap_or(0);

    println!(
        "{:<width$}  {:<10}  Description",
        "Rule",
        "Severity",
        width = max_id_len
    );
    println!("{}", "-".repeat(max_id_len + 30));

    for rule in &rules {
        println!(
            "{:<width$}  {:<10}  {}",
            rule.id(),
            format!("{}", rule.default_severity()),
            rule.description(),
            width = max_id_len
        );
    }
}

/// Print detailed information about a single rule, including its options.
fn print_rule_detail(rule: &dyn Rule) {
    println!("Rule:        {}", rule.id());
    println!("Category:    {}", rule.category());
    println!("Severity:    {}", rule.default_severity());
    println!("Description: {}", rule.description());

    let options = rule.options();
    if options.is_empty() {
        println!("\nNo configurable options.");
    } else {
        println!("\nOptions:");
        let max_name = options.iter().map(|o| o.name.len()).max().unwrap_or(0);
        for opt in &options {
            println!(
                "  {:<width$}  {} (default: {}) — {}",
                opt.name,
                opt.value_type,
                opt.default,
                opt.description,
                width = max_name,
            );
        }

        println!("\nExample gdeye.toml:");
        println!("  [rules.\"{}\"]", rule.id());
        for opt in &options {
            println!("  {} = {}", opt.name, opt.default);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppression_blanket_same_line() {
        let source = "var x = 1 # gdeye:ignore\nvar y = 2\n";
        let s = parse_suppressions(source);
        assert!(s.is_suppressed(1, "correctness/unused-variable"));
        assert!(s.is_suppressed(1, "any/rule"));
        assert!(!s.is_suppressed(2, "correctness/unused-variable"));
    }

    #[test]
    fn suppression_specific_same_line() {
        let source = "var x = 1 # gdeye:ignore correctness/unused-variable\n";
        let s = parse_suppressions(source);
        assert!(s.is_suppressed(1, "correctness/unused-variable"));
        assert!(!s.is_suppressed(1, "other/rule"));
    }

    #[test]
    fn suppression_blanket_next_line() {
        let source = "# gdeye:ignore-next-line\nvar x = 1\nvar y = 2\n";
        let s = parse_suppressions(source);
        assert!(!s.is_suppressed(1, "any/rule"));
        assert!(s.is_suppressed(2, "any/rule"));
        assert!(!s.is_suppressed(3, "any/rule"));
    }

    #[test]
    fn suppression_specific_next_line() {
        let source = "# gdeye:ignore-next-line correctness/unused-variable\nvar x = 1\n";
        let s = parse_suppressions(source);
        assert!(s.is_suppressed(2, "correctness/unused-variable"));
        assert!(!s.is_suppressed(2, "other/rule"));
    }

    #[test]
    fn suppression_wrong_rule_not_suppressed() {
        let source = "# gdeye:ignore-next-line perf/process-allocation\nvar x = 1\n";
        let s = parse_suppressions(source);
        assert!(!s.is_suppressed(2, "correctness/unused-variable"));
    }

    #[test]
    fn suppression_multiple_comments() {
        let source = "# gdeye:ignore-next-line\nvar a = 1\nvar b = 2 # gdeye:ignore\nvar c = 3\n";
        let s = parse_suppressions(source);
        assert!(s.is_suppressed(2, "any/rule"));
        assert!(s.is_suppressed(3, "any/rule"));
        assert!(!s.is_suppressed(4, "any/rule"));
    }

    #[test]
    fn suppression_comment_only_line() {
        let source = "# gdeye:ignore\n";
        let s = parse_suppressions(source);
        // A comment-only line with gdeye:ignore suppresses its own line
        assert!(s.is_suppressed(1, "any/rule"));
    }

    #[test]
    fn suppression_no_comments() {
        let source = "var x = 1\nvar y = 2\n";
        let s = parse_suppressions(source);
        assert!(!s.is_suppressed(1, "any/rule"));
        assert!(!s.is_suppressed(2, "any/rule"));
    }
}
