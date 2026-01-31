//! Semantic tokens and inlay hints handlers.

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::Backend;

/// Handle inlay hints request.
pub async fn inlay_hint(
    backend: &Backend,
    params: InlayHintParams,
) -> Result<Option<Vec<InlayHint>>> {
    let uri = params.text_document.uri.to_string();
    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let symbols = match state.get_symbols(&uri) {
        Some(s) => s,
        None => return Ok(None),
    };

    let mut hints = Vec::new();

    // Add hints for function parameters and return types
    for func in &symbols.functions {
        // Parameter type hints (when no explicit annotation but type is inferred)
        for param in &func.parameters {
            if param.type_annotation.is_none() {
                if let Some(ref inferred) = param.inferred_type {
                    // Use name_end_byte to get position after parameter name
                    let (line, col) = doc.position_at(param.name_end_byte);

                    hints.push(InlayHint {
                        position: Position {
                            line: line as u32,
                            character: col as u32,
                        },
                        label: InlayHintLabel::String(format!(": {}", inferred)),
                        kind: Some(InlayHintKind::TYPE),
                        text_edits: None,
                        tooltip: None,
                        padding_left: None,
                        padding_right: Some(true),
                        data: None,
                    });
                }
            }
        }

        // Return type hints (when no explicit annotation but type is inferred)
        if func.return_type.is_none() {
            if let Some(ref inferred) = func.inferred_return_type {
                // Use end of last parameter or name_end_byte + 2 for "()"
                let pos_byte = if func.parameters.is_empty() {
                    // func name() - position after name + 2 for "()"
                    func.name_start_byte + func.name.len() + 2
                } else {
                    // After last parameter's end_byte + 1 for ")"
                    func.parameters
                        .last()
                        .map(|p| p.end_byte + 1)
                        .unwrap_or(func.name_start_byte)
                };
                let (line, col) = doc.position_at(pos_byte);

                hints.push(InlayHint {
                    position: Position {
                        line: line as u32,
                        character: col as u32,
                    },
                    label: InlayHintLabel::String(format!(" -> {}", inferred)),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: Some(true),
                    padding_right: None,
                    data: None,
                });
            }
        }

        // Local variable type hints
        for local in &func.local_vars {
            if local.type_annotation.is_none() {
                let type_hint = local
                    .inferred_type
                    .as_ref()
                    .or(local.initializer_type.as_ref());

                if let Some(inferred) = type_hint {
                    // Estimate position: start_byte + "var ".len() + name.len()
                    // or use a more precise byte offset if available
                    let pos_byte = local.start_byte + "var ".len() + local.name.len();
                    let (line, col) = doc.position_at(pos_byte);

                    hints.push(InlayHint {
                        position: Position {
                            line: line as u32,
                            character: col as u32,
                        },
                        label: InlayHintLabel::String(format!(": {}", inferred)),
                        kind: Some(InlayHintKind::TYPE),
                        text_edits: None,
                        tooltip: None,
                        padding_left: None,
                        padding_right: Some(true),
                        data: None,
                    });
                }
            }
        }
    }

    // Add hints for member variables
    for var in &symbols.variables {
        if var.type_annotation.is_none() {
            let type_hint = var.inferred_type.as_ref().or(var.initializer_type.as_ref());

            if let Some(inferred) = type_hint {
                // Estimate position: start_byte + "var ".len() + name.len()
                let pos_byte = var.start_byte + "var ".len() + var.name.len();
                let (line, col) = doc.position_at(pos_byte);

                hints.push(InlayHint {
                    position: Position {
                        line: line as u32,
                        character: col as u32,
                    },
                    label: InlayHintLabel::String(format!(": {}", inferred)),
                    kind: Some(InlayHintKind::TYPE),
                    text_edits: None,
                    tooltip: None,
                    padding_left: None,
                    padding_right: Some(true),
                    data: None,
                });
            }
        }
    }

    // Filter hints to only those within the requested range
    let filtered_hints: Vec<InlayHint> = hints
        .into_iter()
        .filter(|hint| {
            hint.position.line >= params.range.start.line
                && hint.position.line <= params.range.end.line
        })
        .collect();

    if filtered_hints.is_empty() {
        Ok(None)
    } else {
        Ok(Some(filtered_hints))
    }
}

/// Semantic token types - must match the order in capabilities.rs
const TOKEN_TYPE_NAMESPACE: u32 = 0; // class_name
const TOKEN_TYPE_CLASS: u32 = 1; // type names
const TOKEN_TYPE_FUNCTION: u32 = 2; // function definitions
const TOKEN_TYPE_PROPERTY: u32 = 4; // member variables
const TOKEN_TYPE_VARIABLE: u32 = 5; // local variables
const TOKEN_TYPE_PARAMETER: u32 = 6; // function parameters
const TOKEN_TYPE_ENUM: u32 = 9; // enum names

/// Semantic token modifiers - must match the order in capabilities.rs
const TOKEN_MOD_DECLARATION: u32 = 1 << 0;
const TOKEN_MOD_DEFINITION: u32 = 1 << 1;
const TOKEN_MOD_READONLY: u32 = 1 << 2;
const TOKEN_MOD_STATIC: u32 = 1 << 3;

/// Handle semantic tokens full request.
pub async fn semantic_tokens_full(
    backend: &Backend,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokensResult>> {
    let uri = params.text_document.uri.to_string();
    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let symbols = match state.get_symbols(&uri) {
        Some(s) => s,
        None => return Ok(None),
    };

    // Collect all tokens
    let mut tokens: Vec<(u32, u32, u32, u32, u32)> = Vec::new();
    // (line, start_char, length, token_type, token_modifiers)

    // Add class_name if present (typically on an early line)
    if let Some(ref class_name) = symbols.class_name {
        // class_name declarations are usually at the top of the file
        // Estimate: line 0, column after "class_name "
        let col = "class_name ".len() as u32;
        tokens.push((
            0u32, // First line estimate
            col,
            class_name.len() as u32,
            TOKEN_TYPE_NAMESPACE,
            TOKEN_MOD_DECLARATION | TOKEN_MOD_DEFINITION,
        ));
    }

    // Add functions
    for func in &symbols.functions {
        // Function name definition
        let (line, col) = doc.position_at(func.name_start_byte);
        tokens.push((
            line as u32,
            col as u32,
            func.name.len() as u32,
            TOKEN_TYPE_FUNCTION,
            TOKEN_MOD_DECLARATION | TOKEN_MOD_DEFINITION,
        ));

        // Parameters
        for param in &func.parameters {
            let (line, col) = doc.position_at(param.name_start_byte);
            tokens.push((
                line as u32,
                col as u32,
                param.name.len() as u32,
                TOKEN_TYPE_PARAMETER,
                TOKEN_MOD_DECLARATION,
            ));
        }

        // Local variables
        for local in &func.local_vars {
            // Estimate: start_byte + "var ".len()
            let name_start = local.start_byte + "var ".len();
            let (line, col) = doc.position_at(name_start);
            tokens.push((
                line as u32,
                col as u32,
                local.name.len() as u32,
                TOKEN_TYPE_VARIABLE,
                TOKEN_MOD_DECLARATION,
            ));
        }
    }

    // Add member variables
    for var in &symbols.variables {
        // Estimate: start_byte + "var ".len() (could also be "const ")
        let name_start = var.start_byte + "var ".len();
        let (line, col) = doc.position_at(name_start);
        let modifiers = if var.is_export || var.is_onready {
            TOKEN_MOD_DECLARATION | TOKEN_MOD_READONLY
        } else {
            TOKEN_MOD_DECLARATION
        };
        tokens.push((
            line as u32,
            col as u32,
            var.name.len() as u32,
            TOKEN_TYPE_PROPERTY,
            modifiers,
        ));
    }

    // Add constants
    for constant in &symbols.constants {
        let (line, col) = doc.position_at(constant.start_byte + "const ".len());
        tokens.push((
            line as u32,
            col as u32,
            constant.name.len() as u32,
            TOKEN_TYPE_PROPERTY,
            TOKEN_MOD_DECLARATION | TOKEN_MOD_READONLY | TOKEN_MOD_STATIC,
        ));
    }

    // Add signals
    for signal in &symbols.signals {
        let (line, col) = doc.position_at(signal.start_byte + "signal ".len());
        tokens.push((
            line as u32,
            col as u32,
            signal.name.len() as u32,
            TOKEN_TYPE_PROPERTY,
            TOKEN_MOD_DECLARATION,
        ));
    }

    // Add enums
    for enum_decl in &symbols.enums {
        let (line, col) = doc.position_at(enum_decl.start_byte + "enum ".len());
        tokens.push((
            line as u32,
            col as u32,
            enum_decl.name.len() as u32,
            TOKEN_TYPE_ENUM,
            TOKEN_MOD_DECLARATION | TOKEN_MOD_DEFINITION,
        ));
    }

    // Add inner classes
    for class in &symbols.inner_classes {
        let (line, col) = doc.position_at(class.start_byte + "class ".len());
        tokens.push((
            line as u32,
            col as u32,
            class.name.len() as u32,
            TOKEN_TYPE_CLASS,
            TOKEN_MOD_DECLARATION | TOKEN_MOD_DEFINITION,
        ));
    }

    // Sort tokens by position (line, then character)
    tokens.sort_by(|a, b| {
        if a.0 != b.0 {
            a.0.cmp(&b.0)
        } else {
            a.1.cmp(&b.1)
        }
    });

    // Convert to delta encoding with SemanticToken structs
    let mut data: Vec<SemanticToken> = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_char = 0u32;

    for (line, start_char, length, token_type, modifiers) in tokens {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 {
            start_char - prev_char
        } else {
            start_char
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: modifiers,
        });

        prev_line = line;
        prev_char = start_char;
    }

    if data.is_empty() {
        Ok(None)
    } else {
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}
