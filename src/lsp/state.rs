//! Server state management for the LSP.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Maximum number of documents to cache diagnostics for.
const MAX_CACHED_DIAGNOSTICS: usize = 100;

use crate::cache::AnalysisCache;
use crate::call_graph::CallGraph;
use crate::classdb::ClassDb;
use crate::config::Config;
use crate::document::Document;
use crate::parser::ParsedFile;
use crate::project::ProjectInfo;
use crate::rules::Diagnostic;
use crate::scene::SceneFile;
use crate::symbol_index::SymbolIndex;
use crate::symbols::FileSymbols;

use crate::project_index::{IndexedFile, ProjectIndex};

use super::worker::AnalysisSnapshot;

/// Server state shared across LSP requests.
pub struct ServerState {
    /// Unified project index for all file symbols and lookups.
    pub project_index: ProjectIndex,
    /// Analysis cache for incremental analysis.
    pub cache: AnalysisCache,
    /// Class database for type information (Arc for cheap cloning).
    pub class_db: Arc<ClassDb>,
    /// Configuration.
    pub config: Config,
    /// Whether the server has been initialized.
    pub initialized: bool,
    /// Parsed project.godot data (Arc for cheap cloning to worker).
    pub project_info: Arc<ProjectInfo>,
    /// Parsed scene files (Arc for cheap cloning to worker).
    pub scenes: Arc<HashMap<PathBuf, SceneFile>>,
    /// Project-wide call graph (Arc for cheap cloning to worker).
    pub call_graph: Arc<CallGraph>,
    /// Functions reachable from entry points (Arc for cheap cloning to worker).
    pub reachable_functions: Arc<HashSet<(PathBuf, String)>>,
    /// Cached diagnostics per document (URI -> (version, diagnostics)).
    /// Used by code actions to avoid re-running analysis.
    pub cached_diagnostics: HashMap<String, (i32, Vec<Diagnostic>)>,
    /// Cached all_symbols for cheap cloning to worker.
    /// Invalidated on project index changes.
    all_symbols_cache: Option<Arc<Vec<FileSymbols>>>,
    /// Whether call graph needs rebuilding (set when documents change).
    call_graph_dirty: bool,
}

impl ServerState {
    /// Create a new server state with an empty class database.
    pub fn new() -> Self {
        Self {
            project_index: ProjectIndex::new(),
            cache: AnalysisCache::new(),
            class_db: Arc::new(ClassDb::empty()),
            config: Config::default(),
            initialized: false,
            project_info: Arc::new(ProjectInfo::default()),
            scenes: Arc::new(HashMap::new()),
            call_graph: Arc::new(CallGraph::default()),
            reachable_functions: Arc::new(HashSet::new()),
            cached_diagnostics: HashMap::new(),
            all_symbols_cache: None,
            call_graph_dirty: false,
        }
    }

    /// Create server state with a pre-loaded class database.
    pub fn with_classdb(class_db: ClassDb) -> Self {
        Self {
            project_index: ProjectIndex::new(),
            cache: AnalysisCache::new(),
            class_db: Arc::new(class_db),
            config: Config::default(),
            initialized: false,
            project_info: Arc::new(ProjectInfo::default()),
            scenes: Arc::new(HashMap::new()),
            call_graph: Arc::new(CallGraph::default()),
            reachable_functions: Arc::new(HashSet::new()),
            cached_diagnostics: HashMap::new(),
            all_symbols_cache: None,
            call_graph_dirty: false,
        }
    }

    /// Get the root path of the workspace.
    pub fn root_path(&self) -> Option<&Path> {
        self.project_index.project_root()
    }

    /// Set the root path of the workspace.
    pub fn set_root_path(&mut self, path: PathBuf) {
        self.project_index.set_project_root(path);
    }

    /// Load the bundled class database, respecting target_version from config.
    /// Returns true if the database was loaded or already present.
    pub fn load_classdb(&mut self) -> bool {
        // Check if we have an empty database (not yet loaded)
        if self.class_db.source.to_string().contains("empty") {
            // Use target_version from config if specified
            let version = self.config.target_version.as_deref();
            if let Ok(db) = ClassDb::from_bundled(version) {
                self.class_db = Arc::new(db);
                return true;
            }
            return false;
        }
        true
    }

    /// Get a reference to the class database.
    pub fn get_classdb(&self) -> &ClassDb {
        &self.class_db
    }

    /// Get a cheap clone of the class database Arc.
    pub fn classdb_arc(&self) -> Arc<ClassDb> {
        Arc::clone(&self.class_db)
    }

    /// Get a cheap clone of the scenes Arc.
    pub fn scenes_arc(&self) -> Arc<HashMap<PathBuf, SceneFile>> {
        Arc::clone(&self.scenes)
    }

    /// Get a cheap clone of the project info Arc.
    pub fn project_info_arc(&self) -> Arc<ProjectInfo> {
        Arc::clone(&self.project_info)
    }

    /// Get a cheap clone of the call graph Arc.
    pub fn call_graph_arc(&self) -> Arc<CallGraph> {
        Arc::clone(&self.call_graph)
    }

    /// Get a cheap clone of the reachable functions Arc.
    pub fn reachable_functions_arc(&self) -> Arc<HashSet<(PathBuf, String)>> {
        Arc::clone(&self.reachable_functions)
    }

    /// Get a snapshot of the project-wide state for analysis.
    /// This contains all the shared Arc data needed by AnalysisRequest.
    /// Rebuilds call graph if documents have changed since last snapshot.
    pub fn analysis_snapshot(&mut self) -> AnalysisSnapshot {
        // Rebuild call graph if dirty (documents changed)
        if self.call_graph_dirty {
            self.rebuild_call_graph();
        }

        AnalysisSnapshot {
            class_db: Arc::clone(&self.class_db),
            config: self.config.clone(),
            scenes: Arc::clone(&self.scenes),
            project_info: Arc::clone(&self.project_info),
            call_graph: Arc::clone(&self.call_graph),
            reachable_functions: Arc::clone(&self.reachable_functions),
            all_file_symbols: self.all_symbols_arc(),
        }
    }

    /// Rebuild the call graph from current project index.
    fn rebuild_call_graph(&mut self) {
        let parsed_files: Vec<(PathBuf, &ParsedFile)> = self
            .project_index
            .iter()
            .filter_map(|(path, file)| file.parsed.as_ref().map(|p| (path.to_path_buf(), p)))
            .collect();
        let all_symbols: Vec<FileSymbols> = self
            .project_index
            .iter()
            .map(|(_, file)| (*file.symbols).clone())
            .collect();

        let call_graph = CallGraph::build_from_refs(&parsed_files, &all_symbols, &self.class_db);
        let entry_points =
            crate::call_graph::collect_entry_points_from_refs(&all_symbols, &parsed_files);
        let reachable_functions = call_graph.compute_reachability(&entry_points);

        self.call_graph = Arc::new(call_graph);
        self.reachable_functions = Arc::new(reachable_functions);
        self.call_graph_dirty = false;
    }

    /// Open a document.
    pub fn open_document(&mut self, uri: &str, version: i32, content: String) {
        let path = uri_to_path(uri);

        // Use unified pipeline for parse+symbols+index
        let processed = match crate::pipeline::process_source(&path, &content, true) {
            Ok(p) => p,
            Err(_) => return, // Can't index unparseable files
        };

        // Create indexed file with full data
        let file = IndexedFile::with_full_data(
            processed.symbols,
            processed.parsed,
            processed.index.unwrap(), // We passed build_index=true
            version,
            content,
        );

        self.project_index.insert(path, file);
        self.invalidate_symbols_cache();
    }

    /// Update a document with new content.
    pub fn update_document(&mut self, uri: &str, version: i32, content: String) {
        let path = uri_to_path(uri);

        // Use unified pipeline for parse+symbols+index
        let processed = match crate::pipeline::process_source(&path, &content, true) {
            Ok(p) => p,
            Err(_) => return,
        };

        // Create indexed file with full data
        let file = IndexedFile::with_full_data(
            processed.symbols,
            processed.parsed,
            processed.index.unwrap(), // We passed build_index=true
            version,
            content,
        );

        self.project_index.insert(path, file);
        self.invalidate_symbols_cache();
    }

    /// Close a document.
    pub fn close_document(&mut self, uri: &str) {
        let path = uri_to_path(uri);

        // When closing, we might want to keep the file in the index if it's part of the project,
        // but mark it as not open. For now, we'll keep the project file if it exists,
        // otherwise remove it.
        if let Some(file) = self.project_index.get_mut(&path) {
            // If file was part of project scan, keep symbols but clear open-doc data
            file.version = None;
            file.content = None;
            // Keep parsed and index for navigation in other files
        }
        // Note: We don't remove project files, only clear their "open" status

        // Clean up cached diagnostics for closed document
        self.cached_diagnostics.remove(uri);
    }

    /// Cache diagnostics for a document, evicting old entries if needed.
    pub fn cache_diagnostics(&mut self, uri: String, version: i32, diagnostics: Vec<Diagnostic>) {
        // Evict oldest entries if cache is full
        if self.cached_diagnostics.len() >= MAX_CACHED_DIAGNOSTICS {
            // Simple eviction: remove entries for non-open documents first
            let open_uris: HashSet<_> = self
                .project_index
                .open_files()
                .map(|(p, _)| super::uri::path_to_uri(p))
                .collect();

            // Remove diagnostics for closed documents
            self.cached_diagnostics
                .retain(|uri, _| open_uris.contains(uri));

            // If still over limit, just clear half (simple strategy)
            if self.cached_diagnostics.len() >= MAX_CACHED_DIAGNOSTICS {
                let to_remove: Vec<_> = self
                    .cached_diagnostics
                    .keys()
                    .take(MAX_CACHED_DIAGNOSTICS / 2)
                    .cloned()
                    .collect();
                for key in to_remove {
                    self.cached_diagnostics.remove(&key);
                }
            }
        }

        self.cached_diagnostics.insert(uri, (version, diagnostics));
    }

    /// Get a file by path.
    pub fn get_file(&self, path: &Path) -> Option<&IndexedFile> {
        self.project_index.get(path)
    }

    /// Get a file by URI.
    pub fn get_file_by_uri(&self, uri: &str) -> Option<&IndexedFile> {
        let path = uri_to_path(uri);
        self.project_index.get(&path)
    }

    /// Get symbols for a URI (backwards compatibility).
    pub fn get_symbols(&self, uri: &str) -> Option<&FileSymbols> {
        self.get_file_by_uri(uri).map(|f| f.symbols.as_ref())
    }

    /// Get the parsed file for a URI (backwards compatibility).
    pub fn get_parsed(&self, uri: &str) -> Option<&ParsedFile> {
        self.get_file_by_uri(uri).and_then(|f| f.parsed.as_ref())
    }

    /// Get the symbol index for a URI (backwards compatibility).
    pub fn get_index(&self, uri: &str) -> Option<&SymbolIndex> {
        self.get_file_by_uri(uri).and_then(|f| f.index.as_ref())
    }

    /// Get a Document for a URI (backwards compatibility).
    ///
    /// Creates a Document on-the-fly from the IndexedFile content.
    /// Returns None if the file is not open (no content available).
    pub fn get_document(&self, uri: &str) -> Option<Document> {
        let file = self.get_file_by_uri(uri)?;
        let content = file.content.as_ref()?;
        let version = file.version.unwrap_or(0);
        Some(Document::with_version(
            &file.symbols.path,
            version,
            content.clone(),
        ))
    }

    /// Iterate over all project symbols (backwards compatibility).
    ///
    /// This replaces the old `project_symbols` field.
    pub fn project_symbols(&self) -> impl Iterator<Item = &FileSymbols> {
        self.project_index.iter().map(|(_, f)| f.symbols.as_ref())
    }

    /// Get symbols for a path by looking up in project index.
    ///
    /// Returns an iterator over tuples of (URI-like string, &FileSymbols).
    /// This is for backwards compatibility with code that iterated over `state.symbols`.
    pub fn symbols_iter(&self) -> impl Iterator<Item = (String, &FileSymbols)> {
        self.project_index.iter().map(|(path, f)| {
            let uri = super::uri::path_to_uri(path);
            (uri, f.symbols.as_ref())
        })
    }

    /// Get a class registry-like lookup by class name (backwards compatibility).
    ///
    /// Returns the path to the file defining the given class_name.
    pub fn class_registry_lookup(&self, class_name: &str) -> Option<&Path> {
        self.project_index.path_for_class_name(class_name)
    }

    /// Add a file to the project index (for file watcher events).
    pub fn add_file_to_project_index(&mut self, path: &Path) {
        // Use unified pipeline for parse+symbols+index
        if let Ok(processed) = crate::pipeline::process_file(path, true) {
            let mut file = IndexedFile::new(processed.symbols);
            file.parsed = Some(processed.parsed);
            file.index = processed.index;
            self.project_index.insert(path.to_path_buf(), file);
        }
    }

    /// Analyze a document and cache the results (backwards compatibility).
    /// Note: With ProjectIndex, this is done in open_document/update_document.
    pub fn analyze_document(&mut self, _uri: &str) -> Option<()> {
        // Analysis is now done automatically in open_document/update_document
        Some(())
    }

    /// Get all project symbols as a Vec (for analysis).
    /// Note: Prefer `all_symbols_arc()` when you have mutable access for cached version.
    pub fn all_symbols(&self) -> Vec<FileSymbols> {
        // If we have a cached version, clone the Arc contents
        if let Some(cached) = &self.all_symbols_cache {
            return cached.as_ref().clone();
        }
        // Otherwise build fresh (requires cloning each FileSymbols)
        self.project_index
            .iter()
            .map(|(_, f)| (*f.symbols).clone())
            .collect()
    }

    /// Get all project symbols as a cached Arc (for cheap cloning to worker).
    /// Builds the cache if not already present.
    pub fn all_symbols_arc(&mut self) -> Arc<Vec<FileSymbols>> {
        if let Some(cached) = &self.all_symbols_cache {
            return Arc::clone(cached);
        }
        // Build the cache (requires cloning each FileSymbols once)
        let symbols: Vec<FileSymbols> = self
            .project_index
            .iter()
            .map(|(_, f)| (*f.symbols).clone())
            .collect();
        let arc = Arc::new(symbols);
        self.all_symbols_cache = Some(Arc::clone(&arc));
        arc
    }

    /// Invalidate the all_symbols cache (call when project index changes).
    fn invalidate_symbols_cache(&mut self) {
        self.all_symbols_cache = None;
        self.call_graph_dirty = true;
    }

    /// Scan the project directory for all GDScript files and build the index.
    pub fn scan_project(&mut self) -> usize {
        let root = match self.project_index.project_root() {
            Some(r) => r.to_path_buf(),
            None => return 0,
        };

        // Use AnalysisBuilder to initialize project context
        let ctx = match crate::analysis::AnalysisBuilder::new()
            .project_root(&root)
            .config(self.config.clone())
            .build()
        {
            Ok(ctx) => ctx,
            Err(_) => {
                self.project_info = Arc::new(crate::project::ProjectInfo::default());
                self.scenes = Arc::new(HashMap::new());
                return 0;
            }
        };

        // Update class_db if not already loaded
        if self.class_db.source.to_string().contains("empty") {
            self.class_db = Arc::new(ctx.class_db);
        }
        self.project_info = Arc::new(ctx.project_info);
        self.scenes = Arc::new(ctx.scenes);

        // Discover all .gd files
        let files = crate::analysis::discover_gdscript_files(
            std::slice::from_ref(&root),
            &self.config,
            Some(&root),
        );

        if files.is_empty() {
            return 0;
        }

        // Clear existing non-open files and re-index
        // Keep open files as they may have unsaved changes
        let open_paths: Vec<PathBuf> = self
            .project_index
            .open_files()
            .map(|(p, _)| p.to_path_buf())
            .collect();

        self.project_index.clear();
        self.invalidate_symbols_cache();
        self.project_index.set_project_root(root.clone());

        // Parse all files in parallel using unified pipeline
        let open_paths_set: HashSet<_> = open_paths.iter().collect();
        let paths_to_process: Vec<_> = files
            .iter()
            .filter(|path| !open_paths_set.contains(path))
            .cloned()
            .collect();
        let processed_results = crate::pipeline::process_files_parallel(&paths_to_process, false);

        // Insert results into index (must be sequential due to &mut self)
        for (path, result) in processed_results {
            if let Ok(processed) = result {
                let mut file = IndexedFile::new(processed.symbols);
                file.parsed = Some(processed.parsed);
                file.index = processed.index;
                self.project_index.insert(path, file);
            }
        }

        // Register autoloads
        self.project_index.register_autoloads(&self.project_info);

        // Collect data for call graph from the index
        let parsed_files: Vec<(PathBuf, &ParsedFile)> = self
            .project_index
            .iter()
            .filter_map(|(path, file)| file.parsed.as_ref().map(|p| (path.to_path_buf(), p)))
            .collect();
        let all_symbols: Vec<FileSymbols> = self
            .project_index
            .iter()
            .map(|(_, file)| (*file.symbols).clone())
            .collect();

        // Build call graph using the already-parsed files
        let call_graph = CallGraph::build_from_refs(&parsed_files, &all_symbols, &self.class_db);

        // Compute entry points and reachability
        let entry_points =
            crate::call_graph::collect_entry_points_from_refs(&all_symbols, &parsed_files);
        let reachable_functions = call_graph.compute_reachability(&entry_points);

        self.call_graph = Arc::new(call_graph);
        self.reachable_functions = Arc::new(reachable_functions);
        self.call_graph_dirty = false;

        self.project_index.len()
    }

    /// Find a method or property in a user-defined class by class name.
    pub fn find_member_in_class(&self, class_name: &str, member_name: &str) -> Option<MemberInfo> {
        // Try class_name lookup first, then autoload
        let file = self
            .project_index
            .get_by_class_name(class_name)
            .or_else(|| self.project_index.get_by_autoload(class_name))?;

        let symbols = &file.symbols;

        // Check functions
        for func in &symbols.functions {
            if func.name == member_name {
                return Some(MemberInfo::Function {
                    name: func.name.clone(),
                    return_type: func
                        .return_type
                        .clone()
                        .or_else(|| func.inferred_return_type.clone()),
                    parameters: func
                        .parameters
                        .iter()
                        .map(|p| (p.name.clone(), p.type_annotation.clone()))
                        .collect(),
                    documentation: func.documentation.clone(),
                });
            }
        }

        // Check variables (properties)
        for var in &symbols.variables {
            if var.name == member_name {
                return Some(MemberInfo::Variable {
                    name: var.name.clone(),
                    type_hint: var
                        .type_annotation
                        .clone()
                        .or_else(|| var.inferred_type.clone()),
                    documentation: var.documentation.clone(),
                });
            }
        }

        // Check signals
        for signal in &symbols.signals {
            if signal.name == member_name {
                return Some(MemberInfo::Signal {
                    name: signal.name.clone(),
                    parameters: signal.parameters.clone(),
                    documentation: signal.documentation.clone(),
                });
            }
        }

        // Check constants
        for constant in &symbols.constants {
            if constant.name == member_name {
                return Some(MemberInfo::Constant {
                    name: constant.name.clone(),
                    type_hint: constant.type_annotation.clone(),
                    documentation: constant.documentation.clone(),
                });
            }
        }

        None
    }

    /// Resolve a variable name to its type within the current file's context.
    pub fn resolve_variable_type(&self, uri: &str, var_name: &str) -> Option<String> {
        let file = self.get_file_by_uri(uri)?;
        let symbols = &file.symbols;

        // Check member variables
        for var in &symbols.variables {
            if var.name == var_name {
                return var
                    .type_annotation
                    .clone()
                    .or_else(|| var.inferred_type.clone())
                    .or_else(|| var.initializer_type.clone());
            }
        }

        // Check function parameters and local variables
        for func in &symbols.functions {
            for param in &func.parameters {
                if param.name == var_name {
                    return param
                        .type_annotation
                        .clone()
                        .or_else(|| param.inferred_type.clone());
                }
            }
            for local in &func.local_vars {
                if local.name == var_name {
                    return local
                        .type_annotation
                        .clone()
                        .or_else(|| local.inferred_type.clone())
                        .or_else(|| local.initializer_type.clone());
                }
            }
        }

        // Check if it's an autoload singleton name
        if self.project_index.get_by_autoload(var_name).is_some() {
            return Some(var_name.to_string());
        }

        // Check if it's a class_name (static access)
        if self.project_index.get_by_class_name(var_name).is_some() {
            return Some(var_name.to_string());
        }

        None
    }
}

/// Information about a class member (method, property, signal, constant).
#[derive(Debug, Clone)]
pub enum MemberInfo {
    Function {
        name: String,
        return_type: Option<String>,
        parameters: Vec<(String, Option<String>)>,
        documentation: Option<String>,
    },
    Variable {
        name: String,
        type_hint: Option<String>,
        documentation: Option<String>,
    },
    Signal {
        name: String,
        parameters: Vec<String>,
        documentation: Option<String>,
    },
    Constant {
        name: String,
        type_hint: Option<String>,
        documentation: Option<String>,
    },
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a URI to a PathBuf (lossy - best effort).
pub fn uri_to_path(uri: &str) -> PathBuf {
    super::uri::uri_to_path_lossy(uri)
}
