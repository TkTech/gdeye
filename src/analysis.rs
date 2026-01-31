//! Unified analysis API for gdeye.
//!
//! This module provides the primary interface for analyzing GDScript files,
//! both as part of a full project and individually.
//!
//! # Example
//!
//! ```no_run
//! use gdeye::AnalysisBuilder;
//! use std::path::Path;
//!
//! let analysis = AnalysisBuilder::new()
//!     .project_root(Path::new("/path/to/project"))
//!     .build()
//!     .unwrap();
//!
//! let result = analysis.analyze_project().unwrap();
//! for file in result.files() {
//!     println!("{}: {} diagnostics", file.path().display(), file.diagnostics().len());
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::call_graph::CallGraph;
use crate::cfg::{self, Cfg};
use crate::classdb::ClassDb;
use crate::classdb_loader::{self, ClassDbMode};
use crate::config::{CliConfig, Config};
use crate::cross_file_usage;
use crate::error::{Error, Result};
use crate::fix::{self, FixResult};
use crate::flow::{self, FlowResults};
use crate::parser::{self, ParsedFile};
use crate::project::{self, ProjectInfo};
use crate::project_index::ProjectIndex;
use crate::rules::{self, Diagnostic, Severity};
use crate::scene::{self, SceneFile};
use crate::symbols::{self, FileSymbols};
use crate::types;
use crate::util::LineIndex;

/// Builder for creating an analysis context.
///
/// Use this to configure and create a [`ProjectContext`] for analyzing GDScript files.
#[derive(Debug, Default)]
pub struct AnalysisBuilder {
    project_root: Option<PathBuf>,
    config: Option<Config>,
    target_version: Option<String>,
    include: Vec<String>,
    exclude: Vec<String>,
    disable: Vec<String>,
}

impl AnalysisBuilder {
    /// Create a new analysis builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the Godot project root directory.
    ///
    /// If not set, gdeye will search for a `project.godot` file in parent directories.
    pub fn project_root(mut self, path: impl AsRef<Path>) -> Self {
        self.project_root = Some(path.as_ref().to_path_buf());
        self
    }

    /// Set a pre-loaded configuration.
    ///
    /// If not set, configuration will be loaded from `gdeye.toml` in the project root.
    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the target Godot version for ClassDB.
    ///
    /// This uses bundled ClassDB data instead of querying a local Godot installation.
    pub fn target_version(mut self, version: impl Into<String>) -> Self {
        self.target_version = Some(version.into());
        self
    }

    /// Add glob patterns for files to include.
    pub fn include(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.include.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Add glob patterns for files to exclude.
    pub fn exclude(mut self, patterns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.exclude.extend(patterns.into_iter().map(Into::into));
        self
    }

    /// Disable specific rules.
    pub fn disable(mut self, rules: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.disable.extend(rules.into_iter().map(Into::into));
        self
    }

    /// Build the analysis context.
    pub fn build(self) -> Result<ProjectContext> {
        // Determine project root
        let project_root = self
            .project_root
            .or_else(|| find_project_root(&[".".into()]));

        // Load or use provided config
        let cli_config = CliConfig {
            include: self.include,
            exclude: self.exclude,
            disable: self.disable,
            only: Vec::new(),
            fail_on: None,
        };
        let config = self
            .config
            .unwrap_or_else(|| Config::load(project_root.as_deref(), &cli_config));

        // Parse project.godot
        let project_info = project_root
            .as_ref()
            .map(|root| project::parse_project(root))
            .unwrap_or_default();

        // Determine ClassDB mode
        let classdb_mode = if let Some(version) = self.target_version {
            ClassDbMode::TargetVersion(version)
        } else if let Some(ref version) = config.target_version {
            ClassDbMode::TargetVersion(version.clone())
        } else {
            ClassDbMode::Auto
        };

        // Load ClassDB
        let class_db = classdb_loader::load_classdb(&classdb_mode)?;

        // Parse scene files
        let scenes = project_root
            .as_ref()
            .map(|root| scene::parse_all_scenes(root))
            .unwrap_or_default();

        Ok(ProjectContext {
            project_root,
            project_info,
            class_db,
            scenes,
            config,
        })
    }
}

/// Immutable project-wide context for analysis.
///
/// Contains all project-level data needed for analyzing GDScript files.
/// This is relatively expensive to create, so it should be reused across
/// multiple analysis runs.
#[derive(Debug)]
pub struct ProjectContext {
    /// Path to the Godot project root (directory containing project.godot).
    pub project_root: Option<PathBuf>,
    /// Parsed project.godot data.
    pub project_info: ProjectInfo,
    /// Godot engine ClassDB for built-in types.
    pub class_db: ClassDb,
    /// Parsed scene files (.tscn).
    pub scenes: HashMap<PathBuf, SceneFile>,
    /// Analysis configuration.
    pub config: Config,
}

impl ProjectContext {
    /// Create a new project context with default settings.
    pub fn new(project_root: Option<PathBuf>) -> Result<Self> {
        AnalysisBuilder::new()
            .project_root(project_root.unwrap_or_else(|| PathBuf::from(".")))
            .build()
    }

    /// Create a builder for more control over context creation.
    pub fn builder() -> AnalysisBuilder {
        AnalysisBuilder::new()
    }

    /// Analyze the entire project.
    ///
    /// Discovers all .gd files in the project and runs the full analysis pipeline.
    pub fn analyze_project(&self) -> Result<ProjectAnalysis> {
        let paths = self
            .project_root
            .as_ref()
            .map(|r| vec![r.clone()])
            .unwrap_or_else(|| vec![PathBuf::from(".")]);

        let files = discover_gdscript_files(&paths, &self.config, self.project_root.as_deref());

        if files.is_empty() {
            return Err(Error::NoFilesFound);
        }

        self.analyze_files(&files)
    }

    /// Analyze specific files.
    ///
    /// Runs the full analysis pipeline on the given files.
    pub fn analyze_files(&self, files: &[PathBuf]) -> Result<ProjectAnalysis> {
        if files.is_empty() {
            return Err(Error::NoFilesFound);
        }

        // Pass 1 & 2: Parse and collect symbols using unified pipeline (parallel)
        let processed_results = crate::pipeline::process_files_parallel(files, false);

        // Collect parse errors for reporting and separate successful results
        let mut parse_errors: Vec<(PathBuf, String)> = Vec::new();
        let mut parsed: Vec<(PathBuf, ParsedFile)> = Vec::new();
        let mut file_symbols: Vec<FileSymbols> = Vec::new();

        for (path, result) in processed_results {
            match result {
                Ok(processed) => {
                    parsed.push((path, processed.parsed));
                    file_symbols.push(processed.symbols);
                }
                Err(e) => parse_errors.push((path, e.to_string())),
            }
        }

        // Pass 3: Cross-file resolution
        symbols::resolve_cross_file(&mut file_symbols, &self.project_info);

        // Pass 4: Type propagation (parallel)
        file_symbols
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, file_sym)| {
                let (_, parsed_file) = &parsed[i];
                types::propagate_types(file_sym, parsed_file, &self.class_db);
            });

        // Pass 4.5: Build project index for cross-file lookups
        let project_index = ProjectIndex::build_from_symbols(
            &file_symbols,
            &self.project_info,
            self.project_root.as_deref(),
        );

        // Pass 4.6: Cross-file usage marking
        cross_file_usage::mark_cross_file_usage(
            &mut file_symbols,
            &parsed,
            &project_index,
            &self.scenes,
            &self.class_db,
        );

        // Pass 4.7: Build call graph and infer parameter types
        let call_graph = CallGraph::build(&parsed, &file_symbols, &self.class_db);
        crate::call_graph::infer_parameter_types(
            &mut file_symbols,
            &parsed,
            &call_graph,
            &self.class_db,
        );

        // Pass 4.8: Compute entry points and reachability for dead code detection
        let entry_points = crate::call_graph::collect_entry_points(&file_symbols, &parsed);
        let reachable_functions = Arc::new(call_graph.compute_reachability(&entry_points));

        // Pass 5 & 6: Build CFGs, run flow analysis, and run lint rules (parallel)
        let file_analyses: Vec<FileAnalysis> = parsed
            .par_iter()
            .enumerate()
            .map(|(i, (path, parsed))| {
                let file_sym = &file_symbols[i];

                // Build CFGs for each function
                let cfgs = cfg::build_cfgs(parsed);

                // Run flow analysis
                let flow_results = flow::analyze(&cfgs, file_sym);

                // Run lint rules
                let ctx = rules::RuleContext {
                    path,
                    parsed,
                    file_sym,
                    all_file_symbols: &file_symbols,
                    cfgs: &cfgs,
                    flow_results: &flow_results,
                    scenes: &self.scenes,
                    class_db: &self.class_db,
                    config: &self.config,
                    project_info: &self.project_info,
                    call_graph: &call_graph,
                    reachable_functions: &reachable_functions,
                };
                let diagnostics = rules::run_all(&ctx);

                FileAnalysis {
                    path: path.clone(),
                    source: parsed.source().to_string(),
                    diagnostics,
                    cfgs,
                    flow_results,
                }
            })
            .collect();

        Ok(ProjectAnalysis {
            files: file_analyses,
            file_symbols,
            project_index,
            call_graph,
            parse_errors,
        })
    }

    /// Analyze a single file with the given source content.
    ///
    /// This is useful for editor integration where you have the file content
    /// in memory. Note that cross-file features will be limited without
    /// a full project analysis.
    pub fn analyze_single(&self, path: &Path, source: &str) -> Result<SingleFileAnalysis> {
        analyze_source_impl(
            path,
            source,
            &self.class_db,
            &self.config,
            &self.scenes,
            &self.project_info,
        )
    }
}

/// Optional cross-file context for analysis.
///
/// When provided, enables cross-file rules like unused function detection.
/// When None, single-file mode is used where all functions are considered reachable.
pub struct CrossFileContext<'a> {
    /// Project-wide call graph.
    pub call_graph: &'a CallGraph,
    /// Functions reachable from entry points.
    pub reachable_functions: &'a Arc<HashSet<(PathBuf, String)>>,
    /// All file symbols in the project.
    pub all_file_symbols: &'a [FileSymbols],
}

/// Analyze a single source file.
///
/// This unified function handles both single-file analysis and project-aware analysis.
/// Pass `cross_file_ctx: Some(...)` to enable cross-file rules like unused function detection.
/// Pass `cross_file_ctx: None` for isolated single-file analysis.
pub fn analyze_source(
    path: &Path,
    source: &str,
    class_db: &ClassDb,
    config: &Config,
    scenes: &HashMap<PathBuf, SceneFile>,
    project_info: &ProjectInfo,
    cross_file_ctx: Option<CrossFileContext<'_>>,
) -> Result<SingleFileAnalysis> {
    let parsed = parser::parse_source(source).map_err(|e| Error::parse(path, e))?;

    let mut file_sym = symbols::collect_symbols(path, &parsed);
    types::propagate_types(&mut file_sym, &parsed, class_db);

    let cfgs = cfg::build_cfgs(&parsed);
    let flow_results = flow::analyze(&cfgs, &file_sym);

    // Use cross-file context if provided, otherwise create single-file defaults
    let (call_graph_owned, reachable_owned);
    let (call_graph, reachable_functions, all_file_symbols): (
        &CallGraph,
        &Arc<HashSet<(PathBuf, String)>>,
        &[FileSymbols],
    ) = match &cross_file_ctx {
        Some(ctx) => (
            ctx.call_graph,
            ctx.reachable_functions,
            ctx.all_file_symbols,
        ),
        None => {
            // Single-file mode: empty call graph, all functions reachable
            call_graph_owned = CallGraph::default();
            reachable_owned = Arc::new(
                file_sym
                    .functions
                    .iter()
                    .map(|f| (path.to_path_buf(), f.name.clone()))
                    .collect(),
            );
            (
                &call_graph_owned,
                &reachable_owned,
                std::slice::from_ref(&file_sym),
            )
        }
    };

    let ctx = rules::RuleContext {
        path,
        parsed: &parsed,
        file_sym: &file_sym,
        all_file_symbols,
        cfgs: &cfgs,
        flow_results: &flow_results,
        scenes,
        class_db,
        config,
        project_info,
        call_graph,
        reachable_functions,
    };
    let diagnostics = rules::run_all(&ctx);

    Ok(SingleFileAnalysis {
        path: path.to_path_buf(),
        source: source.to_string(),
        parsed,
        symbols: file_sym,
        cfgs,
        flow_results,
        diagnostics,
    })
}

/// Shared implementation for single-file analysis (backwards compatibility).
///
/// Prefer using `analyze_source` with `cross_file_ctx: None` for new code.
pub fn analyze_source_impl(
    path: &Path,
    source: &str,
    class_db: &ClassDb,
    config: &Config,
    scenes: &HashMap<PathBuf, SceneFile>,
    project_info: &ProjectInfo,
) -> Result<SingleFileAnalysis> {
    analyze_source(path, source, class_db, config, scenes, project_info, None)
}

/// Analyze a single file using project-wide context (backwards compatibility).
///
/// Prefer using `analyze_source` with `cross_file_ctx: Some(...)` for new code.
#[allow(clippy::too_many_arguments)]
pub fn analyze_source_with_project_context(
    path: &Path,
    source: &str,
    class_db: &ClassDb,
    config: &Config,
    scenes: &HashMap<PathBuf, SceneFile>,
    project_info: &ProjectInfo,
    call_graph: &CallGraph,
    reachable_functions: &Arc<HashSet<(PathBuf, String)>>,
    all_file_symbols: &[FileSymbols],
) -> Result<SingleFileAnalysis> {
    analyze_source(
        path,
        source,
        class_db,
        config,
        scenes,
        project_info,
        Some(CrossFileContext {
            call_graph,
            reachable_functions,
            all_file_symbols,
        }),
    )
}

/// Unified analysis pipeline for both CLI and LSP.
///
/// This struct provides a single entry point for analyzing GDScript files,
/// whether in batch mode (CLI) or incrementally (LSP). It encapsulates all
/// the project-wide context needed for cross-file analysis.
///
/// # Usage
///
/// For CLI batch analysis:
/// ```ignore
/// let ctx = ProjectContext::new(Some(project_root))?;
/// let pipeline = AnalysisPipeline::from_project_context(&ctx);
/// let result = pipeline.analyze_file(&path, &source)?;
/// ```
///
/// For LSP incremental analysis with cached data:
/// ```ignore
/// let pipeline = AnalysisPipeline::with_cross_file_data(
///     class_db, config, scenes, project_info,
///     call_graph, reachable_functions, all_file_symbols
/// );
/// let result = pipeline.analyze_file(&path, &source)?;
/// ```
pub struct AnalysisPipeline {
    /// Class database for type information.
    class_db: Arc<ClassDb>,
    /// Configuration.
    config: Config,
    /// Parsed scene files.
    scenes: Arc<HashMap<PathBuf, SceneFile>>,
    /// Project info (autoloads, settings).
    project_info: Arc<ProjectInfo>,
    /// Project-wide call graph (optional for cross-file analysis).
    call_graph: Option<Arc<CallGraph>>,
    /// Functions reachable from entry points (optional for cross-file analysis).
    reachable_functions: Option<Arc<HashSet<(PathBuf, String)>>>,
    /// All file symbols in the project (optional for cross-file analysis).
    all_file_symbols: Option<Arc<Vec<FileSymbols>>>,
}

impl AnalysisPipeline {
    /// Create a pipeline for single-file analysis (no cross-file context).
    pub fn new(
        class_db: Arc<ClassDb>,
        config: Config,
        scenes: Arc<HashMap<PathBuf, SceneFile>>,
        project_info: Arc<ProjectInfo>,
    ) -> Self {
        Self {
            class_db,
            config,
            scenes,
            project_info,
            call_graph: None,
            reachable_functions: None,
            all_file_symbols: None,
        }
    }

    /// Create a pipeline with full cross-file context (for LSP).
    pub fn with_cross_file_data(
        class_db: Arc<ClassDb>,
        config: Config,
        scenes: Arc<HashMap<PathBuf, SceneFile>>,
        project_info: Arc<ProjectInfo>,
        call_graph: Arc<CallGraph>,
        reachable_functions: Arc<HashSet<(PathBuf, String)>>,
        all_file_symbols: Arc<Vec<FileSymbols>>,
    ) -> Self {
        Self {
            class_db,
            config,
            scenes,
            project_info,
            call_graph: Some(call_graph),
            reachable_functions: Some(reachable_functions),
            all_file_symbols: Some(all_file_symbols),
        }
    }

    /// Create a pipeline from a ProjectContext (for CLI).
    pub fn from_project_context(ctx: &ProjectContext) -> Self {
        Self {
            class_db: Arc::new(ctx.class_db.clone()),
            config: ctx.config.clone(),
            scenes: Arc::new(ctx.scenes.clone()),
            project_info: Arc::new(ctx.project_info.clone()),
            call_graph: None,
            reachable_functions: None,
            all_file_symbols: None,
        }
    }

    /// Analyze a single file.
    ///
    /// Uses cross-file context if available, otherwise runs in single-file mode.
    pub fn analyze_file(&self, path: &Path, source: &str) -> Result<SingleFileAnalysis> {
        let cross_file_ctx = match (
            &self.call_graph,
            &self.reachable_functions,
            &self.all_file_symbols,
        ) {
            (Some(cg), Some(rf), Some(afs)) => Some(CrossFileContext {
                call_graph: cg.as_ref(),
                reachable_functions: rf,
                all_file_symbols: afs.as_slice(),
            }),
            _ => None,
        };

        analyze_source(
            path,
            source,
            &self.class_db,
            &self.config,
            &self.scenes,
            &self.project_info,
            cross_file_ctx,
        )
    }

    /// Check if this pipeline has cross-file context.
    pub fn has_cross_file_context(&self) -> bool {
        self.call_graph.is_some()
            && self.reachable_functions.is_some()
            && self.all_file_symbols.is_some()
    }

    /// Get the class database.
    pub fn class_db(&self) -> &ClassDb {
        &self.class_db
    }

    /// Get the configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Result of analyzing a single file as part of a project.
#[derive(Debug)]
pub struct FileAnalysis {
    /// Path to the analyzed file.
    pub path: PathBuf,
    /// Source code content.
    pub source: String,
    /// Diagnostics found in this file.
    pub diagnostics: Vec<Diagnostic>,
    /// Control flow graphs for functions in this file.
    pub cfgs: Vec<Cfg>,
    /// Flow analysis results.
    pub flow_results: FlowResults,
}

impl FileAnalysis {
    /// Get the file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the source code.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get all diagnostics for this file.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Check if this file has any diagnostics.
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Count diagnostics by severity.
    pub fn severity_counts(&self) -> SeverityCounts {
        let mut counts = SeverityCounts::default();
        for d in &self.diagnostics {
            match d.severity {
                Severity::Error => counts.errors += 1,
                Severity::Warning => counts.warnings += 1,
                Severity::Info => counts.infos += 1,
            }
        }
        counts
    }

    /// Apply fixes to this file and return the modified source.
    /// If `include_unsafe` is true, also applies fixes marked as unsafe.
    pub fn apply_fixes(&self, include_unsafe: bool) -> FixResult {
        fix::apply_fixes(&self.source, &self.diagnostics, include_unsafe)
    }

    /// Get diagnostics that would remain after applying fixes.
    pub fn unfixed_diagnostics(&self, include_unsafe: bool) -> Vec<&Diagnostic> {
        let fix_result = self.apply_fixes(include_unsafe);
        let line_index = LineIndex::new(&self.source);

        self.diagnostics
            .iter()
            .filter(|d| {
                d.fix.is_none()
                    && !fix::overlaps_applied_fix(d, &line_index, &fix_result.applied_ranges)
            })
            .collect()
    }
}

/// Result of analyzing a single file independently.
#[derive(Debug)]
pub struct SingleFileAnalysis {
    /// Path to the analyzed file.
    pub path: PathBuf,
    /// Source code content.
    pub source: String,
    /// Parsed AST.
    pub parsed: ParsedFile,
    /// Extracted symbols.
    pub symbols: FileSymbols,
    /// Control flow graphs.
    pub cfgs: Vec<Cfg>,
    /// Flow analysis results.
    pub flow_results: FlowResults,
    /// Diagnostics found.
    pub diagnostics: Vec<Diagnostic>,
}

impl SingleFileAnalysis {
    /// Get the file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the source code.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get all diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Get the parsed file.
    pub fn parsed(&self) -> &ParsedFile {
        &self.parsed
    }

    /// Get the extracted symbols.
    pub fn symbols(&self) -> &FileSymbols {
        &self.symbols
    }
}

/// Result of analyzing an entire project.
#[derive(Debug)]
pub struct ProjectAnalysis {
    /// Analysis results for each file.
    files: Vec<FileAnalysis>,
    /// Symbols for all files (same order as files).
    file_symbols: Vec<FileSymbols>,
    /// Project-wide index for cross-file lookups.
    project_index: ProjectIndex,
    /// Project-wide call graph.
    call_graph: CallGraph,
    /// Files that failed to parse.
    parse_errors: Vec<(PathBuf, String)>,
}

impl ProjectAnalysis {
    /// Get all file analysis results.
    pub fn files(&self) -> &[FileAnalysis] {
        &self.files
    }

    /// Get analysis for a specific file by path.
    pub fn file(&self, path: &Path) -> Option<&FileAnalysis> {
        self.files.iter().find(|f| f.path == path)
    }

    /// Get all diagnostics across all files.
    pub fn all_diagnostics(&self) -> impl Iterator<Item = (&Path, &Diagnostic)> {
        self.files
            .iter()
            .flat_map(|f| f.diagnostics.iter().map(move |d| (f.path.as_path(), d)))
    }

    /// Get total diagnostic counts.
    pub fn severity_counts(&self) -> SeverityCounts {
        let mut counts = SeverityCounts::default();
        for f in &self.files {
            let fc = f.severity_counts();
            counts.errors += fc.errors;
            counts.warnings += fc.warnings;
            counts.infos += fc.infos;
        }
        counts
    }

    /// Check if any file has diagnostics at or above the given severity.
    pub fn has_diagnostics_at_severity(&self, min_severity: Severity) -> bool {
        let counts = self.severity_counts();
        match min_severity {
            Severity::Error => counts.errors > 0,
            Severity::Warning => counts.errors + counts.warnings > 0,
            Severity::Info => counts.errors + counts.warnings + counts.infos > 0,
        }
    }

    /// Get the symbols for all files.
    pub fn file_symbols(&self) -> &[FileSymbols] {
        &self.file_symbols
    }

    /// Get the project index.
    pub fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    /// Get the call graph.
    pub fn call_graph(&self) -> &CallGraph {
        &self.call_graph
    }

    /// Get files that failed to parse.
    pub fn parse_errors(&self) -> &[(PathBuf, String)] {
        &self.parse_errors
    }

    /// Check if any files failed to parse.
    pub fn has_parse_errors(&self) -> bool {
        !self.parse_errors.is_empty()
    }

    /// Apply fixes to all files and return a map of path -> new source.
    ///
    /// This does not write to disk; call [`write_fixes`] for that.
    /// If `include_unsafe` is true, also applies fixes marked as unsafe.
    pub fn apply_fixes(&self, include_unsafe: bool) -> HashMap<PathBuf, String> {
        self.files
            .iter()
            .filter_map(|f| {
                let result = f.apply_fixes(include_unsafe);
                if result.num_fixed > 0 {
                    Some((f.path.clone(), result.source))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Apply fixes and write them to disk.
    ///
    /// Returns the number of files modified and any errors encountered.
    /// If `include_unsafe` is true, also applies fixes marked as unsafe.
    pub fn write_fixes(&self, include_unsafe: bool) -> (usize, Vec<(PathBuf, std::io::Error)>) {
        let fixes = self.apply_fixes(include_unsafe);
        let mut modified = 0;
        let mut errors = Vec::new();

        for (path, source) in fixes {
            match std::fs::write(&path, &source) {
                Ok(()) => modified += 1,
                Err(e) => errors.push((path, e)),
            }
        }

        (modified, errors)
    }
}

/// Diagnostic severity counts.
#[derive(Debug, Default, Clone, Copy)]
pub struct SeverityCounts {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl SeverityCounts {
    /// Total number of diagnostics.
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.infos
    }

    /// Check if any diagnostics exist.
    pub fn has_any(&self) -> bool {
        self.total() > 0
    }
}

// ============ Helper Functions ============

/// Find the Godot project root by searching for `project.godot` in ancestors.
pub fn find_project_root(paths: &[PathBuf]) -> Option<PathBuf> {
    let start = if let Some(p) = paths.first() {
        if p.is_file() {
            p.parent()?.to_path_buf()
        } else {
            p.clone()
        }
    } else {
        PathBuf::from(".")
    };

    let mut dir = start.canonicalize().ok()?;
    loop {
        if dir.join("project.godot").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Discover all GDScript files in the given paths.
pub fn discover_gdscript_files(
    paths: &[PathBuf],
    config: &Config,
    project_root: Option<&Path>,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() && path.extension().is_some_and(|e| e == "gd") {
            let rel = relative_to_root(path, project_root);
            if config.should_include(&rel) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let p = entry.path();
                if p.is_file() && p.extension().is_some_and(|e| e == "gd") {
                    let rel = relative_to_root(p, project_root);
                    if config.should_include(&rel) {
                        files.push(p.to_path_buf());
                    }
                }
            }
        }
    }
    files
}

/// Compute a path relative to the project root for glob matching.
fn relative_to_root(path: &Path, project_root: Option<&Path>) -> PathBuf {
    if let Some(root) = project_root {
        if let Ok(canonical) = path.canonicalize() {
            if let Ok(root_canonical) = root.canonicalize() {
                if let Ok(rel) = canonical.strip_prefix(&root_canonical) {
                    return rel.to_path_buf();
                }
            }
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_counts() {
        let counts = SeverityCounts {
            errors: 1,
            warnings: 2,
            infos: 3,
        };
        assert_eq!(counts.total(), 6);
        assert!(counts.has_any());

        let empty = SeverityCounts::default();
        assert_eq!(empty.total(), 0);
        assert!(!empty.has_any());
    }

    #[test]
    fn test_builder_default() {
        let builder = AnalysisBuilder::new();
        assert!(builder.project_root.is_none());
        assert!(builder.config.is_none());
        assert!(builder.target_version.is_none());
    }

    #[test]
    fn test_builder_chain() {
        let builder = AnalysisBuilder::new()
            .target_version("4.5")
            .exclude(["test/**"])
            .disable(["correctness/unused-variable"]);

        assert_eq!(builder.target_version, Some("4.5".to_string()));
        assert_eq!(builder.exclude, vec!["test/**"]);
        assert_eq!(builder.disable, vec!["correctness/unused-variable"]);
    }
}
