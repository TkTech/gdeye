//! Background analysis worker with debouncing support.
//!
//! This module provides infrastructure for running CPU-intensive analysis
//! in background tasks without blocking the LSP event loop.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, RwLock};
use tower_lsp::lsp_types::{Diagnostic, Url};
use tower_lsp::Client;

use crate::call_graph::CallGraph;
use crate::classdb::ClassDb;
use crate::config::Config;
use crate::project::ProjectInfo;
use crate::scene::SceneFile;
use crate::symbols::FileSymbols;

use super::convert::to_lsp_diagnostic_with_uri;
use super::ServerState;

/// Default debounce delay for analysis requests.
const DEFAULT_DEBOUNCE_MS: u64 = 300;

/// Maximum pending analysis requests before backpressure kicks in.
/// This prevents memory exhaustion under sustained rapid editing.
const MAX_PENDING_REQUESTS: usize = 100;

/// Maximum time allowed for a single file analysis before timeout.
/// This prevents runaway analysis from blocking the thread pool.
const ANALYSIS_TIMEOUT_SECS: u64 = 30;

/// Snapshot of project-wide state for analysis.
///
/// Contains all the expensive-to-copy data wrapped in Arc for cheap cloning
/// to worker threads. This is a point-in-time snapshot of the project state
/// used by the analysis worker.
///
/// Note: This is distinct from `analysis::ProjectContext` which is the CLI's
/// project context containing owned data for batch analysis.
#[derive(Clone)]
pub struct AnalysisSnapshot {
    /// Class database for type information.
    pub class_db: Arc<ClassDb>,
    /// Configuration.
    pub config: Config,
    /// Scene files for cross-file analysis.
    pub scenes: Arc<HashMap<PathBuf, SceneFile>>,
    /// Project info for autoloads and settings.
    pub project_info: Arc<ProjectInfo>,
    /// Project-wide call graph for cross-file analysis.
    pub call_graph: Arc<CallGraph>,
    /// Functions reachable from entry points.
    pub reachable_functions: Arc<HashSet<(PathBuf, String)>>,
    /// All project symbols for cross-file analysis.
    pub all_file_symbols: Arc<Vec<FileSymbols>>,
}

/// Request to analyze a document.
#[derive(Clone)]
pub struct AnalysisRequest {
    /// Document URI.
    pub uri: String,
    /// Document path.
    pub path: PathBuf,
    /// Document version (for staleness detection).
    pub version: i32,
    /// Document content.
    pub content: String,
    /// Project-wide state snapshot for analysis.
    pub project_ctx: AnalysisSnapshot,
}

/// Result of document analysis.
pub struct AnalysisResult {
    /// Document URI.
    pub uri: String,
    /// Document version that was analyzed.
    pub version: i32,
    /// LSP diagnostics for publishing.
    pub lsp_diagnostics: Vec<Diagnostic>,
    /// Raw diagnostics with fix data for code actions.
    pub raw_diagnostics: Vec<crate::rules::Diagnostic>,
}

/// Manages background analysis with debouncing.
pub struct AnalysisWorker {
    /// Channel to send analysis requests (bounded to prevent memory exhaustion).
    request_tx: mpsc::Sender<AnalysisRequest>,
    /// Debounce delay.
    debounce_delay: Duration,
}

impl AnalysisWorker {
    /// Create a new analysis worker.
    ///
    /// Returns the worker and spawns background tasks for processing.
    pub fn new(
        client: Client,
        current_versions: Arc<RwLock<HashMap<String, i32>>>,
        state: Arc<RwLock<ServerState>>,
    ) -> Self {
        let (request_tx, request_rx) = mpsc::channel(MAX_PENDING_REQUESTS);
        let debounce_delay = Duration::from_millis(DEFAULT_DEBOUNCE_MS);

        // Spawn the debouncer task
        tokio::spawn(debouncer_task(
            request_rx,
            client,
            current_versions,
            state,
            debounce_delay,
        ));

        Self {
            request_tx,
            debounce_delay,
        }
    }

    /// Queue a document for analysis.
    ///
    /// The analysis will be debounced - if multiple requests come in for the
    /// same document within the debounce window, only the latest will be processed.
    /// If the channel is full (backpressure), the request is dropped - this is
    /// acceptable since a newer edit will supersede it anyway.
    pub fn queue_analysis(&self, request: AnalysisRequest) {
        // Use try_send to avoid blocking; drop request if channel is full
        if let Err(e) = self.request_tx.try_send(request) {
            eprintln!(
                "Analysis queue full, dropping request: {}",
                match &e {
                    mpsc::error::TrySendError::Full(r) => &r.uri,
                    mpsc::error::TrySendError::Closed(r) => &r.uri,
                }
            );
        }
    }

    /// Get the debounce delay.
    pub fn debounce_delay(&self) -> Duration {
        self.debounce_delay
    }
}

/// Background task that debounces analysis requests.
async fn debouncer_task(
    mut request_rx: mpsc::Receiver<AnalysisRequest>,
    client: Client,
    current_versions: Arc<RwLock<HashMap<String, i32>>>,
    state: Arc<RwLock<ServerState>>,
    debounce_delay: Duration,
) {
    // Pending requests by URI (only keep the latest)
    let mut pending: HashMap<String, AnalysisRequest> = HashMap::new();

    // Timer for debounce window
    let mut debounce_timer = tokio::time::interval(debounce_delay);
    debounce_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            // New request received
            Some(request) = request_rx.recv() => {
                // Always keep the latest request for each URI
                pending.insert(request.uri.clone(), request);
            }

            // Debounce timer fired
            _ = debounce_timer.tick() => {
                if pending.is_empty() {
                    continue;
                }

                // Take all pending requests
                let requests: Vec<_> = pending.drain().collect();

                // Process each request
                for (uri, request) in requests {
                    // Check if this version is still current
                    let is_current = {
                        let versions = current_versions.read().await;
                        versions.get(&uri).copied() == Some(request.version)
                    };

                    if !is_current {
                        // Skip stale analysis
                        continue;
                    }

                    // Spawn blocking analysis task
                    let client = client.clone();
                    let current_versions = Arc::clone(&current_versions);
                    let state = Arc::clone(&state);

                    tokio::spawn(async move {
                        // Run analysis in blocking thread pool with timeout
                        let analysis_timeout = Duration::from_secs(ANALYSIS_TIMEOUT_SECS);
                        let uri_for_timeout = request.uri.clone();

                        let result = tokio::time::timeout(
                            analysis_timeout,
                            tokio::task::spawn_blocking(move || run_analysis(request)),
                        )
                        .await;

                        match result {
                            Ok(Ok(Some(analysis_result))) => {
                                // Check if still current before publishing
                                let is_current = {
                                    let versions = current_versions.read().await;
                                    versions.get(&analysis_result.uri).copied()
                                        == Some(analysis_result.version)
                                };

                                if is_current {
                                    // Cache raw diagnostics for code actions
                                    {
                                        let mut state = state.write().await;
                                        state.cache_diagnostics(
                                            analysis_result.uri.clone(),
                                            analysis_result.version,
                                            analysis_result.raw_diagnostics,
                                        );
                                    }

                                    // Parse URI and publish diagnostics
                                    if let Ok(url) = Url::parse(&analysis_result.uri) {
                                        client
                                            .publish_diagnostics(
                                                url,
                                                analysis_result.lsp_diagnostics,
                                                Some(analysis_result.version),
                                            )
                                            .await;
                                    }
                                }
                            }
                            Ok(Ok(None)) => {
                                // Analysis failed silently (parse error, etc.)
                            }
                            Ok(Err(e)) => {
                                // Task panicked
                                eprintln!("Analysis task panicked: {:?}", e);
                            }
                            Err(_) => {
                                // Timeout elapsed
                                eprintln!(
                                    "Analysis timed out after {}s for {}",
                                    ANALYSIS_TIMEOUT_SECS, uri_for_timeout
                                );
                            }
                        }
                    });
                }
            }
        }
    }
}

/// Run the full analysis pipeline (synchronous, runs in blocking pool).
///
/// Uses project-wide context for analysis to ensure LSP diagnostics match
/// CLI output for cross-file rules like unused function detection.
fn run_analysis(request: AnalysisRequest) -> Option<AnalysisResult> {
    // Use unified AnalysisPipeline for consistent diagnostics with CLI
    let ctx = &request.project_ctx;
    let pipeline = crate::analysis::AnalysisPipeline::with_cross_file_data(
        Arc::clone(&ctx.class_db),
        ctx.config.clone(),
        Arc::clone(&ctx.scenes),
        Arc::clone(&ctx.project_info),
        Arc::clone(&ctx.call_graph),
        Arc::clone(&ctx.reachable_functions),
        Arc::clone(&ctx.all_file_symbols),
    );

    let result = match pipeline.analyze_file(&request.path, &request.content) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Analysis failed for {}: {}", request.uri, e);
            return None;
        }
    };

    // Parse URI for related_information context
    let uri = Url::parse(&request.uri).ok();

    // Convert to LSP diagnostics with URI context for labels
    let lsp_diagnostics: Vec<Diagnostic> = result
        .diagnostics
        .iter()
        .map(|d| to_lsp_diagnostic_with_uri(d, uri.as_ref()))
        .collect();

    Some(AnalysisResult {
        uri: request.uri,
        version: request.version,
        lsp_diagnostics,
        raw_diagnostics: result.diagnostics,
    })
}
