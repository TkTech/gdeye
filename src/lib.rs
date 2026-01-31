//! gdeye - Static analysis library for GDScript (Godot 4.x)
//!
//! This library provides comprehensive static analysis capabilities for GDScript,
//! including parsing, symbol extraction, type inference, control flow analysis,
//! and lint rule checking.
//!
//! # Example
//!
//! ```no_run
//! use gdeye::{AnalysisBuilder, Config};
//! use std::path::Path;
//!
//! // Create analysis context for a Godot project
//! let analysis = AnalysisBuilder::new()
//!     .project_root(Path::new("/path/to/godot/project"))
//!     .build()
//!     .expect("Failed to create analysis context");
//!
//! // Analyze all GDScript files
//! let result = analysis.analyze_project().expect("Analysis failed");
//!
//! // Process diagnostics
//! for file_result in result.files() {
//!     for diagnostic in file_result.diagnostics() {
//!         println!("{}:{}: {}", file_result.path().display(), diagnostic.line, diagnostic.message);
//!     }
//! }
//! ```

pub mod analysis;
pub mod cache;
pub mod call_graph;
pub mod cfg;
pub mod classdb;
pub mod classdb_loader;
pub mod config;
pub mod cross_file_usage;
pub mod document;
pub mod error;
pub mod fix;
pub mod flow;
pub mod fmt;
pub mod parser;
pub mod pipeline;
pub mod project;
pub mod project_index;
pub mod report;
pub mod rules;
pub mod scene;
pub mod symbol_index;
pub mod symbols;
pub mod types;
pub mod util;

// LSP module (requires "lsp" feature)
#[cfg(feature = "lsp")]
pub mod lsp;

// MCP module (requires "mcp" feature)
#[cfg(feature = "mcp")]
pub mod mcp;

// Internal modules (not part of public API)
mod debug;

// Re-export primary types for convenience
pub use analysis::{
    analyze_source, analyze_source_impl, AnalysisBuilder, AnalysisPipeline, FileAnalysis,
    ProjectAnalysis, ProjectContext, SingleFileAnalysis,
};
pub use config::Config;
pub use error::{Error, Result};
pub use parser::ParsedFile;
pub use rules::{DiagLabel, Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};
pub use symbols::{FileSymbols, FuncDecl, ParamDecl, SignalDecl, VarDecl};

// Re-export incremental analysis infrastructure
pub use cache::{AnalysisCache, CacheStats, CachedFile, CachedProjectData};
pub use document::{Document, Position, Range};
pub use pipeline::{process_file, process_files_parallel, process_source, ProcessedFile};
pub use project_index::{IndexedFile, ProjectIndex};
pub use symbol_index::{SymbolAtResult, SymbolDef, SymbolId, SymbolIndex, SymbolKind, SymbolRef};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
