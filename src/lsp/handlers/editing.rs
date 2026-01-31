//! Editing handlers: rename, code actions, formatting.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::Backend;

/// Handle prepare rename request.
pub async fn prepare_rename(
    backend: &Backend,
    params: TextDocumentPositionParams,
) -> Result<Option<PrepareRenameResponse>> {
    let uri = params.text_document.uri.to_string();
    let position = params.position;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    let symbol_result = index.symbol_at(offset);

    match symbol_result {
        Some(crate::symbol_index::SymbolAtResult::Definition(def)) => {
            if let Some((start, end)) = def.name_range {
                let (start_line, start_col) = doc.position_at(start);
                let (end_line, end_col) = doc.position_at(end);
                Ok(Some(PrepareRenameResponse::Range(Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                })))
            } else {
                Ok(None)
            }
        }
        Some(crate::symbol_index::SymbolAtResult::Reference(ref_)) => {
            let (start_line, start_col) = doc.position_at(ref_.range.0);
            let (end_line, end_col) = doc.position_at(ref_.range.1);
            Ok(Some(PrepareRenameResponse::Range(Range {
                start: Position {
                    line: start_line as u32,
                    character: start_col as u32,
                },
                end: Position {
                    line: end_line as u32,
                    character: end_col as u32,
                },
            })))
        }
        None => Ok(None),
    }
}

/// Handle rename request.
pub async fn rename(backend: &Backend, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;
    let new_name = params.new_name;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    let symbol_result = index.symbol_at(offset);

    let name = match symbol_result {
        Some(crate::symbol_index::SymbolAtResult::Definition(def)) => def.name.clone(),
        Some(crate::symbol_index::SymbolAtResult::Reference(ref_)) => ref_.name.clone(),
        None => return Ok(None),
    };

    let mut edits = Vec::new();

    // Rename definition
    for def in index.definitions_named(&name) {
        if let Some((start, end)) = def.name_range {
            let (start_line, start_col) = doc.position_at(start);
            let (end_line, end_col) = doc.position_at(end);
            edits.push(TextEdit {
                range: Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                },
                new_text: new_name.clone(),
            });
        }
    }

    // Rename all references
    for ref_ in index.references() {
        if ref_.name == name {
            let (start_line, start_col) = doc.position_at(ref_.range.0);
            let (end_line, end_col) = doc.position_at(ref_.range.1);
            edits.push(TextEdit {
                range: Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                },
                new_text: new_name.clone(),
            });
        }
    }

    if edits.is_empty() {
        return Ok(None);
    }

    let mut changes = std::collections::HashMap::new();
    changes.insert(params.text_document_position.text_document.uri, edits);

    Ok(Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    }))
}

/// Handle code action request.
pub async fn code_action(
    backend: &Backend,
    params: CodeActionParams,
) -> Result<Option<CodeActionResponse>> {
    let uri = params.text_document.uri.to_string();
    let state = backend.state.read().await;

    // Get the document
    let doc = match state.get_document(&uri) {
        Some(doc) => doc,
        None => return Ok(Some(Vec::new())),
    };

    // Try to use cached diagnostics from the background worker
    let all_symbols = state.all_symbols();
    let diagnostics =
        if let Some((cached_version, cached_diags)) = state.cached_diagnostics.get(&uri) {
            if *cached_version == doc.version() {
                // Cache is current, use it
                cached_diags.clone()
            } else {
                // Cache is stale, need to re-analyze (shouldn't happen often)
                let source = doc.content().to_string();
                let path = crate::lsp::state::uri_to_path(&uri);
                match crate::analysis::analyze_source_with_project_context(
                    &path,
                    &source,
                    &state.class_db,
                    &state.config,
                    &state.scenes,
                    &state.project_info,
                    &state.call_graph,
                    &state.reachable_functions,
                    &all_symbols,
                ) {
                    Ok(a) => a.diagnostics,
                    Err(_) => return Ok(Some(Vec::new())),
                }
            }
        } else {
            // No cache, need to analyze
            let source = doc.content().to_string();
            let path = crate::lsp::state::uri_to_path(&uri);
            match crate::analysis::analyze_source_with_project_context(
                &path,
                &source,
                &state.class_db,
                &state.config,
                &state.scenes,
                &state.project_info,
                &state.call_graph,
                &state.reachable_functions,
                &all_symbols,
            ) {
                Ok(a) => a.diagnostics,
                Err(_) => return Ok(Some(Vec::new())),
            }
        };

    // Build code actions from diagnostics with fixes
    let mut actions = Vec::new();

    for diag in diagnostics {
        // Check if diagnostic overlaps with requested range
        let diag_range = Range {
            start: Position {
                line: diag.line.saturating_sub(1) as u32,
                character: diag.col.saturating_sub(1) as u32,
            },
            end: Position {
                line: diag.end_line.saturating_sub(1) as u32,
                character: diag.end_col.saturating_sub(1) as u32,
            },
        };

        if !ranges_overlap(&diag_range, &params.range) {
            continue;
        }

        // Convert Fix to CodeAction
        if let Some(fix) = &diag.fix {
            let edits: Vec<TextEdit> = fix
                .edits
                .iter()
                .map(|e| {
                    let (start_line, start_col) = doc.position_at(e.start_byte);
                    let (end_line, end_col) = doc.position_at(e.end_byte);
                    TextEdit {
                        range: Range {
                            start: Position {
                                line: start_line as u32,
                                character: start_col as u32,
                            },
                            end: Position {
                                line: end_line as u32,
                                character: end_col as u32,
                            },
                        },
                        new_text: e.replacement.clone(),
                    }
                })
                .collect();

            let mut changes = std::collections::HashMap::new();
            changes.insert(params.text_document.uri.clone(), edits);

            let edit = WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            };

            let lsp_diag = crate::lsp::convert::to_lsp_diagnostic(&diag);

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: fix.description.clone(),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![lsp_diag]),
                edit: Some(edit),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: None,
            }));
        }
    }

    Ok(Some(actions))
}

/// Check if two ranges overlap.
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    // Ranges overlap if neither is completely before the other
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character < b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character < a.start.character))
}

/// Handle document formatting request.
pub async fn formatting(
    backend: &Backend,
    params: DocumentFormattingParams,
) -> Result<Option<Vec<TextEdit>>> {
    let uri = params.text_document.uri.to_string();
    let state = backend.state.read().await;

    // Get the document
    let doc = match state.get_document(&uri) {
        Some(doc) => doc,
        None => return Ok(None),
    };
    let source = doc.content().to_string();

    // Configure formatter from project config
    let config: crate::fmt::FmtConfig = (&state.config.formatter).into();

    // Format the source
    let result = match crate::fmt::format_source(&source, &config) {
        Ok(result) => result,
        Err(_) => return Ok(None), // Parse error, can't format
    };

    // If unchanged, return empty
    if result.unchanged {
        return Ok(None);
    }

    // Return single edit replacing entire document
    // Use Document's position_at for proper UTF-16 handling and trailing newline support
    let (end_line, end_col) = doc.position_at(source.len());

    Ok(Some(vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: end_line as u32,
                character: end_col as u32,
            },
        },
        new_text: result.output,
    }]))
}
