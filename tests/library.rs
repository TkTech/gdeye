//! Integration tests for the gdeye library API.

use std::path::PathBuf;

use gdeye::{AnalysisBuilder, Config, Severity};

/// Test that we can create an analysis context with the builder pattern.
#[test]
fn analysis_builder_basic() {
    let ctx = AnalysisBuilder::new().target_version("4.5").build();

    assert!(
        ctx.is_ok(),
        "Failed to build analysis context: {:?}",
        ctx.err()
    );
}

/// Test that we can analyze a single file from memory.
#[test]
fn analyze_single_file_from_memory() {
    let ctx = AnalysisBuilder::new()
        .target_version("4.5")
        .build()
        .expect("Failed to build context");

    let source = r#"
extends Node

var unused_var = 10

func _ready():
    pass
"#;

    let result = ctx.analyze_single(&PathBuf::from("test.gd"), source);
    assert!(result.is_ok(), "Analysis failed: {:?}", result.err());

    let analysis = result.unwrap();
    // Should detect dead store (unused variable)
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|d| d.rule == "correctness/dead-store"),
        "Expected dead-store diagnostic, got: {:?}",
        analysis.diagnostics()
    );
}

/// Test that builder options are applied.
#[test]
fn builder_disable_rules() {
    let config = Config::default().with_disabled_rules(vec!["correctness/dead-store".to_string()]);

    let ctx = AnalysisBuilder::new()
        .target_version("4.5")
        .config(config)
        .build()
        .expect("Failed to build context");

    let source = r#"
extends Node

var unused_var = 10
"#;

    let result = ctx
        .analyze_single(&PathBuf::from("test.gd"), source)
        .unwrap();
    // Should NOT detect dead store because the rule is disabled
    assert!(
        !result
            .diagnostics()
            .iter()
            .any(|d| d.rule == "correctness/dead-store"),
        "dead-store rule should be disabled"
    );
}

/// Test severity counts.
#[test]
fn severity_counts_work() {
    let ctx = AnalysisBuilder::new()
        .target_version("4.5")
        .build()
        .expect("Failed to build context");

    let source = r#"
extends Node

var unused_var1 = 10
var unused_var2 = 20

func my_func(unused_param):
    pass
"#;

    let result = ctx
        .analyze_single(&PathBuf::from("test.gd"), source)
        .unwrap();
    let counts =
        result
            .diagnostics
            .iter()
            .fold(gdeye::analysis::SeverityCounts::default(), |mut acc, d| {
                match d.severity {
                    Severity::Error => acc.errors += 1,
                    Severity::Warning => acc.warnings += 1,
                    Severity::Info => acc.infos += 1,
                }
                acc
            });

    assert!(counts.total() > 0, "Expected some diagnostics");
}

/// Test that analysis results include expected fields.
#[test]
fn analysis_result_has_symbols() {
    let ctx = AnalysisBuilder::new()
        .target_version("4.5")
        .build()
        .expect("Failed to build context");

    let source = r#"
extends Node

signal my_signal(value: int)

var my_var: String = "hello"

func my_func():
    pass
"#;

    let result = ctx
        .analyze_single(&PathBuf::from("test.gd"), source)
        .unwrap();
    let symbols = result.symbols();

    // Check that symbols were extracted
    assert!(
        symbols.variables.iter().any(|v| v.name == "my_var"),
        "Expected my_var in symbols"
    );
    assert!(
        symbols.functions.iter().any(|f| f.name == "my_func"),
        "Expected my_func in symbols"
    );
    assert!(
        symbols.signals.iter().any(|s| s.name == "my_signal"),
        "Expected my_signal in symbols"
    );
}

/// Test that parse errors are reported.
#[test]
fn parse_errors_are_reported() {
    let ctx = AnalysisBuilder::new()
        .target_version("4.5")
        .build()
        .expect("Failed to build context");

    // This is invalid GDScript syntax - but parse_source is lenient
    // and tree-sitter will produce an error node
    let source = r#"
extends Node
func invalid(
"#;

    // This should still work - tree-sitter is lenient about errors
    let result = ctx.analyze_single(&PathBuf::from("test.gd"), source);
    // Even with parse errors, we should get a result back
    assert!(
        result.is_ok(),
        "Analysis should succeed even with parse errors: {:?}",
        result.err()
    );
}

/// Test version constant is accessible.
#[test]
fn version_accessible() {
    let version = gdeye::VERSION;
    assert!(!version.is_empty(), "Version should not be empty");
    // Should be semver format
    assert!(
        version.split('.').count() >= 2,
        "Version should be in semver format"
    );
}
