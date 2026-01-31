//! LSP lifecycle handlers: initialization and document management.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::worker::AnalysisRequest;
use crate::lsp::{capabilities, Backend};

/// Handle the initialize request.
pub async fn initialize(_backend: &Backend, params: InitializeParams) -> Result<InitializeResult> {
    // Store root path if provided
    if let Some(root_uri) = params.root_uri {
        if let Ok(path) = root_uri.to_file_path() {
            let mut state = _backend.state.write().await;
            state.set_root_path(path);
        }
    }

    Ok(InitializeResult {
        capabilities: capabilities::server_capabilities(),
        server_info: Some(ServerInfo {
            name: "gdeye".to_string(),
            version: Some(crate::VERSION.to_string()),
        }),
    })
}

/// Handle the initialized notification.
pub async fn initialized(backend: &Backend) {
    let file_count = {
        let mut state = backend.state.write().await;
        state.initialized = true;

        // Load configuration from gdeye.toml if present
        state.config = crate::config::Config::load_from_project(state.root_path());

        // Try to load the class database
        state.load_classdb();

        // Scan the project for all GDScript files
        state.scan_project()
    };

    backend
        .client
        .log_message(
            MessageType::INFO,
            format!(
                "GDScript language server initialized (indexed {} files)",
                file_count
            ),
        )
        .await;
}

/// Handle document open notification.
pub async fn did_open(backend: &Backend, params: DidOpenTextDocumentParams) {
    let uri = params.text_document.uri.to_string();
    let version = params.text_document.version;
    let content = params.text_document.text.clone();

    // Update document version tracking
    {
        let mut versions = backend.document_versions.write().await;
        versions.insert(uri.clone(), version);
    }

    // Update state and analyze for navigation features
    let analysis_request = {
        let mut state = backend.state.write().await;
        state.open_document(&uri, version, params.text_document.text);
        state.analyze_document(&uri);

        // open_document already handles adding to project_index
        // Build analysis request with project-wide context
        let project_ctx = state.analysis_snapshot();
        state.get_document(&uri).map(|doc| AnalysisRequest {
            uri: uri.clone(),
            path: doc.path().to_path_buf(),
            version,
            content,
            project_ctx,
        })
    };

    // Queue background analysis for diagnostics
    if let Some(request) = analysis_request {
        backend.worker.queue_analysis(request);
    }
}

/// Handle document change notification.
pub async fn did_change(backend: &Backend, params: DidChangeTextDocumentParams) {
    let uri = params.text_document.uri.to_string();
    let version = params.text_document.version;

    // We use full sync, so take the first (and only) change
    let content = match params.content_changes.into_iter().next() {
        Some(change) => change.text,
        None => return,
    };

    // Update document version tracking
    {
        let mut versions = backend.document_versions.write().await;
        versions.insert(uri.clone(), version);
    }

    // Update state and analyze for navigation features
    let analysis_request = {
        let mut state = backend.state.write().await;
        state.update_document(&uri, version, content.clone());
        state.analyze_document(&uri);

        // Build analysis request with project-wide context
        let project_ctx = state.analysis_snapshot();
        state.get_document(&uri).map(|doc| AnalysisRequest {
            uri: uri.clone(),
            path: doc.path().to_path_buf(),
            version,
            content,
            project_ctx,
        })
    };

    // Queue background analysis for diagnostics (debounced)
    if let Some(request) = analysis_request {
        backend.worker.queue_analysis(request);
    }
}

/// Handle document save notification.
pub async fn did_save(backend: &Backend, params: DidSaveTextDocumentParams) {
    let uri = params.text_document.uri.to_string();

    // If text was included, update the document
    let Some(text) = params.text else {
        return;
    };

    // Update state and build analysis request with project-wide context
    let (version, analysis_request) = {
        let mut state = backend.state.write().await;
        let Some(doc) = state.get_document(&uri) else {
            return;
        };
        let version = doc.version() + 1;
        let path = doc.path().to_path_buf();

        state.update_document(&uri, version, text.clone());
        state.analyze_document(&uri);

        let project_ctx = state.analysis_snapshot();
        let request = AnalysisRequest {
            uri: uri.clone(),
            path,
            version,
            content: text,
            project_ctx,
        };
        (version, request)
    };

    // Update version tracking
    {
        let mut versions = backend.document_versions.write().await;
        versions.insert(uri.clone(), version);
    }

    // Queue background analysis for diagnostics
    backend.worker.queue_analysis(analysis_request);
}

/// Handle document close notification.
pub async fn did_close(backend: &Backend, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri.to_string();

    // Remove from version tracking
    {
        let mut versions = backend.document_versions.write().await;
        versions.remove(&uri);
    }

    // Remove from state
    {
        let mut state = backend.state.write().await;
        state.close_document(&uri);
    }

    // Clear diagnostics for closed document
    backend
        .client
        .publish_diagnostics(params.text_document.uri, vec![], None)
        .await;
}

/// Handle workspace/didChangeWatchedFiles notification.
pub async fn did_change_watched_files(backend: &Backend, params: DidChangeWatchedFilesParams) {
    let mut has_project_changes = false;
    let mut has_config_changes = false;

    for change in &params.changes {
        let path = change.uri.path();
        if path.ends_with(".gd") || path.ends_with("project.godot") || path.ends_with(".tscn") {
            has_project_changes = true;
        }
        if path.ends_with("gdeye.toml") {
            has_config_changes = true;
        }
    }

    // Reload config if gdeye.toml changed
    if has_config_changes {
        let mut state = backend.state.write().await;
        state.config = crate::config::Config::load_from_project(state.root_path());

        // Reload classdb if target_version changed
        state.load_classdb();

        backend
            .client
            .log_message(MessageType::INFO, "Reloaded gdeye.toml configuration")
            .await;

        // Re-scan project to rebuild with new config
        state.scan_project();
    } else if has_project_changes {
        // Re-scan project on file system changes to pick up new/deleted files
        let mut state = backend.state.write().await;
        state.scan_project();
    }
}
