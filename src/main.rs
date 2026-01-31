//! gdeye CLI - Static analysis tool for GDScript
//!
//! This binary provides the command-line interface for gdeye.
//! For library usage, see the `gdeye` crate documentation.

use std::path::PathBuf;
use std::process;
use std::time::Instant;

use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;

// Use the library's public modules
use gdeye::analysis::{self, AnalysisBuilder, SeverityCounts};
use gdeye::config::{CliConfig, Config};
use gdeye::fix::{self, FixCounts};
use gdeye::fmt;
use gdeye::parser;
use gdeye::report;
use gdeye::rules::{self, Severity};
use gdeye::util::LineIndex;

// Internal debug module (not part of library API)
mod debug;

#[derive(clap::Args)]
struct CheckArgs {
    /// Glob patterns for files to include (overrides gdeye.toml)
    #[arg(long, value_name = "GLOB")]
    include: Vec<String>,

    /// Glob patterns for files to exclude (merged with gdeye.toml)
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Disable specific rules (merged with gdeye.toml)
    #[arg(long, value_name = "RULE")]
    disable: Vec<String>,

    /// Run only specific rules (can be repeated, e.g., --rule correctness/unused-variable)
    #[arg(long, value_name = "RULE")]
    rule: Vec<String>,

    /// Automatically fix problems that have safe remediations
    #[arg(long)]
    fix: bool,

    /// Also apply aggressive fixes (e.g., removing unreachable code). Requires --fix.
    #[arg(long, requires = "fix")]
    r#unsafe: bool,

    /// Output format for diagnostics
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Target Godot version for ClassDB (e.g., "4.5"). Uses bundled data, skips runtime Godot.
    #[arg(long, value_name = "VERSION")]
    target_version: Option<String>,

    /// Minimum severity that causes a non-zero exit code (overrides gdeye.toml)
    #[arg(long, value_enum)]
    fail_on: Option<FailOn>,
}

#[derive(Parser)]
#[command(name = "gdeye", about = "Static analysis tool for GDScript", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to a Godot project directory or individual .gd files
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    #[command(flatten)]
    args: CheckArgs,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    /// Human-readable text with source spans (default)
    Text,
    /// One diagnostic per line: path:line:col: severity [rule] message
    Compact,
    /// JSON array of diagnostics
    Json,
    /// SARIF v2.1.0 for CI integration
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FailOn {
    /// Exit with failure only when errors are found
    Error,
    /// Exit with failure when warnings or errors are found
    Warning,
    /// Exit with failure when any diagnostic is found (default)
    Info,
}

impl FailOn {
    fn to_severity(self) -> Severity {
        match self {
            FailOn::Error => Severity::Error,
            FailOn::Warning => Severity::Warning,
            FailOn::Info => Severity::Info,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze GDScript files
    Check {
        /// Files or directories to analyze
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,

        #[command(flatten)]
        args: CheckArgs,
    },
    /// List available lint rules (pass a rule ID for details)
    Rules {
        /// Show detailed info for a specific rule
        #[arg(value_name = "RULE")]
        rule: Option<String>,
    },
    /// Dump the AST of a GDScript file
    DumpAst {
        #[arg(value_name = "FILE")]
        file: PathBuf,
    },
    /// Format GDScript files
    Fmt {
        /// Files or directories to format
        #[arg(value_name = "PATH")]
        paths: Vec<PathBuf>,

        /// Write formatted files in place (default: check mode, print diffs)
        #[arg(long)]
        write: bool,

        /// Maximum line width (default: from gdeye.toml or 100)
        #[arg(long)]
        print_width: Option<usize>,

        /// Glob patterns for files to include
        #[arg(long, value_name = "GLOB")]
        include: Vec<String>,

        /// Glob patterns for files to exclude
        #[arg(long, value_name = "GLOB")]
        exclude: Vec<String>,
    },
    /// Start the Language Server Protocol (LSP) server
    #[cfg(feature = "lsp")]
    Lsp,
    /// Start the MCP (Model Context Protocol) server for AI assistants
    #[cfg(feature = "mcp")]
    Mcp,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::DumpAst { file }) => {
            let parsed = match parser::parse_file(&file) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Error parsing {}: {}", file.display(), e);
                    process::exit(1);
                }
            };
            debug::print_tree(parsed.root_node(), parsed.source(), 0);
        }
        Some(Commands::Rules { rule }) => {
            rules::print_rules(rule.as_deref());
        }
        #[cfg(feature = "lsp")]
        Some(Commands::Lsp) => {
            run_lsp_server();
        }
        #[cfg(feature = "mcp")]
        Some(Commands::Mcp) => {
            run_mcp_server();
        }
        Some(Commands::Check { paths, args }) => {
            let cli_config = CliConfig {
                include: args.include,
                exclude: args.exclude,
                disable: args.disable,
                only: args.rule,
                fail_on: args.fail_on.map(FailOn::to_severity),
            };
            let (fail_level, severity_counts) = analyze_paths(
                &paths,
                &cli_config,
                args.fix,
                args.r#unsafe,
                args.format,
                args.target_version.as_deref(),
            );
            if should_fail(fail_level, &severity_counts) {
                process::exit(1);
            }
        }
        Some(Commands::Fmt {
            paths,
            write,
            print_width,
            include,
            exclude,
        }) => {
            let exit_code = run_fmt(&paths, write, print_width, &include, &exclude);
            if exit_code != 0 {
                process::exit(exit_code);
            }
        }
        None => {
            let path = cli.path.unwrap_or_else(|| PathBuf::from("."));
            let cli_config = CliConfig {
                include: cli.args.include,
                exclude: cli.args.exclude,
                disable: cli.args.disable,
                only: cli.args.rule,
                fail_on: cli.args.fail_on.map(FailOn::to_severity),
            };
            let (fail_level, severity_counts) = analyze_paths(
                &[path],
                &cli_config,
                cli.args.fix,
                cli.args.r#unsafe,
                cli.args.format,
                cli.args.target_version.as_deref(),
            );
            if should_fail(fail_level, &severity_counts) {
                process::exit(1);
            }
        }
    }
}

/// Determine if the process should exit with a failure code based on the
/// resolved fail-on threshold and the severity counts.
fn should_fail(fail_on: Severity, counts: &SeverityCounts) -> bool {
    match fail_on {
        Severity::Error => counts.errors > 0,
        Severity::Warning => counts.errors + counts.warnings > 0,
        Severity::Info => counts.total() > 0,
    }
}

/// Run the full analysis pipeline on the given paths and return the resolved
/// fail-on threshold and severity counts.
fn analyze_paths(
    paths: &[PathBuf],
    cli_config: &CliConfig,
    apply_fixes: bool,
    include_unsafe: bool,
    format: OutputFormat,
    target_version: Option<&str>,
) -> (Severity, SeverityCounts) {
    // Validate --rule arguments before doing any work
    if !cli_config.only.is_empty() {
        validate_rule_ids(&cli_config.only);
    }

    let project_root = analysis::find_project_root(paths);

    // Load configuration (gdeye.toml + CLI overrides)
    let config = Config::load(project_root.as_deref(), cli_config);
    let fail_on = config.fail_on;

    // Build analysis context using the builder API
    let mut builder = AnalysisBuilder::new();
    if let Some(root) = &project_root {
        builder = builder.project_root(root);
    }
    builder = builder.config(config.clone());
    if let Some(version) = target_version {
        builder = builder.target_version(version);
    }

    let ctx = match builder.build() {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // Discover all .gd files, filtered by include/exclude
    let files = analysis::discover_gdscript_files(paths, &config, project_root.as_deref());

    if files.is_empty() {
        eprintln!("No .gd files found.");
        return (fail_on, SeverityCounts::default());
    }

    let start = Instant::now();

    // Run the analysis using the library API
    let result = match ctx.analyze_files(&files) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    };

    // Report parse errors
    for (path, err) in result.parse_errors() {
        eprintln!("Error: Failed to parse {}: {}", path.display(), err);
    }

    // Sequential phase: reporting, fixes, severity counting
    let mut total_fixed = 0;
    let mut severity_counts = SeverityCounts::default();
    let mut fix_counts = FixCounts::default();
    let mut all_file_diagnostics: Vec<(PathBuf, Vec<rules::Diagnostic>)> = Vec::new();

    for file_analysis in result.files() {
        let path = file_analysis.path();
        let source = file_analysis.source();
        let diagnostics = file_analysis.diagnostics();

        if apply_fixes {
            let fix_result = file_analysis.apply_fixes(include_unsafe);
            if fix_result.num_fixed > 0 {
                if let Err(e) = std::fs::write(path, &fix_result.source) {
                    eprintln!("Error writing fix to {}: {}", path.display(), e);
                } else {
                    total_fixed += fix_result.num_fixed;
                }
            }
            // Report remaining unfixed diagnostics that don't overlap with applied fixes
            let line_index = LineIndex::new(source);
            let unfixed: Vec<_> = diagnostics
                .iter()
                .filter(|d| {
                    d.fix.is_none()
                        && !fix::overlaps_applied_fix(d, &line_index, &fix_result.applied_ranges)
                })
                .cloned()
                .collect();
            count_severities(&unfixed, &mut severity_counts);
            // Count remaining fixable issues (those not applied due to --unsafe requirement)
            let remaining_fix_counts = fix::count_fixable(&unfixed);
            fix_counts.safe += remaining_fix_counts.safe;
            fix_counts.unsafe_ += remaining_fix_counts.unsafe_;
            match format {
                OutputFormat::Text => {
                    report::emit_diagnostics(path, source, &unfixed);
                }
                OutputFormat::Compact => {
                    report::emit_compact(path, &unfixed);
                }
                _ => {
                    if !unfixed.is_empty() {
                        all_file_diagnostics.push((path.to_path_buf(), unfixed));
                    }
                }
            }
        } else {
            count_severities(diagnostics, &mut severity_counts);
            // Track fix counts for summary
            let file_fix_counts = fix::count_fixable(diagnostics);
            fix_counts.safe += file_fix_counts.safe;
            fix_counts.unsafe_ += file_fix_counts.unsafe_;
            match format {
                OutputFormat::Text => {
                    report::emit_diagnostics(path, source, diagnostics);
                }
                OutputFormat::Compact => {
                    report::emit_compact(path, diagnostics);
                }
                _ => {
                    if !diagnostics.is_empty() {
                        all_file_diagnostics.push((path.to_path_buf(), diagnostics.to_vec()));
                    }
                }
            }
        }
    }

    // Emit structured output formats
    match format {
        OutputFormat::Json => report::emit_json(&all_file_diagnostics),
        OutputFormat::Sarif => report::emit_sarif(&all_file_diagnostics),
        OutputFormat::Text | OutputFormat::Compact => {}
    }

    if apply_fixes && total_fixed > 0 {
        eprintln!("Fixed {} problem(s).", total_fixed);
    }

    // Print summary for human-readable formats
    if matches!(format, OutputFormat::Text) && severity_counts.has_any() {
        report::emit_summary(&severity_counts, &fix_counts);
    }

    if matches!(format, OutputFormat::Text | OutputFormat::Compact) {
        let elapsed = start.elapsed();
        let total_lines: usize = result
            .files()
            .iter()
            .map(|f| f.source().lines().count())
            .sum();
        eprintln!(
            "Checked {} file(s) in {}, {} lines",
            result.files().len(),
            format_duration(elapsed),
            format_number(total_lines),
        );
    }

    (fail_on, severity_counts)
}

/// Accumulate severity counts from a slice of diagnostics.
fn count_severities(diagnostics: &[rules::Diagnostic], counts: &mut SeverityCounts) {
    for d in diagnostics {
        match d.severity {
            Severity::Error => counts.errors += 1,
            Severity::Warning => counts.warnings += 1,
            Severity::Info => counts.infos += 1,
        }
    }
}

/// Run the formatter on the given paths.
fn run_fmt(
    paths: &[PathBuf],
    write: bool,
    print_width: Option<usize>,
    include: &[String],
    exclude: &[String],
) -> i32 {
    // Build a minimal config for file discovery.
    let cli_config = CliConfig {
        include: include.to_vec(),
        exclude: exclude.to_vec(),
        disable: Vec::new(),
        only: Vec::new(),
        fail_on: None,
    };

    let project_root = analysis::find_project_root(paths);
    let file_config = Config::load(project_root.as_deref(), &cli_config);

    // Use CLI print_width if provided, otherwise use config
    let mut config: fmt::FmtConfig = file_config.formatter.clone().into();
    if let Some(width) = print_width {
        config.print_width = width;
    }

    let effective_paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    };

    let files =
        analysis::discover_gdscript_files(&effective_paths, &file_config, project_root.as_deref());

    if files.is_empty() {
        eprintln!("No .gd files found.");
        return 0;
    }

    let start = Instant::now();

    // Format files in parallel, collecting results.
    let results: Vec<_> = files
        .par_iter()
        .filter_map(|path| match fmt::format_file(path, &config) {
            Ok(result) => Some((path.clone(), result)),
            Err(e) => {
                eprintln!("Error: {}", e);
                None
            }
        })
        .collect();

    let total_lines: usize = results.iter().map(|(_, r)| r.output.lines().count()).sum();
    let mut needs_formatting = 0;
    let mut written = 0;

    for (path, result) in &results {
        if result.unchanged {
            continue;
        }
        needs_formatting += 1;

        if write {
            if let Err(e) = std::fs::write(path, &result.output) {
                eprintln!("Error writing {}: {}", path.display(), e);
            } else {
                written += 1;
            }
        } else {
            // Check mode: print diff.
            let original = std::fs::read_to_string(path).unwrap_or_default();
            let diff = fmt::make_diff(&path.display().to_string(), &original, &result.output);
            if !diff.is_empty() {
                print!("{}", diff);
            }
        }
    }

    let elapsed = start.elapsed();
    let time_str = format_duration(elapsed);
    let lines_str = format_number(total_lines);

    if write {
        if written > 0 {
            eprintln!(
                "Formatted {} file(s) in {}, {} lines",
                written, time_str, lines_str
            );
        } else {
            eprintln!(
                "Checked {} file(s) in {}, {} lines",
                results.len(),
                time_str,
                lines_str
            );
        }
        0
    } else if needs_formatting > 0 {
        eprintln!(
            "{} file(s) would be reformatted (checked {} files in {}, {} lines)",
            needs_formatting,
            results.len(),
            time_str,
            lines_str
        );
        1
    } else {
        eprintln!(
            "All files already formatted (checked {} files in {}, {} lines)",
            results.len(),
            time_str,
            lines_str
        );
        0
    }
}

fn format_duration(d: std::time::Duration) -> String {
    let ms = d.as_secs_f64() * 1000.0;
    if ms < 1.0 {
        format!("{:.2}ms", ms)
    } else if ms < 1000.0 {
        format!("{:.1}ms", ms)
    } else {
        format!("{:.2}s", d.as_secs_f64())
    }
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

/// Validate that all specified rule IDs exist. Exits with an error if any are invalid.
fn validate_rule_ids(rule_ids: &[String]) {
    let all_rules = rules::all_rules();
    let valid_ids: std::collections::HashSet<&str> = all_rules.iter().map(|r| r.id()).collect();

    let mut invalid = Vec::new();
    for id in rule_ids {
        if !valid_ids.contains(id.as_str()) {
            invalid.push(id.as_str());
        }
    }

    if !invalid.is_empty() {
        eprintln!(
            "Error: Unknown rule(s): {}. Run `gdeye rules` to see available rules.",
            invalid.join(", ")
        );
        process::exit(1);
    }
}

/// Run the LSP server (requires "lsp" feature).
#[cfg(feature = "lsp")]
fn run_lsp_server() {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        gdeye::lsp::run_server().await;
    });
}

/// Run the MCP server (requires "mcp" feature).
#[cfg(feature = "mcp")]
fn run_mcp_server() {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        gdeye::mcp::run_server().await;
    });
}
