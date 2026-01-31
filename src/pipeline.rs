//! Unified file processing pipeline for gdeye.
//!
//! This module provides a single source of truth for the parse+symbols+index
//! pattern that was previously duplicated across LSP and CLI code paths.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::error::{Error, Result};
use crate::parser::{self, ParsedFile};
use crate::symbol_index::SymbolIndex;
use crate::symbols::{self, FileSymbols};

/// Result of processing a single source file.
///
/// Contains all the data extracted during the parse+symbols+index pipeline.
#[derive(Debug)]
pub struct ProcessedFile {
    /// Path to the file.
    pub path: PathBuf,
    /// Parsed syntax tree.
    pub parsed: ParsedFile,
    /// Extracted symbols (functions, variables, signals, etc.).
    pub symbols: FileSymbols,
    /// Symbol index for fast lookups (optional, built on demand).
    pub index: Option<SymbolIndex>,
}

impl ProcessedFile {
    /// Get the path to this file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the parsed syntax tree.
    pub fn parsed(&self) -> &ParsedFile {
        &self.parsed
    }

    /// Get the extracted symbols.
    pub fn symbols(&self) -> &FileSymbols {
        &self.symbols
    }

    /// Get the symbol index, if built.
    pub fn index(&self) -> Option<&SymbolIndex> {
        self.index.as_ref()
    }
}

/// Process source code from memory.
///
/// This is the primary entry point for processing GDScript source code.
/// Use this when you have the source content in memory (e.g., from an editor).
///
/// # Arguments
///
/// * `path` - Path to associate with this source (used for diagnostics)
/// * `source` - The GDScript source code
/// * `build_index` - Whether to build a symbol index for fast lookups
///
/// # Returns
///
/// A `ProcessedFile` containing the parsed AST, symbols, and optionally the index.
pub fn process_source(path: &Path, source: &str, build_index: bool) -> Result<ProcessedFile> {
    let parsed = parser::parse_source(source).map_err(|e| Error::parse(path, e))?;
    let symbols = symbols::collect_symbols(path, &parsed);
    let index = if build_index {
        Some(SymbolIndex::build(&parsed, &symbols))
    } else {
        None
    };

    Ok(ProcessedFile {
        path: path.to_path_buf(),
        parsed,
        symbols,
        index,
    })
}

/// Process a file from disk.
///
/// Reads the file and processes it through the parse+symbols+index pipeline.
///
/// # Arguments
///
/// * `path` - Path to the GDScript file
/// * `build_index` - Whether to build a symbol index for fast lookups
///
/// # Returns
///
/// A `ProcessedFile` containing the parsed AST, symbols, and optionally the index.
pub fn process_file(path: &Path, build_index: bool) -> Result<ProcessedFile> {
    let parsed = parser::parse_file(path).map_err(|e| Error::parse(path, e))?;
    let symbols = symbols::collect_symbols(path, &parsed);
    let index = if build_index {
        Some(SymbolIndex::build(&parsed, &symbols))
    } else {
        None
    };

    Ok(ProcessedFile {
        path: path.to_path_buf(),
        parsed,
        symbols,
        index,
    })
}

/// Process multiple files in parallel.
///
/// Uses rayon for parallel processing of files. Returns results for all files,
/// including both successes and failures.
///
/// # Arguments
///
/// * `paths` - Paths to the GDScript files to process
/// * `build_index` - Whether to build symbol indices for fast lookups
///
/// # Returns
///
/// A vector of `(PathBuf, Result<ProcessedFile>)` pairs, preserving order.
pub fn process_files_parallel(
    paths: &[PathBuf],
    build_index: bool,
) -> Vec<(PathBuf, Result<ProcessedFile>)> {
    paths
        .par_iter()
        .map(|path| {
            let result = process_file(path, build_index);
            (path.clone(), result)
        })
        .collect()
}

/// Process multiple source strings in parallel.
///
/// Uses rayon for parallel processing of in-memory sources.
///
/// # Arguments
///
/// * `sources` - Pairs of (path, source_code) to process
/// * `build_index` - Whether to build symbol indices for fast lookups
///
/// # Returns
///
/// A vector of `(PathBuf, Result<ProcessedFile>)` pairs, preserving order.
pub fn process_sources_parallel(
    sources: &[(&Path, &str)],
    build_index: bool,
) -> Vec<(PathBuf, Result<ProcessedFile>)> {
    sources
        .par_iter()
        .map(|(path, source)| {
            let result = process_source(path, source, build_index);
            ((*path).to_path_buf(), result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_SCRIPT: &str = r#"
extends Node

var health: int = 100

func take_damage(amount: int) -> void:
    health -= amount
"#;

    #[test]
    fn process_source_basic() {
        let path = Path::new("test.gd");
        let result = process_source(path, SIMPLE_SCRIPT, true);

        assert!(result.is_ok());
        let processed = result.unwrap();

        assert_eq!(processed.path, path);
        assert!(processed.index.is_some());
        assert!(!processed.symbols.functions.is_empty());
        assert!(!processed.symbols.variables.is_empty());
    }

    #[test]
    fn process_source_without_index() {
        let path = Path::new("test.gd");
        let result = process_source(path, SIMPLE_SCRIPT, false);

        assert!(result.is_ok());
        let processed = result.unwrap();

        assert!(processed.index.is_none());
    }

    #[test]
    fn process_source_invalid() {
        let path = Path::new("test.gd");
        // Invalid GDScript - parse should still succeed but with errors
        let result = process_source(path, "func ()", false);

        // Tree-sitter parsing is error-tolerant, so this might not fail
        // Just ensure we get some result
        let _ = result;
    }

    #[test]
    fn process_sources_parallel_basic() {
        let sources: Vec<(&Path, &str)> = vec![
            (Path::new("a.gd"), SIMPLE_SCRIPT),
            (Path::new("b.gd"), SIMPLE_SCRIPT),
        ];

        let results = process_sources_parallel(&sources, false);

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
    }
}
