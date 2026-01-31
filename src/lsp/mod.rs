//! Language Server Protocol implementation for GDScript.
//!
//! This module provides an LSP server for GDScript using tower-lsp.
//! It supports features like diagnostics, hover, go-to-definition,
//! find-references, and document symbols.
//!
//! # Features
//!
//! - **Debounced analysis**: Edit notifications are debounced (300ms default)
//!   to avoid redundant analysis on rapid edits.
//! - **Background analysis**: CPU-intensive analysis runs in a separate thread
//!   pool, keeping the LSP event loop responsive.
//! - **Stale request cancellation**: Analysis results for outdated document
//!   versions are discarded.
//!
//! # Example
//!
//! ```ignore
//! use gdeye::lsp::run_server;
//!
//! #[tokio::main]
//! async fn main() {
//!     run_server().await;
//! }
//! ```

mod capabilities;
mod context;
mod convert;
pub mod cursor;
mod handlers;
mod state;
mod type_resolver;
pub mod uri;
mod worker;

pub use context::{RequestContext, RequestContextBuilder};
// Re-export from main crate for backwards compatibility
pub use crate::project_index::{IndexedFile, ProjectIndex};

pub use capabilities::server_capabilities;
pub use state::ServerState;
pub use worker::AnalysisWorker;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::classdb::ClassDb;

/// The GDScript language server backend.
pub struct Backend {
    /// LSP client for sending notifications.
    client: Client,
    /// Server state shared across requests.
    state: Arc<RwLock<ServerState>>,
    /// Background analysis worker with debouncing.
    worker: AnalysisWorker,
    /// Current document versions (for stale request detection).
    document_versions: Arc<RwLock<HashMap<String, i32>>>,
}

impl Backend {
    /// Create a new backend with the given client.
    pub fn new(client: Client) -> Self {
        Self::with_state(client, ServerState::new())
    }

    /// Create a new backend with the given client and initial state.
    pub fn with_state(client: Client, initial_state: ServerState) -> Self {
        let document_versions = Arc::new(RwLock::new(HashMap::new()));
        let state = Arc::new(RwLock::new(initial_state));
        let worker = AnalysisWorker::new(
            client.clone(),
            Arc::clone(&document_versions),
            Arc::clone(&state),
        );

        Self {
            client,
            state,
            worker,
            document_versions,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        handlers::initialize(self, params).await
    }

    async fn initialized(&self, _params: InitializedParams) {
        handlers::initialized(self).await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        handlers::did_open(self, params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        handlers::did_change(self, params).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        handlers::did_save(self, params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        handlers::did_close(self, params).await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        handlers::hover(self, params).await
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        handlers::goto_definition(self, params).await
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        handlers::references(self, params).await
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        handlers::document_symbol(self, params).await
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        handlers::completion(self, params).await
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        handlers::signature_help(self, params).await
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        handlers::workspace_symbol(self, params).await
    }

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        handlers::prepare_rename(self, params).await
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        handlers::rename(self, params).await
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        handlers::code_action(self, params).await
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        handlers::did_change_watched_files(self, params).await;
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        handlers::formatting(self, params).await
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        handlers::inlay_hint(self, params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        handlers::semantic_tokens_full(self, params).await
    }

    async fn prepare_call_hierarchy(
        &self,
        params: CallHierarchyPrepareParams,
    ) -> Result<Option<Vec<CallHierarchyItem>>> {
        handlers::prepare_call_hierarchy(self, params).await
    }

    async fn incoming_calls(
        &self,
        params: CallHierarchyIncomingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
        handlers::incoming_calls(self, params).await
    }

    async fn outgoing_calls(
        &self,
        params: CallHierarchyOutgoingCallsParams,
    ) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
        handlers::outgoing_calls(self, params).await
    }
}

/// Run the LSP server on stdin/stdout.
pub async fn run_server() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// Run the LSP server with a custom class database.
pub async fn run_server_with_classdb(class_db: ClassDb) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(|client| Backend::with_state(client, ServerState::with_classdb(class_db)));
    Server::new(stdin, stdout, socket).serve(service).await;
}
