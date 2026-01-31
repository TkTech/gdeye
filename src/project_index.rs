//! Unified project index for file symbol storage.
//!
//! This module provides a single source of truth for all file symbols,
//! used by both CLI and LSP code paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::parser::ParsedFile;
use crate::project::ProjectInfo;
use crate::symbol_index::SymbolIndex;
use crate::symbols::FileSymbols;

/// Maximum size of the path normalization cache before clearing.
const MAX_PATH_CACHE_SIZE: usize = 1000;

/// A single indexed file with all associated data.
#[derive(Debug)]
pub struct IndexedFile {
    /// Extracted symbols from this file (Arc for cheap sharing).
    pub symbols: Arc<FileSymbols>,
    /// Parsed syntax tree (optional - may be dropped for memory).
    pub parsed: Option<ParsedFile>,
    /// Symbol index for fast lookups.
    pub index: Option<SymbolIndex>,
    /// Document version if this file is open (Some = open, None = on disk).
    pub version: Option<i32>,
    /// Whether this file needs re-parsing.
    pub dirty: bool,
    /// The original source content (for open documents).
    pub content: Option<String>,
}

impl IndexedFile {
    /// Create a new indexed file from symbols.
    pub fn new(symbols: FileSymbols) -> Self {
        Self {
            symbols: Arc::new(symbols),
            parsed: None,
            index: None,
            version: None,
            dirty: false,
            content: None,
        }
    }

    /// Create an indexed file with full data (for open documents).
    pub fn with_full_data(
        symbols: FileSymbols,
        parsed: ParsedFile,
        index: SymbolIndex,
        version: i32,
        content: String,
    ) -> Self {
        Self {
            symbols: Arc::new(symbols),
            parsed: Some(parsed),
            index: Some(index),
            version: Some(version),
            dirty: false,
            content: Some(content),
        }
    }

    /// Check if this file is currently open in the editor.
    pub fn is_open(&self) -> bool {
        self.version.is_some()
    }

    /// Get the path to this file.
    pub fn path(&self) -> &Path {
        &self.symbols.path
    }

    /// Get the class_name declared in this file, if any.
    pub fn class_name(&self) -> Option<&str> {
        self.symbols.class_name.as_deref()
    }
}

/// Unified index of all project files.
///
/// Provides a single source of truth for file symbols, class names,
/// autoloads, and resource paths. Used by both CLI batch analysis
/// and LSP incremental analysis.
#[derive(Debug)]
pub struct ProjectIndex {
    /// All indexed files by normalized (canonical) path.
    files: HashMap<PathBuf, IndexedFile>,
    /// class_name -> path lookup for quick class resolution.
    class_names: HashMap<String, PathBuf>,
    /// Autoload singleton name -> path lookup.
    autoloads: HashMap<String, PathBuf>,
    /// res:// path -> filesystem path mapping.
    res_paths: HashMap<String, PathBuf>,
    /// Project root path (for res:// resolution).
    project_root: Option<PathBuf>,
    /// Cache of path normalization to avoid repeated canonicalize() syscalls.
    /// Maps original paths to their canonical forms.
    path_cache: HashMap<PathBuf, PathBuf>,
    /// Symbol name -> paths containing that symbol (for fast cross-file lookups).
    /// Includes functions, variables, signals, constants, and enums.
    symbol_names: HashMap<String, Vec<PathBuf>>,
    /// Ordered list of paths for index-based lookups.
    path_order: Vec<PathBuf>,
}

impl ProjectIndex {
    /// Create a new empty project index.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            class_names: HashMap::new(),
            autoloads: HashMap::new(),
            res_paths: HashMap::new(),
            project_root: None,
            path_cache: HashMap::new(),
            symbol_names: HashMap::new(),
            path_order: Vec::new(),
        }
    }

    /// Set the project root path.
    pub fn set_project_root(&mut self, root: PathBuf) {
        self.project_root = Some(root);
        // Clear path cache since paths may resolve differently with new root
        self.path_cache.clear();
    }

    /// Get the project root path.
    pub fn project_root(&self) -> Option<&Path> {
        self.project_root.as_deref()
    }

    /// Normalize a path to its canonical form for consistent lookups.
    /// Uses cache to avoid repeated filesystem syscalls.
    fn normalize_path(&mut self, path: &Path) -> PathBuf {
        // Check cache first
        if let Some(cached) = self.path_cache.get(path) {
            return cached.clone();
        }
        // Check if it's already a known canonical path (common case)
        if self.files.contains_key(path) {
            return path.to_path_buf();
        }
        // Fall back to filesystem canonicalization
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        // Enforce cache size limit (simple clear strategy)
        if self.path_cache.len() >= MAX_PATH_CACHE_SIZE {
            self.path_cache.clear();
        }

        // Cache the result
        self.path_cache
            .insert(path.to_path_buf(), canonical.clone());
        canonical
    }

    /// Normalize path for read-only operations (checks cache but can't update it).
    fn normalize_path_readonly(&self, path: &Path) -> PathBuf {
        // Check cache first
        if let Some(cached) = self.path_cache.get(path) {
            return cached.clone();
        }
        // Check if it's already a known canonical path (common case)
        if self.files.contains_key(path) {
            return path.to_path_buf();
        }
        // Fall back to filesystem canonicalization
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    /// Add or update a file in the index.
    pub fn insert(&mut self, path: PathBuf, file: IndexedFile) {
        let normalized = self.normalize_path(&path);

        // If updating, first remove old symbol entries
        let is_new = !self.files.contains_key(&normalized);
        if !is_new {
            self.remove_symbol_entries(&normalized);
        }

        // Update class_name lookup
        if let Some(class_name) = file.class_name() {
            self.class_names
                .insert(class_name.to_string(), normalized.clone());
        }

        // Update res:// path mapping
        if let Some(ref root) = self.project_root {
            if let Ok(root_canonical) = root.canonicalize() {
                if let Ok(rel) = normalized.strip_prefix(&root_canonical) {
                    let res_path = format!("res://{}", rel.display());
                    self.res_paths.insert(res_path, normalized.clone());
                }
            }
        }

        // Update symbol name index
        self.add_symbol_entries(&normalized, &file.symbols);

        self.files.insert(normalized.clone(), file);

        // Add to path order for index-based lookups (only if new)
        if is_new {
            self.path_order.push(normalized);
        }
    }

    /// Add symbol name entries for a file.
    fn add_symbol_entries(&mut self, path: &Path, symbols: &FileSymbols) {
        let path_buf = path.to_path_buf();

        // Index functions
        for func in &symbols.functions {
            self.symbol_names
                .entry(func.name.clone())
                .or_default()
                .push(path_buf.clone());
        }
        // Index variables
        for var in &symbols.variables {
            self.symbol_names
                .entry(var.name.clone())
                .or_default()
                .push(path_buf.clone());
        }
        // Index signals
        for sig in &symbols.signals {
            self.symbol_names
                .entry(sig.name.clone())
                .or_default()
                .push(path_buf.clone());
        }
        // Index constants
        for con in &symbols.constants {
            self.symbol_names
                .entry(con.name.clone())
                .or_default()
                .push(path_buf.clone());
        }
        // Index enums
        for en in &symbols.enums {
            self.symbol_names
                .entry(en.name.clone())
                .or_default()
                .push(path_buf.clone());
        }
    }

    /// Remove symbol name entries for a file.
    fn remove_symbol_entries(&mut self, path: &Path) {
        // Remove this path from all symbol entries
        for paths in self.symbol_names.values_mut() {
            paths.retain(|p| p != path);
        }
        // Clean up empty entries
        self.symbol_names.retain(|_, paths| !paths.is_empty());
    }

    /// Get a file by path.
    pub fn get(&self, path: &Path) -> Option<&IndexedFile> {
        let normalized = self.normalize_path_readonly(path);
        self.files.get(&normalized)
    }

    /// Get a mutable reference to a file by path.
    pub fn get_mut(&mut self, path: &Path) -> Option<&mut IndexedFile> {
        let normalized = self.normalize_path(path);
        self.files.get_mut(&normalized)
    }

    /// Remove a file from the index.
    pub fn remove(&mut self, path: &Path) -> Option<IndexedFile> {
        let normalized = self.normalize_path(path);
        if let Some(file) = self.files.remove(&normalized) {
            // Clean up lookups
            if let Some(class_name) = file.class_name() {
                self.class_names.remove(class_name);
            }
            // Clean up res:// mapping
            self.res_paths.retain(|_, p| *p != normalized);
            // Clean up path cache entries pointing to this file
            self.path_cache.retain(|_, v| *v != normalized);
            // Clean up symbol name entries
            self.remove_symbol_entries(&normalized);
            // Remove from path order
            self.path_order.retain(|p| *p != normalized);
            Some(file)
        } else {
            None
        }
    }

    /// Check if a file exists in the index.
    pub fn contains(&self, path: &Path) -> bool {
        let normalized = self.normalize_path_readonly(path);
        self.files.contains_key(&normalized)
    }

    /// Get a file by its class_name.
    pub fn get_by_class_name(&self, class_name: &str) -> Option<&IndexedFile> {
        self.class_names
            .get(class_name)
            .and_then(|path| self.files.get(path))
    }

    /// Get the path for a class_name.
    pub fn path_for_class_name(&self, class_name: &str) -> Option<&Path> {
        self.class_names.get(class_name).map(|p| p.as_path())
    }

    /// Get a file by its autoload name.
    pub fn get_by_autoload(&self, autoload_name: &str) -> Option<&IndexedFile> {
        self.autoloads
            .get(autoload_name)
            .and_then(|path| self.files.get(path))
    }

    /// Get the path for an autoload name.
    pub fn path_for_autoload(&self, autoload_name: &str) -> Option<&Path> {
        self.autoloads.get(autoload_name).map(|p| p.as_path())
    }

    /// Get a file by its res:// path.
    pub fn get_by_res_path(&self, res_path: &str) -> Option<&IndexedFile> {
        self.res_paths
            .get(res_path)
            .and_then(|path| self.files.get(path))
    }

    /// Resolve a res:// path to a filesystem path.
    pub fn resolve_res_path(&self, res_path: &str) -> Option<&Path> {
        self.res_paths.get(res_path).map(|p| p.as_path())
    }

    /// Get files containing a symbol with the given name.
    ///
    /// Returns paths to files that define a function, variable, signal,
    /// constant, or enum with this name. O(1) lookup via hash index.
    pub fn files_with_symbol(&self, name: &str) -> Option<&[PathBuf]> {
        self.symbol_names.get(name).map(|v| v.as_slice())
    }

    /// Get indexed files for a symbol name.
    ///
    /// Convenience method that returns the actual IndexedFile references.
    pub fn get_by_symbol_name(&self, name: &str) -> Vec<&IndexedFile> {
        self.symbol_names
            .get(name)
            .map(|paths| paths.iter().filter_map(|p| self.files.get(p)).collect())
            .unwrap_or_default()
    }

    /// Iterate over all files.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &IndexedFile)> {
        self.files.iter().map(|(p, f)| (p.as_path(), f))
    }

    /// Iterate over all files mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Path, &mut IndexedFile)> {
        self.files.iter_mut().map(|(p, f)| (p.as_path(), f))
    }

    /// Get all class_name -> path mappings.
    pub fn class_names(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.class_names
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_path()))
    }

    /// Get all autoload -> path mappings.
    pub fn autoloads(&self) -> impl Iterator<Item = (&str, &Path)> {
        self.autoloads
            .iter()
            .map(|(n, p)| (n.as_str(), p.as_path()))
    }

    /// Register autoloads from project info.
    pub fn register_autoloads(&mut self, project_info: &ProjectInfo) {
        for (name, res_path) in &project_info.autoloads {
            let clean_path = res_path.trim_start_matches('*');
            if let Some(fs_path) = self.res_paths.get(clean_path) {
                self.autoloads.insert(name.clone(), fs_path.clone());
            }
        }
    }

    /// Get the number of indexed files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Mark a file as dirty (needs re-parsing).
    pub fn mark_dirty(&mut self, path: &Path) {
        if let Some(file) = self.get_mut(path) {
            file.dirty = true;
        }
    }

    /// Get all open files (documents with a version).
    pub fn open_files(&self) -> impl Iterator<Item = (&Path, &IndexedFile)> {
        self.files
            .iter()
            .filter(|(_, f)| f.is_open())
            .map(|(p, f)| (p.as_path(), f))
    }

    /// Clear all files from the index.
    pub fn clear(&mut self) {
        self.files.clear();
        self.class_names.clear();
        self.autoloads.clear();
        self.res_paths.clear();
        self.path_cache.clear();
        self.path_order.clear();
    }

    /// Collect all FileSymbols as a Vec (for backwards compatibility).
    pub fn all_symbols(&self) -> Vec<&FileSymbols> {
        self.files.values().map(|f| f.symbols.as_ref()).collect()
    }

    /// Get the file index for a given path.
    pub fn index_for_path(&self, path: &Path) -> Option<usize> {
        let normalized = self.normalize_path_readonly(path);
        self.path_order.iter().position(|p| p == &normalized)
    }

    /// Get the file index for a class_name.
    pub fn index_for_class_name(&self, class_name: &str) -> Option<usize> {
        self.class_names
            .get(class_name)
            .and_then(|path| self.index_for_path(path))
    }

    /// Get the file index for an autoload name.
    pub fn index_for_autoload(&self, autoload_name: &str) -> Option<usize> {
        self.autoloads
            .get(autoload_name)
            .and_then(|path| self.index_for_path(path))
    }

    /// Get the file index for a res:// path.
    pub fn index_for_res_path(&self, res_path: &str) -> Option<usize> {
        self.res_paths
            .get(res_path)
            .and_then(|path| self.index_for_path(path))
    }

    /// Build the index from a slice of FileSymbols.
    pub fn build_from_symbols(
        file_symbols: &[FileSymbols],
        project_info: &ProjectInfo,
        project_root: Option<&Path>,
    ) -> Self {
        let mut index = Self::new();

        if let Some(root) = project_root {
            index.set_project_root(root.to_path_buf());
        }

        for fs in file_symbols {
            let file = IndexedFile::new((*fs).clone());
            index.insert(fs.path.clone(), file);
        }

        index.register_autoloads(project_info);

        index
    }
}

impl Default for ProjectIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbols(path: &str, class_name: Option<&str>) -> FileSymbols {
        FileSymbols {
            path: PathBuf::from(path),
            class_name: class_name.map(String::from),
            extends: None,
            signals: Vec::new(),
            enums: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            inner_classes: Vec::new(),
            parent_file: None,
            autoloads: std::collections::HashSet::new(),
            preloads: Vec::new(),
        }
    }

    #[test]
    fn insert_and_get() {
        let mut index = ProjectIndex::new();
        let symbols = make_symbols("/test/file.gd", None);
        index.insert(PathBuf::from("/test/file.gd"), IndexedFile::new(symbols));

        assert!(index.contains(Path::new("/test/file.gd")));
        assert!(index.get(Path::new("/test/file.gd")).is_some());
    }

    #[test]
    fn class_name_lookup() {
        let mut index = ProjectIndex::new();
        let symbols = make_symbols("/test/player.gd", Some("Player"));
        index.insert(PathBuf::from("/test/player.gd"), IndexedFile::new(symbols));

        assert!(index.get_by_class_name("Player").is_some());
        assert!(index.get_by_class_name("Enemy").is_none());
    }

    #[test]
    fn index_for_class_name() {
        let mut index = ProjectIndex::new();
        let symbols = make_symbols("/test/player.gd", Some("Player"));
        index.insert(PathBuf::from("/test/player.gd"), IndexedFile::new(symbols));

        assert!(index.index_for_class_name("Player").is_some());
        assert!(index.index_for_class_name("Enemy").is_none());
    }

    #[test]
    fn remove_cleans_up_lookups() {
        let mut index = ProjectIndex::new();
        let symbols = make_symbols("/test/player.gd", Some("Player"));
        index.insert(PathBuf::from("/test/player.gd"), IndexedFile::new(symbols));

        assert!(index.get_by_class_name("Player").is_some());

        index.remove(Path::new("/test/player.gd"));

        assert!(index.get_by_class_name("Player").is_none());
        assert!(!index.contains(Path::new("/test/player.gd")));
    }

    #[test]
    fn iteration() {
        let mut index = ProjectIndex::new();
        index.insert(
            PathBuf::from("/test/a.gd"),
            IndexedFile::new(make_symbols("/test/a.gd", None)),
        );
        index.insert(
            PathBuf::from("/test/b.gd"),
            IndexedFile::new(make_symbols("/test/b.gd", None)),
        );

        let paths: Vec<_> = index.iter().map(|(p, _)| p.to_path_buf()).collect();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn open_files_filter() {
        let mut index = ProjectIndex::new();

        let mut open_file = IndexedFile::new(make_symbols("/test/open.gd", None));
        open_file.version = Some(1);
        index.insert(PathBuf::from("/test/open.gd"), open_file);

        let disk_file = IndexedFile::new(make_symbols("/test/disk.gd", None));
        index.insert(PathBuf::from("/test/disk.gd"), disk_file);

        let open_count = index.open_files().count();
        assert_eq!(open_count, 1);
    }

    #[test]
    fn path_order_maintained() {
        let mut index = ProjectIndex::new();
        index.insert(
            PathBuf::from("/test/a.gd"),
            IndexedFile::new(make_symbols("/test/a.gd", Some("A"))),
        );
        index.insert(
            PathBuf::from("/test/b.gd"),
            IndexedFile::new(make_symbols("/test/b.gd", Some("B"))),
        );

        // The indices should be assigned in insertion order
        let idx_a = index.index_for_class_name("A");
        let idx_b = index.index_for_class_name("B");

        assert!(idx_a.is_some());
        assert!(idx_b.is_some());
        assert_ne!(idx_a, idx_b);
    }
}
