//! LSP request and notification handlers.
//!
//! This module is split into submodules by feature area:
//! - `lifecycle`: Connection and document lifecycle
//! - `navigation`: Hover, goto definition, references, symbols
//! - `completion`: Completion and signature help
//! - `editing`: Code actions, formatting, rename
//! - `semantic`: Inlay hints and semantic tokens
//! - `call_hierarchy`: Call hierarchy features

mod call_hierarchy;
mod completion;
mod editing;
mod lifecycle;
mod navigation;
mod semantic;

// Re-export all handlers for backward compatibility
pub use call_hierarchy::{incoming_calls, outgoing_calls, prepare_call_hierarchy};
pub use completion::{completion, signature_help};
pub use editing::{code_action, formatting, prepare_rename, rename};
pub use lifecycle::{
    did_change, did_change_watched_files, did_close, did_open, did_save, initialize, initialized,
};
pub use navigation::{document_symbol, goto_definition, hover, references, workspace_symbol};
pub use semantic::{inlay_hint, semantic_tokens_full};
