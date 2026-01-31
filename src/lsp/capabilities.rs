//! Server capabilities configuration.

use tower_lsp::lsp_types::*;

/// Get the server capabilities.
pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        // Text document sync - we want full content on change for simplicity
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::FULL),
                will_save: None,
                will_save_wait_until: None,
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
            },
        )),

        // Hover support
        hover_provider: Some(HoverProviderCapability::Simple(true)),

        // Completion support
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string(), "\"".to_string(), "'".to_string()]),
            all_commit_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
            completion_item: None,
        }),

        // Go to definition
        definition_provider: Some(OneOf::Left(true)),

        // Find references
        references_provider: Some(OneOf::Left(true)),

        // Document symbols (outline)
        document_symbol_provider: Some(OneOf::Left(true)),

        // Workspace symbols (global search)
        workspace_symbol_provider: Some(OneOf::Left(true)),

        // Signature help
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
            retrigger_characters: None,
            work_done_progress_options: WorkDoneProgressOptions::default(),
        }),

        // Rename support
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: WorkDoneProgressOptions::default(),
        })),

        // Code actions (quick fixes)
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![
                CodeActionKind::QUICKFIX,
                CodeActionKind::REFACTOR,
                CodeActionKind::SOURCE,
            ]),
            work_done_progress_options: WorkDoneProgressOptions::default(),
            resolve_provider: Some(false),
        })),

        // Diagnostics are pushed via publishDiagnostics notification
        diagnostic_provider: None,

        // Document formatting
        document_formatting_provider: Some(OneOf::Left(true)),

        // Inlay hints (type annotations)
        inlay_hint_provider: Some(OneOf::Left(true)),

        // Semantic tokens for enhanced highlighting
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                work_done_progress_options: WorkDoneProgressOptions::default(),
                legend: SemanticTokensLegend {
                    token_types: vec![
                        SemanticTokenType::NAMESPACE,   // class_name
                        SemanticTokenType::CLASS,       // type names
                        SemanticTokenType::FUNCTION,    // function definitions
                        SemanticTokenType::METHOD,      // method calls
                        SemanticTokenType::PROPERTY,    // member variables
                        SemanticTokenType::VARIABLE,    // local variables
                        SemanticTokenType::PARAMETER,   // function parameters
                        SemanticTokenType::TYPE,        // type annotations
                        SemanticTokenType::DECORATOR,   // @onready, @export, etc.
                        SemanticTokenType::ENUM,        // enum names
                        SemanticTokenType::ENUM_MEMBER, // enum values
                    ],
                    token_modifiers: vec![
                        SemanticTokenModifier::DECLARATION,
                        SemanticTokenModifier::DEFINITION,
                        SemanticTokenModifier::READONLY,
                        SemanticTokenModifier::STATIC,
                    ],
                },
                range: Some(false),
                full: Some(SemanticTokensFullOptions::Bool(true)),
            },
        )),

        // Other capabilities we don't support yet
        declaration_provider: None,
        type_definition_provider: None,
        implementation_provider: None,
        document_highlight_provider: None,
        code_lens_provider: None,
        document_link_provider: None,
        color_provider: None,
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: None,
        folding_range_provider: None,
        execute_command_provider: None,
        selection_range_provider: None,
        linked_editing_range_provider: None,
        call_hierarchy_provider: Some(CallHierarchyServerCapability::Simple(true)),
        moniker_provider: None,
        inline_value_provider: None,
        position_encoding: None,
        workspace: Some(WorkspaceServerCapabilities {
            workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(OneOf::Left(true)),
            }),
            file_operations: None,
        }),
        experimental: None,
    }
}
