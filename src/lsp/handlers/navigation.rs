//! Navigation handlers: hover, goto definition, references, symbols.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::document::Document;
use crate::lsp::convert::{
    format_classdb_method_hover, format_hover_content, format_member_hover, to_lsp_document_symbol,
};
use crate::lsp::Backend;
use crate::symbol_index::SymbolAtResult;
use crate::symbols::FileSymbols;

/// Handle hover request.
pub async fn hover(backend: &Backend, params: HoverParams) -> Result<Option<Hover>> {
    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .to_string();
    let position = params.text_document_position_params.position;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    let parsed = match state.get_parsed(&uri) {
        Some(p) => p,
        None => return Ok(None),
    };

    // Convert position to byte offset
    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    // Look up the symbol at this position
    let symbol_result = index.symbol_at(offset);

    match symbol_result {
        Some(SymbolAtResult::Definition(def)) => {
            let content = format_hover_content(def);
            Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: content,
                }),
                range: def.name_range.map(|(start, end)| {
                    let (start_line, start_col) = doc.position_at(start);
                    let (end_line, end_col) = doc.position_at(end);
                    Range {
                        start: Position {
                            line: start_line as u32,
                            character: start_col as u32,
                        },
                        end: Position {
                            line: end_line as u32,
                            character: end_col as u32,
                        },
                    }
                }),
            }))
        }
        Some(SymbolAtResult::Reference(ref_)) => {
            // Try to resolve the reference to its definition locally
            if let Some(def) = index.resolve_reference(ref_) {
                let content = format_hover_content(def);
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: content,
                    }),
                    range: Some({
                        let (start_line, start_col) = doc.position_at(ref_.range.0);
                        let (end_line, end_col) = doc.position_at(ref_.range.1);
                        Range {
                            start: Position {
                                line: start_line as u32,
                                character: start_col as u32,
                            },
                            end: Position {
                                line: end_line as u32,
                                character: end_col as u32,
                            },
                        }
                    }),
                }));
            }

            // Local resolution failed - try cross-file resolution

            // First check if this is a type annotation
            let node = find_node_at_offset(parsed.root_node(), offset);
            if let Some(node) = node {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "type" {
                        if let Some(hover) = try_type_hover(&state, &doc, &ref_.name, ref_.range) {
                            return Ok(Some(hover));
                        }
                    }
                }
            }

            // Then try as member access (obj.member)
            if let Some(hover) =
                try_cross_file_hover(&state, &uri, parsed, &doc, offset, &ref_.name, ref_.range)
            {
                return Ok(Some(hover));
            }

            Ok(None)
        }
        None => {
            // No symbol found at exact position - try cross-file resolution
            // by finding the identifier node at this position
            if let Some(hover) = try_cross_file_hover_at_offset(&state, &uri, parsed, &doc, offset)
            {
                return Ok(Some(hover));
            }
            Ok(None)
        }
    }
}

/// Try to resolve hover for a member access (obj.member) across files.
fn try_cross_file_hover(
    state: &crate::lsp::state::ServerState,
    uri: &str,
    parsed: &crate::parser::ParsedFile,
    doc: &crate::document::Document,
    offset: usize,
    member_name: &str,
    member_range: (usize, usize),
) -> Option<Hover> {
    // Find the node at offset and check if it's part of an attribute access
    let node = find_node_at_offset(parsed.root_node(), offset)?;

    // Walk up to find an attribute node
    let mut current = node;
    let mut attr_node = None;
    while let Some(parent) = current.parent() {
        if parent.kind() == "attribute" {
            attr_node = Some(parent);
            break;
        }
        current = parent;
    }

    let attr_node = attr_node?;

    // Get the receiver (first named child of attribute)
    let receiver_node = get_attribute_receiver(attr_node)?;
    let receiver_text = parsed.node_text(receiver_node);

    // Don't handle 'self' - that's a local reference
    if receiver_text == "self" {
        return None;
    }

    // Resolve the receiver's type
    let receiver_type = state.resolve_variable_type(uri, receiver_text)?;

    // Try to find the member in a user-defined class
    if let Some(member_info) = state.find_member_in_class(&receiver_type, member_name) {
        let content = format_member_hover(&member_info, &receiver_type);
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some({
                let (start_line, start_col) = doc.position_at(member_range.0);
                let (end_line, end_col) = doc.position_at(member_range.1);
                Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                }
            }),
        });
    }

    // Try ClassDB for engine types
    let class_db = state.get_classdb();

    // Check methods
    if let Some(method) = class_db.get_method(&receiver_type, member_name) {
        let content = format_classdb_method_hover(method, &receiver_type);
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some({
                let (start_line, start_col) = doc.position_at(member_range.0);
                let (end_line, end_col) = doc.position_at(member_range.1);
                Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                }
            }),
        });
    }

    // Check builtin methods (Array, Vector2, etc.)
    if let Some(method) = class_db.get_builtin_method(&receiver_type, member_name) {
        let content = format_classdb_method_hover(method, &receiver_type);
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some({
                let (start_line, start_col) = doc.position_at(member_range.0);
                let (end_line, end_col) = doc.position_at(member_range.1);
                Range {
                    start: Position {
                        line: start_line as u32,
                        character: start_col as u32,
                    },
                    end: Position {
                        line: end_line as u32,
                        character: end_col as u32,
                    },
                }
            }),
        });
    }

    None
}

/// Try cross-file hover when no symbol was found at the exact offset.
fn try_cross_file_hover_at_offset(
    state: &crate::lsp::state::ServerState,
    uri: &str,
    parsed: &crate::parser::ParsedFile,
    doc: &crate::document::Document,
    offset: usize,
) -> Option<Hover> {
    // Find the identifier node at offset
    let node = find_node_at_offset(parsed.root_node(), offset)?;
    if node.kind() != "identifier" && node.kind() != "name" {
        return None;
    }

    let text = parsed.node_text(node);
    let range = (node.start_byte(), node.end_byte());

    // Check if this is a type annotation (parent is "type" node)
    if let Some(parent) = node.parent() {
        if parent.kind() == "type" {
            // This is a type reference - look it up as a class/autoload
            return try_type_hover(state, doc, text, range);
        }
    }

    // Otherwise try as attribute access
    try_cross_file_hover(state, uri, parsed, doc, offset, text, range)
}

/// Try to provide hover for a type name (class_name or autoload).
fn try_type_hover(
    state: &crate::lsp::state::ServerState,
    doc: &crate::document::Document,
    type_name: &str,
    type_range: (usize, usize),
) -> Option<Hover> {
    // Check if it's a user-defined class
    if let Some(file) = state.project_index.get_by_class_name(type_name) {
        let extends = file
            .symbols
            .extends
            .as_ref()
            .map(|e| format!(" extends {}", e))
            .unwrap_or_default();
        let content = format!(
            "```gdscript\nclass_name {}{}\n```\n\n*User-defined class*",
            type_name, extends
        );
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some(make_range(doc, type_range)),
        });
    }

    // Check if it's an autoload
    if let Some(file) = state.project_index.get_by_autoload(type_name) {
        let extends = file
            .symbols
            .extends
            .as_ref()
            .map(|e| format!(" extends {}", e))
            .unwrap_or_default();
        let content = format!(
            "```gdscript\n{}{}\n```\n\n*Autoload singleton*\n\nPath: `{}`",
            type_name,
            extends,
            file.symbols.path.display()
        );
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some(make_range(doc, type_range)),
        });
    }

    // Check if it's an engine class
    let class_db = state.get_classdb();
    if let Some(class_info) = class_db.get_class(type_name) {
        let parent = if class_info.parent.is_empty() {
            String::new()
        } else {
            format!(" extends {}", class_info.parent)
        };
        let content = format!(
            "```gdscript\nclass {}{}\n```\n\n*Godot engine class*",
            type_name, parent
        );
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some(make_range(doc, type_range)),
        });
    }

    // Check if it's a builtin type (Array, Vector2, etc.)
    if class_db.get_builtin_class(type_name).is_some() {
        let content = format!("```gdscript\n{}\n```\n\n*Godot builtin type*", type_name);
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: content,
            }),
            range: Some(make_range(doc, type_range)),
        });
    }

    None
}

/// Helper to create an LSP Range from byte offsets.
fn make_range(doc: &crate::document::Document, byte_range: (usize, usize)) -> Range {
    let (start_line, start_col) = doc.position_at(byte_range.0);
    let (end_line, end_col) = doc.position_at(byte_range.1);
    Range {
        start: Position {
            line: start_line as u32,
            character: start_col as u32,
        },
        end: Position {
            line: end_line as u32,
            character: end_col as u32,
        },
    }
}

/// Find the deepest node containing the given byte offset.
fn find_node_at_offset(root: tree_sitter::Node, offset: usize) -> Option<tree_sitter::Node> {
    let mut cursor = root.walk();
    let mut result = None;

    loop {
        let node = cursor.node();
        if node.start_byte() <= offset && offset < node.end_byte() {
            result = Some(node);
            if !cursor.goto_first_child() {
                break;
            }
        } else if !cursor.goto_next_sibling() {
            break;
        }
    }

    result
}

/// Get the receiver node of an attribute access (the object before the dot).
fn get_attribute_receiver(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let count = node.child_count();
    for i in 0..count {
        if let Some(child) = node.child(i) {
            if child.is_named() {
                return Some(child);
            }
        }
    }
    None
}

/// Handle go to definition request.
pub async fn goto_definition(
    backend: &Backend,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .to_string();
    let position = params.text_document_position_params.position;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let parsed = match state.get_parsed(&uri) {
        Some(p) => p,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    // Convert position to byte offset
    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    // Use CursorContext to understand what's at the cursor
    let cursor = crate::lsp::cursor::CursorContext::at_offset(parsed, offset);

    if let Some(cursor) = cursor {
        use crate::lsp::cursor::CursorKind;

        match &cursor.kind {
            // Type annotation: navigate to the class definition
            CursorKind::TypeAnnotation { type_name } => {
                // Try class_name or autoload lookup
                if let Some(file) = state
                    .project_index
                    .get_by_class_name(type_name)
                    .or_else(|| state.project_index.get_by_autoload(type_name))
                {
                    return Ok(Some(make_file_location(&file.symbols.path)));
                }
            }

            // Reference: could be local, class_name, or autoload
            CursorKind::Reference { name } => {
                // First try local resolution
                if let Some(def_id) = index.resolve_name(name, offset) {
                    if let Some(def) = index.get_definition(def_id) {
                        if let Some((start, end)) = def.name_range.or(def.range) {
                            return Ok(Some(make_location_in_doc(
                                &params.text_document_position_params.text_document.uri,
                                &doc,
                                start,
                                end,
                            )));
                        }
                    }
                }

                // Try cross-file: class_name or autoload
                if let Some(file) = state
                    .project_index
                    .get_by_class_name(name)
                    .or_else(|| state.project_index.get_by_autoload(name))
                {
                    return Ok(Some(make_file_location(&file.symbols.path)));
                }
            }

            // Member access: resolve receiver type, then find member
            CursorKind::MemberAccess {
                receiver_text,
                member,
                ..
            }
            | CursorKind::MethodCall {
                receiver_text: Some(receiver_text),
                method: member,
                ..
            } => {
                // Resolve the receiver's type
                if let Some(receiver_type) = state.resolve_variable_type(&uri, receiver_text) {
                    // Find the target file
                    if let Some(file) = state
                        .project_index
                        .get_by_class_name(&receiver_type)
                        .or_else(|| state.project_index.get_by_autoload(&receiver_type))
                    {
                        // Find the member in the target
                        if let Some(location) = find_member_location(&file.symbols, member) {
                            return Ok(Some(location));
                        }
                    }
                }

                // Also check if receiver is directly an autoload/class_name
                if let Some(file) = state
                    .project_index
                    .get_by_class_name(receiver_text)
                    .or_else(|| state.project_index.get_by_autoload(receiver_text))
                {
                    if let Some(location) = find_member_location(&file.symbols, member) {
                        return Ok(Some(location));
                    }
                }
            }

            _ => {}
        }
    }

    // Fallback: use existing symbol_at logic
    let symbol_result = index.symbol_at(offset);

    match symbol_result {
        Some(SymbolAtResult::Definition(def)) => {
            if let Some((start, end)) = def.name_range.or(def.range) {
                Ok(Some(make_location_in_doc(
                    &params.text_document_position_params.text_document.uri,
                    &doc,
                    start,
                    end,
                )))
            } else {
                Ok(None)
            }
        }
        Some(SymbolAtResult::Reference(ref_)) => {
            if let Some(def) = index.resolve_reference(ref_) {
                if let Some((start, end)) = def.name_range.or(def.range) {
                    Ok(Some(make_location_in_doc(
                        &params.text_document_position_params.text_document.uri,
                        &doc,
                        start,
                        end,
                    )))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

/// Create a location at the start of a file.
fn make_file_location(path: &std::path::Path) -> GotoDefinitionResponse {
    let uri = crate::lsp::uri::path_to_uri(path);
    GotoDefinitionResponse::Scalar(Location {
        uri: Url::parse(&uri).unwrap_or_else(|_| Url::parse("file:///").unwrap()),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
    })
}

/// Create a location in a document using byte offsets.
fn make_location_in_doc(
    uri: &Url,
    doc: &Document,
    start: usize,
    end: usize,
) -> GotoDefinitionResponse {
    let (start_line, start_col) = doc.position_at(start);
    let (end_line, end_col) = doc.position_at(end);
    GotoDefinitionResponse::Scalar(Location {
        uri: uri.clone(),
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
    })
}

/// Find the location of a member (function, variable, signal) in file symbols.
fn find_member_location(
    symbols: &FileSymbols,
    member_name: &str,
) -> Option<GotoDefinitionResponse> {
    let uri = crate::lsp::uri::path_to_uri(&symbols.path);
    let uri = Url::parse(&uri).ok()?;

    // Check functions
    for func in &symbols.functions {
        if func.name == member_name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri,
                range: Range {
                    start: Position {
                        line: func.line.saturating_sub(1) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: func.line.saturating_sub(1) as u32,
                        character: func.name.len() as u32,
                    },
                },
            }));
        }
    }

    // Check variables
    for var in &symbols.variables {
        if var.name == member_name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri,
                range: Range {
                    start: Position {
                        line: var.line.saturating_sub(1) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: var.line.saturating_sub(1) as u32,
                        character: var.name.len() as u32,
                    },
                },
            }));
        }
    }

    // Check signals
    for signal in &symbols.signals {
        if signal.name == member_name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri,
                range: Range {
                    start: Position {
                        line: signal.line.saturating_sub(1) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: signal.line.saturating_sub(1) as u32,
                        character: signal.name.len() as u32,
                    },
                },
            }));
        }
    }

    // Check constants
    for constant in &symbols.constants {
        if constant.name == member_name {
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri,
                range: Range {
                    start: Position {
                        line: constant.line.saturating_sub(1) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: constant.line.saturating_sub(1) as u32,
                        character: constant.name.len() as u32,
                    },
                },
            }));
        }
    }

    None
}

/// Handle find references request.
pub async fn references(
    backend: &Backend,
    params: ReferenceParams,
) -> Result<Option<Vec<Location>>> {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    // Convert position to byte offset
    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    // Find the symbol at this position
    let symbol_result = index.symbol_at(offset);

    let name = match symbol_result {
        Some(SymbolAtResult::Definition(def)) => def.name.clone(),
        Some(SymbolAtResult::Reference(ref_)) => ref_.name.clone(),
        None => return Ok(None),
    };

    // Find all references to this name
    let mut locations = Vec::new();

    // Include the definition if requested - first in current file
    if params.context.include_declaration {
        for def in index.definitions_named(&name) {
            if let Some((start, end)) = def.name_range.or(def.range) {
                let (start_line, start_col) = doc.position_at(start);
                let (end_line, end_col) = doc.position_at(end);
                locations.push(Location {
                    uri: params.text_document_position.text_document.uri.clone(),
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
                });
            }
        }
    }

    // Add all references in current file
    for ref_ in index.references() {
        if ref_.name == name {
            let (start_line, start_col) = doc.position_at(ref_.range.0);
            let (end_line, end_col) = doc.position_at(ref_.range.1);
            locations.push(Location {
                uri: params.text_document_position.text_document.uri.clone(),
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
            });
        }
    }

    // Cross-file search: search all project files using the unified ProjectIndex
    for (file_path, indexed_file) in state.project_index.iter() {
        let file_uri = crate::lsp::uri::path_to_uri(file_path);

        // Skip the current file (already searched above)
        if file_uri == uri {
            continue;
        }

        // If this file has an index, use it for precise references
        if let Some(other_index) = &indexed_file.index {
            // Get Document for position mapping if content is available
            if let Some(content) = &indexed_file.content {
                let other_doc = Document::with_version(
                    file_path,
                    indexed_file.version.unwrap_or(0),
                    content.clone(),
                );
                for ref_ in other_index.references() {
                    if ref_.name == name {
                        let (start_line, start_col) = other_doc.position_at(ref_.range.0);
                        let (end_line, end_col) = other_doc.position_at(ref_.range.1);
                        if let Ok(parsed_uri) = Url::parse(&file_uri) {
                            locations.push(Location {
                                uri: parsed_uri,
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
                            });
                        }
                    }
                }
                continue;
            }
        }

        // Fallback: search the symbols directly (for non-open files without index)
        let references = find_references_in_symbols(&indexed_file.symbols, &name);
        for (line, col, end_col) in references {
            if let Ok(parsed_uri) = Url::parse(&file_uri) {
                locations.push(Location {
                    uri: parsed_uri,
                    range: Range {
                        start: Position {
                            line: line.saturating_sub(1) as u32,
                            character: col as u32,
                        },
                        end: Position {
                            line: line.saturating_sub(1) as u32,
                            character: end_col as u32,
                        },
                    },
                });
            }
        }
    }

    if locations.is_empty() {
        Ok(None)
    } else {
        Ok(Some(locations))
    }
}

/// Find references to a name in file symbols.
/// Returns Vec of (line, start_col, end_col) for each reference found.
/// Note: This is a simple search based on symbol metadata. It won't find all
/// usages in code - for complete results, files would need to be parsed.
fn find_references_in_symbols(symbols: &FileSymbols, name: &str) -> Vec<(usize, usize, usize)> {
    let mut refs = Vec::new();

    // Check variables that have this type or initializer call
    for var in &symbols.variables {
        if var.type_annotation.as_deref() == Some(name) {
            refs.push((var.line, 0, name.len()));
        }
        if var.initializer_call.as_deref() == Some(name) {
            refs.push((var.line, 0, name.len()));
        }
        if var.inferred_type.as_deref() == Some(name) {
            refs.push((var.line, 0, name.len()));
        }
    }

    // Check function return types and parameters
    for func in &symbols.functions {
        if func.return_type.as_deref() == Some(name) {
            refs.push((func.line, 0, name.len()));
        }
        for param in &func.parameters {
            if param.type_annotation.as_deref() == Some(name) {
                refs.push((param.line, 0, name.len()));
            }
        }
        // Check local variables
        for local in &func.local_vars {
            if local.type_annotation.as_deref() == Some(name) {
                refs.push((local.line, 0, name.len()));
            }
            if local.initializer_call.as_deref() == Some(name) {
                refs.push((local.line, 0, name.len()));
            }
            if local.inferred_type.as_deref() == Some(name) {
                refs.push((local.line, 0, name.len()));
            }
        }
    }

    // Check if extends this name
    if symbols.extends.as_deref() == Some(name) {
        refs.push((1, 0, name.len())); // extends is usually on first lines
    }

    refs
}

/// Handle document symbol request.
pub async fn document_symbol(
    backend: &Backend,
    params: DocumentSymbolParams,
) -> Result<Option<DocumentSymbolResponse>> {
    let uri = params.text_document.uri.to_string();

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    let symbols: Vec<DocumentSymbol> = index
        .definitions()
        .iter()
        .filter_map(|def| to_lsp_document_symbol(def, &doc))
        .collect();

    if symbols.is_empty() {
        Ok(None)
    } else {
        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }
}

/// Handle workspace symbol request.
pub async fn workspace_symbol(
    backend: &Backend,
    params: WorkspaceSymbolParams,
) -> Result<Option<Vec<SymbolInformation>>> {
    let query = params.query.to_lowercase();
    let state = backend.state.read().await;

    let mut results = Vec::new();

    // Search through all project files (not just open documents)
    for symbols in state.project_symbols() {
        let file_uri = match Url::from_file_path(&symbols.path) {
            Ok(u) => u,
            Err(_) => continue,
        };

        // Search functions
        for func in &symbols.functions {
            if func.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: func.name.clone(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: func.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: func.line.saturating_sub(1) as u32,
                                character: func.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Search variables
        for var in &symbols.variables {
            if var.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: var.name.clone(),
                    kind: SymbolKind::VARIABLE,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: var.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: var.line.saturating_sub(1) as u32,
                                character: var.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Search signals
        for signal in &symbols.signals {
            if signal.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: signal.name.clone(),
                    kind: SymbolKind::EVENT,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: signal.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: signal.line.saturating_sub(1) as u32,
                                character: signal.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Search constants
        for constant in &symbols.constants {
            if constant.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: constant.name.clone(),
                    kind: SymbolKind::CONSTANT,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: constant.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: constant.line.saturating_sub(1) as u32,
                                character: constant.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Search enums
        for enum_decl in &symbols.enums {
            if enum_decl.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: enum_decl.name.clone(),
                    kind: SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: enum_decl.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: enum_decl.line.saturating_sub(1) as u32,
                                character: enum_decl.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Search inner classes
        for class in &symbols.inner_classes {
            if class.name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: class.name.clone(),
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: class.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: class.line.saturating_sub(1) as u32,
                                character: class.name.len() as u32,
                            },
                        },
                    },
                    container_name: symbols.class_name.clone(),
                });
            }
        }

        // Also search class_name if it exists
        if let Some(ref class_name) = symbols.class_name {
            if class_name.to_lowercase().contains(&query) {
                #[allow(deprecated)]
                results.push(SymbolInformation {
                    name: class_name.clone(),
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: file_uri.clone(),
                        range: Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: class_name.len() as u32,
                            },
                        },
                    },
                    container_name: None,
                });
            }
        }
    }

    // Also search autoload names
    for (autoload_name, path) in state.project_index.autoloads() {
        if autoload_name.to_lowercase().contains(&query) {
            let file_uri = match Url::from_file_path(path) {
                Ok(u) => u,
                Err(_) => continue,
            };
            #[allow(deprecated)]
            results.push(SymbolInformation {
                name: format!("{} (autoload)", autoload_name),
                kind: SymbolKind::MODULE,
                tags: None,
                deprecated: None,
                location: Location {
                    uri: file_uri,
                    range: Range {
                        start: Position {
                            line: 0,
                            character: 0,
                        },
                        end: Position {
                            line: 0,
                            character: autoload_name.len() as u32,
                        },
                    },
                },
                container_name: None,
            });
        }
    }

    if results.is_empty() {
        Ok(None)
    } else {
        Ok(Some(results))
    }
}
