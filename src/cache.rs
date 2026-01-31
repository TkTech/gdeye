//! Analysis cache for incremental compilation.
//!
//! This module provides caching infrastructure for storing and reusing
//! analysis results. It tracks file versions to determine when cached
//! data is stale and needs to be recomputed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::call_graph::CallGraph;
use crate::cfg::Cfg;
use crate::flow::FlowResults;
use crate::parser::ParsedFile;
use crate::project_index::ProjectIndex;
use crate::rules::Diagnostic;
use crate::symbols::FileSymbols;

/// Default maximum number of files to cache before LRU eviction.
const DEFAULT_MAX_CACHE_FILES: usize = 500;

/// Cache for analysis results.
///
/// The cache stores per-file analysis results and project-wide data.
/// It tracks versions to determine staleness and supports incremental
/// invalidation when files change.
#[derive(Debug)]
pub struct AnalysisCache {
    /// Per-file cached data.
    files: HashMap<PathBuf, CachedFile>,
    /// Project-wide cached data (invalidated when any file changes).
    project: Option<CachedProjectData>,
    /// Statistics for cache performance monitoring.
    stats: CacheStats,
    /// Maximum number of files to cache before LRU eviction.
    max_files: usize,
}

/// Cached analysis data for a single file.
#[derive(Debug)]
pub struct CachedFile {
    /// Version of the document when this was cached.
    pub version: i32,
    /// Hash of the content (for staleness checks without version tracking).
    pub content_hash: u64,
    /// Parsed AST.
    pub parsed: ParsedFile,
    /// Extracted symbols.
    pub symbols: FileSymbols,
    /// Control flow graphs (if computed).
    pub cfgs: Option<Vec<Cfg>>,
    /// Flow analysis results (if computed).
    pub flow_results: Option<FlowResults>,
    /// Diagnostics (if computed).
    pub diagnostics: Option<Vec<Diagnostic>>,
    /// When this entry was last accessed.
    pub last_accessed: Instant,
}

/// Project-wide cached data.
#[derive(Debug)]
pub struct CachedProjectData {
    /// Project index for cross-file lookups.
    pub project_index: ProjectIndex,
    /// Call graph for the project.
    pub call_graph: CallGraph,
    /// Versions of files when this was computed.
    pub file_versions: HashMap<PathBuf, i32>,
}

/// Cache performance statistics.
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: usize,
    /// Number of cache misses.
    pub misses: usize,
    /// Number of invalidations.
    pub invalidations: usize,
}

impl Default for AnalysisCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisCache {
    /// Create a new empty cache with default size limits.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            project: None,
            stats: CacheStats::default(),
            max_files: DEFAULT_MAX_CACHE_FILES,
        }
    }

    /// Create a new cache with a custom maximum file limit.
    pub fn with_max_files(max_files: usize) -> Self {
        Self {
            files: HashMap::new(),
            project: None,
            stats: CacheStats::default(),
            max_files,
        }
    }

    /// Get cached data for a file if it's still valid.
    ///
    /// Returns `None` if the file is not cached or the cached version
    /// doesn't match the requested version.
    pub fn get(&mut self, path: &Path, version: i32) -> Option<&CachedFile> {
        if let Some(entry) = self.files.get_mut(path) {
            if entry.version == version {
                entry.last_accessed = Instant::now();
                self.stats.hits += 1;
                return self.files.get(path);
            }
        }
        self.stats.misses += 1;
        None
    }

    /// Get cached data for a file by content hash.
    ///
    /// This is useful when version tracking is not available.
    pub fn get_by_hash(&mut self, path: &Path, content_hash: u64) -> Option<&CachedFile> {
        if let Some(entry) = self.files.get_mut(path) {
            if entry.content_hash == content_hash {
                entry.last_accessed = Instant::now();
                self.stats.hits += 1;
                return self.files.get(path);
            }
        }
        self.stats.misses += 1;
        None
    }

    /// Check if cached data exists and is valid for a file.
    pub fn is_valid(&self, path: &Path, version: i32) -> bool {
        self.files
            .get(path)
            .is_some_and(|entry| entry.version == version)
    }

    /// Store parsed file data in the cache.
    ///
    /// Automatically evicts least-recently-used entries if the cache
    /// exceeds the configured maximum size.
    pub fn store_parsed(
        &mut self,
        path: PathBuf,
        version: i32,
        content_hash: u64,
        parsed: ParsedFile,
        symbols: FileSymbols,
    ) {
        // Invalidate project-wide data since a file changed
        self.invalidate_project();

        self.files.insert(
            path,
            CachedFile {
                version,
                content_hash,
                parsed,
                symbols,
                cfgs: None,
                flow_results: None,
                diagnostics: None,
                last_accessed: Instant::now(),
            },
        );

        // Enforce size limit via LRU eviction
        if self.files.len() > self.max_files {
            self.evict_lru(self.max_files);
        }
    }

    /// Update CFGs and flow results for a cached file.
    pub fn store_analysis(
        &mut self,
        path: &Path,
        cfgs: Vec<Cfg>,
        flow_results: FlowResults,
        diagnostics: Vec<Diagnostic>,
    ) {
        if let Some(entry) = self.files.get_mut(path) {
            entry.cfgs = Some(cfgs);
            entry.flow_results = Some(flow_results);
            entry.diagnostics = Some(diagnostics);
            entry.last_accessed = Instant::now();
        }
    }

    /// Invalidate cached data for a specific file.
    pub fn invalidate(&mut self, path: &Path) {
        if self.files.remove(path).is_some() {
            self.stats.invalidations += 1;
            self.invalidate_project();
        }
    }

    /// Invalidate project-wide cached data.
    pub fn invalidate_project(&mut self) {
        if self.project.is_some() {
            self.project = None;
            self.stats.invalidations += 1;
        }
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.stats.invalidations += self.files.len();
        self.files.clear();
        self.project = None;
    }

    /// Store project-wide data.
    pub fn store_project(&mut self, data: CachedProjectData) {
        self.project = Some(data);
    }

    /// Get project-wide data if valid.
    ///
    /// Returns `None` if no project data is cached or if any file
    /// has changed since the project data was computed.
    pub fn get_project(&self) -> Option<&CachedProjectData> {
        let project = self.project.as_ref()?;

        // Check if all file versions still match
        for (path, &cached_version) in &project.file_versions {
            match self.files.get(path) {
                Some(entry) if entry.version == cached_version => continue,
                _ => return None,
            }
        }

        Some(project)
    }

    /// Check if project-wide reanalysis is needed.
    pub fn needs_project_reanalysis(&self) -> bool {
        self.get_project().is_none()
    }

    /// Get cache statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get the number of cached files.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Evict least recently used entries to reduce memory usage.
    ///
    /// Keeps at most `max_files` entries in the cache.
    pub fn evict_lru(&mut self, max_files: usize) {
        if self.files.len() <= max_files {
            return;
        }

        // Collect paths sorted by last access time
        let mut entries: Vec<_> = self
            .files
            .iter()
            .map(|(path, entry)| (path.clone(), entry.last_accessed))
            .collect();
        entries.sort_by_key(|(_, time)| *time);

        // Remove oldest entries
        let to_remove = self.files.len() - max_files;
        for (path, _) in entries.into_iter().take(to_remove) {
            self.files.remove(&path);
            self.stats.invalidations += 1;
        }

        // Project data is likely stale after eviction
        self.invalidate_project();
    }

    /// Get all cached file paths.
    pub fn cached_paths(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(|p| p.as_path())
    }

    /// Get mutable access to symbols for a cached file.
    ///
    /// This is needed for cross-file resolution passes that mutate symbols.
    pub fn symbols_mut(&mut self, path: &Path) -> Option<&mut FileSymbols> {
        self.files.get_mut(path).map(|entry| &mut entry.symbols)
    }

    /// Collect all symbols from cached files in order.
    pub fn all_symbols(&self, paths: &[PathBuf]) -> Vec<&FileSymbols> {
        paths
            .iter()
            .filter_map(|p| self.files.get(p).map(|e| &e.symbols))
            .collect()
    }
}

impl CacheStats {
    /// Calculate the cache hit rate (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Reset statistics.
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.invalidations = 0;
    }
}

/// Compute a hash of the content for cache validation.
pub fn hash_content(content: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_file() -> (ParsedFile, FileSymbols) {
        let parsed = crate::parser::parse_source("var x = 1\n").unwrap();
        let symbols = crate::symbols::collect_symbols(Path::new("test.gd"), &parsed);
        (parsed, symbols)
    }

    #[test]
    fn cache_store_and_retrieve() {
        let mut cache = AnalysisCache::new();
        let (parsed, symbols) = make_test_file();
        let hash = hash_content("var x = 1\n");

        cache.store_parsed(PathBuf::from("test.gd"), 1, hash, parsed, symbols);

        assert!(cache.get(Path::new("test.gd"), 1).is_some());
        assert!(cache.get(Path::new("test.gd"), 2).is_none());
        assert!(cache.get(Path::new("other.gd"), 1).is_none());
    }

    #[test]
    fn cache_invalidation() {
        let mut cache = AnalysisCache::new();
        let (parsed, symbols) = make_test_file();
        let hash = hash_content("var x = 1\n");

        cache.store_parsed(PathBuf::from("test.gd"), 1, hash, parsed, symbols);
        assert!(cache.is_valid(Path::new("test.gd"), 1));

        cache.invalidate(Path::new("test.gd"));
        assert!(!cache.is_valid(Path::new("test.gd"), 1));
    }

    #[test]
    fn cache_stats() {
        let mut cache = AnalysisCache::new();
        let (parsed, symbols) = make_test_file();
        let hash = hash_content("var x = 1\n");

        cache.store_parsed(PathBuf::from("test.gd"), 1, hash, parsed, symbols);

        // Hit
        cache.get(Path::new("test.gd"), 1);
        assert_eq!(cache.stats().hits, 1);

        // Miss (wrong version)
        cache.get(Path::new("test.gd"), 2);
        assert_eq!(cache.stats().misses, 1);

        // Miss (wrong path)
        cache.get(Path::new("other.gd"), 1);
        assert_eq!(cache.stats().misses, 2);

        assert!(cache.stats().hit_rate() > 0.3);
    }

    #[test]
    fn cache_lru_eviction() {
        let mut cache = AnalysisCache::new();

        // Add 5 files
        for i in 0..5 {
            let (parsed, symbols) = make_test_file();
            let path = PathBuf::from(format!("file{}.gd", i));
            cache.store_parsed(path, 1, i as u64, parsed, symbols);
        }

        assert_eq!(cache.file_count(), 5);

        // Access file2 to make it more recently used
        cache.get(Path::new("file2.gd"), 1);

        // Evict to keep only 2 files
        cache.evict_lru(2);

        assert_eq!(cache.file_count(), 2);
        // file2 should still be there (recently accessed)
        assert!(cache.is_valid(Path::new("file2.gd"), 1));
    }

    #[test]
    fn hash_content_consistency() {
        let content = "func test():\n    pass\n";
        let hash1 = hash_content(content);
        let hash2 = hash_content(content);
        assert_eq!(hash1, hash2);

        let hash3 = hash_content("func test():\n    return\n");
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn get_by_hash() {
        let mut cache = AnalysisCache::new();
        let content = "var x = 1\n";
        let (parsed, symbols) = make_test_file();
        let hash = hash_content(content);

        cache.store_parsed(PathBuf::from("test.gd"), 1, hash, parsed, symbols);

        assert!(cache.get_by_hash(Path::new("test.gd"), hash).is_some());
        assert!(cache.get_by_hash(Path::new("test.gd"), hash + 1).is_none());
    }
}
