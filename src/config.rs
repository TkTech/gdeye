use std::collections::HashMap;
use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

use crate::rules::Severity;

/// Indentation style for the formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IndentStyle {
    #[default]
    Tabs,
    Spaces,
}

/// Quote style for string literals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum QuoteStyle {
    /// Normalize to double quotes when safe.
    Double,
    /// Normalize to single quotes when safe.
    Single,
    /// Preserve original quote style.
    #[default]
    Preserve,
}

/// Trailing comma behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TrailingComma {
    /// Always add trailing commas in multi-element collections.
    All,
    /// Add trailing commas only when broken across multiple lines (default).
    #[default]
    Multiline,
    /// Never add trailing commas.
    None,
}

/// Formatter configuration options.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FormatterConfig {
    /// Maximum line width before breaking (default: 100).
    pub print_width: usize,
    /// Indentation style: "tabs" or "spaces" (default: "tabs").
    pub indent_style: IndentStyle,
    /// Number of spaces per indentation level when using spaces (default: 4).
    pub indent_size: usize,
    /// Quote style for strings: "double", "single", or "preserve" (default: "preserve").
    pub quote_style: QuoteStyle,
    /// Trailing comma behavior: "all", "multiline", or "none" (default: "multiline").
    pub trailing_comma: TrailingComma,
    /// Maximum consecutive blank lines allowed (default: 2).
    pub max_blank_lines: usize,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            print_width: 100,
            indent_style: IndentStyle::Tabs,
            indent_size: 4,
            quote_style: QuoteStyle::Preserve,
            trailing_comma: TrailingComma::Multiline,
            max_blank_lines: 2,
        }
    }
}

/// Resolved configuration after merging TOML file + CLI arguments.
#[derive(Debug, Clone)]
pub struct Config {
    /// Glob patterns for files to include (empty = include all .gd files)
    pub include: Vec<String>,
    /// Glob patterns for files to exclude
    pub exclude: Vec<String>,
    /// Rules that are completely disabled
    pub disable: Vec<String>,
    /// Run only these rules (if non-empty, all other rules are disabled)
    pub only: Vec<String>,
    /// Per-rule configuration (severity and/or options)
    pub rules: HashMap<String, RuleConfig>,
    /// Target Godot version for ClassDB (e.g., "4.5"). None means auto-detect.
    pub target_version: Option<String>,
    /// Minimum severity level that triggers a non-zero exit code.
    pub fail_on: Severity,
    /// Formatter configuration.
    pub formatter: FormatterConfig,
    /// Compiled include globs
    include_set: Option<GlobSet>,
    /// Compiled exclude globs
    exclude_set: Option<GlobSet>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            include: Vec::new(),
            exclude: Vec::new(),
            disable: Vec::new(),
            only: Vec::new(),
            rules: HashMap::new(),
            target_version: None,
            fail_on: Severity::Info,
            formatter: FormatterConfig::default(),
            include_set: None,
            exclude_set: None,
        }
    }
}

impl Config {
    /// Create a new default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the disabled rules (builder pattern).
    pub fn with_disabled_rules(mut self, rules: Vec<String>) -> Self {
        self.disable = rules;
        self
    }

    /// Set the target Godot version (builder pattern).
    pub fn with_target_version(mut self, version: impl Into<String>) -> Self {
        self.target_version = Some(version.into());
        self
    }

    /// Set the fail-on severity level (builder pattern).
    pub fn with_fail_on(mut self, severity: Severity) -> Self {
        self.fail_on = severity;
        self
    }
}

/// Severity value in config, supporting "off" to disable a rule.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleSeverity {
    Off,
    Info,
    Warning,
    Error,
}

impl RuleSeverity {
    pub fn to_severity(&self) -> Option<Severity> {
        match self {
            RuleSeverity::Off => None,
            RuleSeverity::Info => Some(Severity::Info),
            RuleSeverity::Warning => Some(Severity::Warning),
            RuleSeverity::Error => Some(Severity::Error),
        }
    }
}

/// Per-rule configuration: either a simple severity or a table with options.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RuleConfig {
    Severity(RuleSeverity),
    Options(RuleOptionsMap),
}

/// A rule options table with an optional severity and arbitrary key-value options.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleOptionsMap {
    #[serde(default)]
    pub severity: Option<RuleSeverity>,
    #[serde(flatten)]
    pub options: HashMap<String, toml::Value>,
}

/// Raw TOML representation (before merging with CLI).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ConfigFile {
    include: Vec<String>,
    exclude: Vec<String>,
    disable: Vec<String>,
    rules: HashMap<String, RuleConfig>,
    /// Target Godot version for ClassDB (e.g., "4.5"). Absent means auto-detect.
    target_version: Option<String>,
    /// Minimum severity that triggers a non-zero exit code.
    fail_on: Option<RuleSeverity>,
    /// Formatter configuration.
    #[serde(default)]
    formatter: FormatterConfig,
}

impl Config {
    /// Load config from a `gdeye.toml` file in the given directory without
    /// any CLI overrides. Suitable for LSP and programmatic use.
    pub fn load_from_project(project_root: Option<&Path>) -> Self {
        Self::load(project_root, &CliConfig::default())
    }

    /// Load config from a `gdeye.toml` file in the given directory, then
    /// overlay CLI arguments on top.
    pub fn load(project_root: Option<&Path>, cli: &CliConfig) -> Self {
        let file_config = project_root.and_then(load_toml).unwrap_or_default();

        // Resolve fail_on: CLI > TOML > default (Info = fail on anything)
        let fail_on = if let Some(sev) = cli.fail_on {
            sev
        } else if let Some(ref toml_sev) = file_config.fail_on {
            toml_sev.to_severity().unwrap_or(Severity::Info)
        } else {
            Severity::Info
        };

        let mut config = Config {
            include: file_config.include,
            exclude: file_config.exclude,
            disable: file_config.disable,
            only: Vec::new(),
            rules: file_config.rules,
            target_version: file_config.target_version,
            fail_on,
            formatter: file_config.formatter,
            include_set: None,
            exclude_set: None,
        };

        // CLI overrides: append to include/exclude/disable lists
        if !cli.include.is_empty() {
            config.include = cli.include.clone();
        }
        if !cli.exclude.is_empty() {
            config.exclude.extend(cli.exclude.iter().cloned());
        }
        if !cli.disable.is_empty() {
            config.disable.extend(cli.disable.iter().cloned());
        }
        if !cli.only.is_empty() {
            config.only = cli.only.clone();
        }

        // Compile glob sets
        config.include_set = build_globset(&config.include);
        config.exclude_set = build_globset(&config.exclude);

        config
    }

    /// Check if a file path should be analyzed based on include/exclude rules.
    /// The path should be relative to the project root.
    pub fn should_include(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // If exclude matches, skip the file
        if let Some(ref exclude_set) = self.exclude_set {
            if exclude_set.is_match(path) || exclude_set.is_match(path_str.as_ref()) {
                return false;
            }
        }

        // If include is set, the file must match
        if let Some(ref include_set) = self.include_set {
            return include_set.is_match(path) || include_set.is_match(path_str.as_ref());
        }

        true
    }

    /// Check if a rule is enabled.
    pub fn is_rule_enabled(&self, rule_id: &str) -> bool {
        // If --rule was specified, only run those rules
        if !self.only.is_empty() {
            return self.only.iter().any(|r| r == rule_id);
        }

        // Explicitly disabled
        if self.disable.iter().any(|d| d == rule_id) {
            return false;
        }

        // Disabled via severity override
        if let Some(rule_config) = self.rules.get(rule_id) {
            let severity = match rule_config {
                RuleConfig::Severity(s) => Some(s),
                RuleConfig::Options(opts) => opts.severity.as_ref(),
            };
            if severity == Some(&RuleSeverity::Off) {
                return false;
            }
        }

        true
    }

    /// Get the effective severity for a rule (returns None if disabled).
    pub fn effective_severity(&self, rule_id: &str, default: Severity) -> Option<Severity> {
        if !self.is_rule_enabled(rule_id) {
            return None;
        }
        if let Some(rule_config) = self.rules.get(rule_id) {
            let severity = match rule_config {
                RuleConfig::Severity(s) => Some(s),
                RuleConfig::Options(opts) => opts.severity.as_ref(),
            };
            if let Some(s) = severity {
                return s.to_severity();
            }
        }
        Some(default)
    }

    /// Get a per-rule option value.
    pub fn rule_option(&self, rule_id: &str, key: &str) -> Option<&toml::Value> {
        match self.rules.get(rule_id)? {
            RuleConfig::Options(opts) => opts.options.get(key),
            RuleConfig::Severity(_) => None,
        }
    }
}

/// CLI-provided configuration values that override the TOML file.
#[derive(Debug, Default, Clone)]
pub struct CliConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub disable: Vec<String>,
    pub only: Vec<String>,
    pub fail_on: Option<Severity>,
}

fn load_toml(project_root: &Path) -> Option<ConfigFile> {
    let config_path = project_root.join("gdeye.toml");
    let content = std::fs::read_to_string(&config_path).ok()?;
    match toml::from_str::<ConfigFile>(&content) {
        Ok(config) => Some(config),
        Err(e) => {
            eprintln!("Warning: Failed to parse {}: {}", config_path.display(), e);
            None
        }
    }
}

fn build_globset(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => {
                eprintln!("Warning: Invalid glob pattern '{}': {}", pattern, e);
            }
        }
    }
    builder.build().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_includes_everything() {
        let config = Config::load(None, &CliConfig::default());
        assert!(config.should_include(Path::new("scripts/Player.gd")));
        assert!(config.should_include(Path::new("addons/plugin/main.gd")));
    }

    #[test]
    fn exclude_filters_files() {
        let cli = CliConfig {
            exclude: vec!["addons/**".to_string()],
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert!(config.should_include(Path::new("scripts/Player.gd")));
        assert!(!config.should_include(Path::new("addons/plugin/main.gd")));
    }

    #[test]
    fn include_restricts_to_matching() {
        let cli = CliConfig {
            include: vec!["scripts/**/*.gd".to_string()],
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert!(config.should_include(Path::new("scripts/Player.gd")));
        assert!(config.should_include(Path::new("scripts/sub/Foo.gd")));
        assert!(!config.should_include(Path::new("addons/plugin/main.gd")));
    }

    #[test]
    fn exclude_takes_priority_over_include() {
        let cli = CliConfig {
            include: vec!["scripts/**/*.gd".to_string()],
            exclude: vec!["scripts/generated/**".to_string()],
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert!(config.should_include(Path::new("scripts/Player.gd")));
        assert!(!config.should_include(Path::new("scripts/generated/auto.gd")));
    }

    #[test]
    fn disable_rules() {
        let cli = CliConfig {
            disable: vec!["correctness/unused-variable".to_string()],
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert!(!config.is_rule_enabled("correctness/unused-variable"));
        assert!(config.is_rule_enabled("perf/process-allocation"));
    }

    #[test]
    fn only_runs_specified_rules() {
        let cli = CliConfig {
            only: vec![
                "correctness/unused-variable".to_string(),
                "perf/process-allocation".to_string(),
            ],
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert!(config.is_rule_enabled("correctness/unused-variable"));
        assert!(config.is_rule_enabled("perf/process-allocation"));
        assert!(!config.is_rule_enabled("style/untyped-parameter"));
        assert!(!config.is_rule_enabled("correctness/unreachable-code"));
    }

    #[test]
    fn rule_severity_off_disables() {
        let mut rules = HashMap::new();
        rules.insert(
            "perf/process-allocation".to_string(),
            RuleConfig::Severity(RuleSeverity::Off),
        );

        let config = Config {
            include: Vec::new(),
            exclude: Vec::new(),
            disable: Vec::new(),
            only: Vec::new(),
            rules,
            target_version: None,
            fail_on: Severity::Info,
            formatter: FormatterConfig::default(),
            include_set: None,
            exclude_set: None,
        };

        assert!(!config.is_rule_enabled("perf/process-allocation"));
    }

    #[test]
    fn effective_severity_override() {
        let mut rules = HashMap::new();
        rules.insert(
            "perf/process-allocation".to_string(),
            RuleConfig::Severity(RuleSeverity::Error),
        );

        let config = Config {
            include: Vec::new(),
            exclude: Vec::new(),
            disable: Vec::new(),
            only: Vec::new(),
            rules,
            target_version: None,
            fail_on: Severity::Info,
            formatter: FormatterConfig::default(),
            include_set: None,
            exclude_set: None,
        };

        assert_eq!(
            config.effective_severity("perf/process-allocation", Severity::Warning),
            Some(Severity::Error)
        );
    }

    #[test]
    fn toml_roundtrip() {
        let toml_str = r#"
include = ["scripts/**/*.gd"]
exclude = ["addons/third_party/**"]
disable = ["correctness/unused-signal"]

[rules]
"perf/process-allocation" = "error"
"correctness/unused-variable" = "off"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.include, vec!["scripts/**/*.gd"]);
        assert_eq!(config.exclude, vec!["addons/third_party/**"]);
        assert_eq!(config.disable, vec!["correctness/unused-signal"]);
        assert!(matches!(
            config.rules["perf/process-allocation"],
            RuleConfig::Severity(RuleSeverity::Error)
        ));
        assert!(matches!(
            config.rules["correctness/unused-variable"],
            RuleConfig::Severity(RuleSeverity::Off)
        ));
        assert_eq!(config.target_version, None); // Not set in this TOML
    }

    #[test]
    fn toml_rule_options() {
        let toml_str = r#"
[rules."style/function-too-long"]
severity = "warning"
max_length = 60

[rules."style/excessive-nesting"]
max_depth = 4
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        match &config.rules["style/function-too-long"] {
            RuleConfig::Options(opts) => {
                assert_eq!(opts.severity, Some(RuleSeverity::Warning));
                assert_eq!(opts.options["max_length"].as_integer(), Some(60));
            }
            _ => panic!("Expected RuleConfig::Options"),
        }
        match &config.rules["style/excessive-nesting"] {
            RuleConfig::Options(opts) => {
                assert_eq!(opts.severity, None);
                assert_eq!(opts.options["max_depth"].as_integer(), Some(4));
            }
            _ => panic!("Expected RuleConfig::Options"),
        }
    }

    #[test]
    fn rule_option_accessor() {
        let mut rules = HashMap::new();
        rules.insert(
            "style/function-too-long".to_string(),
            RuleConfig::Options(RuleOptionsMap {
                severity: Some(RuleSeverity::Warning),
                options: {
                    let mut m = HashMap::new();
                    m.insert("max_length".to_string(), toml::Value::Integer(60));
                    m
                },
            }),
        );
        rules.insert(
            "perf/process-allocation".to_string(),
            RuleConfig::Severity(RuleSeverity::Error),
        );

        let config = Config {
            include: Vec::new(),
            exclude: Vec::new(),
            disable: Vec::new(),
            only: Vec::new(),
            rules,
            target_version: None,
            fail_on: Severity::Info,
            formatter: FormatterConfig::default(),
            include_set: None,
            exclude_set: None,
        };

        assert_eq!(
            config
                .rule_option("style/function-too-long", "max_length")
                .and_then(|v| v.as_integer()),
            Some(60)
        );
        assert_eq!(
            config.rule_option("perf/process-allocation", "max_length"),
            None
        );
        assert_eq!(config.rule_option("nonexistent/rule", "key"), None);
    }

    #[test]
    fn toml_target_version() {
        let toml_str = r#"
target_version = "4.5"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target_version, Some("4.5".to_string()));
    }

    #[test]
    fn toml_target_version_default_is_none() {
        let config = Config::load(None, &CliConfig::default());
        assert_eq!(config.target_version, None);
    }

    #[test]
    fn toml_fail_on_error() {
        let toml_str = r#"
fail_on = "error"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.fail_on, Some(RuleSeverity::Error));
    }

    #[test]
    fn toml_fail_on_warning() {
        let toml_str = r#"
fail_on = "warning"
"#;
        let config: ConfigFile = toml::from_str(toml_str).unwrap();
        assert_eq!(config.fail_on, Some(RuleSeverity::Warning));
    }

    #[test]
    fn fail_on_default_is_info() {
        let config = Config::load(None, &CliConfig::default());
        assert_eq!(config.fail_on, Severity::Info);
    }

    #[test]
    fn fail_on_cli_overrides_toml() {
        // TOML would set "error", but CLI sets "warning"
        // Since we can't load a real TOML here, just verify CLI takes precedence
        let cli = CliConfig {
            fail_on: Some(Severity::Warning),
            ..Default::default()
        };
        let config = Config::load(None, &cli);
        assert_eq!(config.fail_on, Severity::Warning);
    }
}
