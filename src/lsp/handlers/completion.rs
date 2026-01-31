//! Completion and signature help handlers.

use std::path::PathBuf;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::type_resolver::TypeResolver;
use crate::lsp::uri::uri_to_path;
use crate::lsp::Backend;
use crate::symbols::FileSymbols;
use crate::types::types_compatible;

/// Handle completion request.
pub async fn completion(
    backend: &Backend,
    params: CompletionParams,
) -> Result<Option<CompletionResponse>> {
    let uri = params.text_document_position.text_document.uri.to_string();
    let position = params.text_document_position.position;

    let state = backend.state.read().await;

    let doc = match state.get_document(&uri) {
        Some(d) => d,
        None => return Ok(None),
    };

    let symbols = match state.get_symbols(&uri) {
        Some(s) => s,
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

    let mut items = Vec::new();

    // Use CursorContext to understand what kind of completion we need
    let cursor = crate::lsp::cursor::CursorContext::at_offset(parsed, offset);

    // Check if this is member access completion (after a dot)
    let is_member_access = cursor
        .as_ref()
        .map(|c| matches!(c.kind, crate::lsp::cursor::CursorKind::MemberAccess { .. }))
        .unwrap_or(false);

    if is_member_access {
        // Member access completion - get members from the receiver type
        if let Some(ref cursor) = cursor {
            if let crate::lsp::cursor::CursorKind::MemberAccess { receiver_text, .. } = &cursor.kind
            {
                // Create a type resolver with position awareness
                let file_path = uri_to_path(&uri).unwrap_or_else(|_| PathBuf::from(&uri));
                let class_db = state.get_classdb();
                let index = state.get_index(&uri);
                let type_resolver =
                    TypeResolver::new(&state.project_index, class_db, &file_path, symbols, index);

                // Resolve receiver type with position awareness
                let receiver_type = type_resolver
                    .resolve_variable_type(receiver_text, offset)
                    .or_else(|| {
                        // Fallback: check if it's a class name or autoload directly
                        if state
                            .project_index
                            .get_by_class_name(receiver_text)
                            .is_some()
                            || state.project_index.get_by_autoload(receiver_text).is_some()
                        {
                            Some(receiver_text.to_string())
                        } else {
                            None
                        }
                    });

                if let Some(receiver_type) = receiver_type {
                    // Try user-defined class
                    if let Some(file) = state
                        .project_index
                        .get_by_class_name(&receiver_type)
                        .or_else(|| state.project_index.get_by_autoload(&receiver_type))
                    {
                        add_symbols_to_completion(&mut items, &file.symbols);
                    }

                    // Also check if receiver_text is directly a class_name/autoload
                    if let Some(file) = state
                        .project_index
                        .get_by_class_name(receiver_text)
                        .or_else(|| state.project_index.get_by_autoload(receiver_text))
                    {
                        add_symbols_to_completion(&mut items, &file.symbols);
                    }

                    // Add class_db members for builtin types
                    let class_db = state.get_classdb();
                    if let Some(class_info) = class_db.get_class(&receiver_type) {
                        // Add methods
                        for method in &class_info.methods {
                            let return_str = if method.return_type.is_empty() {
                                "void".to_string()
                            } else {
                                method.return_type.clone()
                            };
                            items.push(CompletionItem {
                                label: method.name.clone(),
                                kind: Some(CompletionItemKind::METHOD),
                                detail: Some(return_str),
                                ..Default::default()
                            });
                        }
                        // Add properties
                        for prop in &class_info.properties {
                            items.push(CompletionItem {
                                label: prop.name.clone(),
                                kind: Some(CompletionItemKind::PROPERTY),
                                detail: Some(prop.prop_type.clone()),
                                ..Default::default()
                            });
                        }
                        // Add signals
                        for signal in &class_info.signals {
                            items.push(CompletionItem {
                                label: signal.name.clone(),
                                kind: Some(CompletionItemKind::EVENT),
                                ..Default::default()
                            });
                        }
                    }
                    // Also check builtin types (Vector2, Array, etc.)
                    if let Some(builtin_info) = class_db.get_builtin_class(&receiver_type) {
                        for method in &builtin_info.methods {
                            let return_str = if method.return_type.is_empty() {
                                "void".to_string()
                            } else {
                                method.return_type.clone()
                            };
                            items.push(CompletionItem {
                                label: method.name.clone(),
                                kind: Some(CompletionItemKind::METHOD),
                                detail: Some(return_str),
                                ..Default::default()
                            });
                        }
                    }
                }
            }
        }
    } else {
        // General completion - add local symbols + cross-file symbols

        // Get expected type for context-aware completion
        let expected_type = cursor.as_ref().and_then(|c| c.expected_type.clone());

        // For bool return type, add true/false as top completions
        if expected_type.as_deref() == Some("bool") {
            items.push(CompletionItem {
                label: "true".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                sort_text: Some("0000_true".to_string()),
                ..Default::default()
            });
            items.push(CompletionItem {
                label: "false".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                sort_text: Some("0000_false".to_string()),
                ..Default::default()
            });
        }

        // Add current file symbols (with type-aware sorting)
        let class_db = state.get_classdb();
        add_symbols_to_completion_ranked(&mut items, symbols, expected_type.as_deref(), class_db);

        // Add all class_names from the project
        for (class_name, path) in state.project_index.class_names() {
            let detail = state
                .project_index
                .get(path)
                .and_then(|f| f.symbols.extends.clone())
                .map(|e| format!("extends {}", e));
            items.push(CompletionItem {
                label: class_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail,
                documentation: Some(Documentation::String("User-defined class".to_string())),
                ..Default::default()
            });
        }

        // Add all autoloads
        for (autoload_name, path) in state.project_index.autoloads() {
            let detail = state
                .project_index
                .get(path)
                .and_then(|f| f.symbols.extends.clone())
                .map(|e| format!("extends {}", e));
            items.push(CompletionItem {
                label: autoload_name.to_string(),
                kind: Some(CompletionItemKind::MODULE),
                detail,
                documentation: Some(Documentation::String("Autoload singleton".to_string())),
                ..Default::default()
            });
        }

        // Add all Godot engine classes from class_db
        for class_name in class_db.class_names() {
            items.push(CompletionItem {
                label: class_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                documentation: Some(Documentation::String("Godot engine class".to_string())),
                ..Default::default()
            });
        }

        // Add builtin types (Vector2, Array, Dictionary, etc.)
        for builtin_name in class_db.builtin_class_names() {
            items.push(CompletionItem {
                label: builtin_name.to_string(),
                kind: Some(CompletionItemKind::STRUCT),
                documentation: Some(Documentation::String("Godot builtin type".to_string())),
                ..Default::default()
            });
        }
    }

    if items.is_empty() {
        Ok(None)
    } else {
        Ok(Some(CompletionResponse::Array(items)))
    }
}

/// Add symbols from a FileSymbols to the completion list.
fn add_symbols_to_completion(items: &mut Vec<CompletionItem>, symbols: &FileSymbols) {
    // Add functions
    for func in &symbols.functions {
        items.push(CompletionItem {
            label: func.name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: func
                .return_type
                .clone()
                .or(func.inferred_return_type.clone()),
            documentation: func.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });
    }

    // Add variables
    for var in &symbols.variables {
        items.push(CompletionItem {
            label: var.name.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: var.type_annotation.clone().or(var.inferred_type.clone()),
            documentation: var.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });
    }

    // Add signals
    for signal in &symbols.signals {
        items.push(CompletionItem {
            label: signal.name.clone(),
            kind: Some(CompletionItemKind::EVENT),
            documentation: signal.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });
    }

    // Add constants
    for constant in &symbols.constants {
        items.push(CompletionItem {
            label: constant.name.clone(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: constant.type_annotation.clone(),
            documentation: constant.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });
    }

    // Add enums
    for enum_decl in &symbols.enums {
        items.push(CompletionItem {
            label: enum_decl.name.clone(),
            kind: Some(CompletionItemKind::ENUM),
            documentation: enum_decl.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });

        // Add enum values
        for value in &enum_decl.values {
            items.push(CompletionItem {
                label: value.clone(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(enum_decl.name.clone()),
                ..Default::default()
            });
        }
    }

    // Add inner classes
    for class in &symbols.inner_classes {
        items.push(CompletionItem {
            label: class.name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: class.extends.clone(),
            documentation: class.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            ..Default::default()
        });
    }
}

/// Add symbols with type-aware ranking.
/// Items with types compatible with expected_type get higher priority (lower sort_text).
fn add_symbols_to_completion_ranked(
    items: &mut Vec<CompletionItem>,
    symbols: &FileSymbols,
    expected_type: Option<&str>,
    class_db: &crate::classdb::ClassDb,
) {
    // Helper to compute sort prefix based on type compatibility
    let sort_prefix = |item_type: Option<&str>| -> &'static str {
        match (expected_type, item_type) {
            (Some(expected), Some(actual)) if types_compatible(expected, actual, class_db) => {
                "0001_" // Compatible type - high priority
            }
            (Some(_), None) => "0003_", // Unknown type - lower priority
            (Some(_), Some(_)) => "0002_", // Incompatible type - medium priority
            (None, _) => "",            // No expected type - no sorting
        }
    };

    // Add functions
    for func in &symbols.functions {
        let func_type = func
            .return_type
            .as_deref()
            .or(func.inferred_return_type.as_deref());
        let prefix = sort_prefix(func_type);
        items.push(CompletionItem {
            label: func.name.clone(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: func
                .return_type
                .clone()
                .or(func.inferred_return_type.clone()),
            documentation: func.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: if prefix.is_empty() {
                None
            } else {
                Some(format!("{}{}", prefix, func.name))
            },
            ..Default::default()
        });
    }

    // Add variables
    for var in &symbols.variables {
        let var_type = var
            .type_annotation
            .as_deref()
            .or(var.inferred_type.as_deref());
        let prefix = sort_prefix(var_type);
        items.push(CompletionItem {
            label: var.name.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: var.type_annotation.clone().or(var.inferred_type.clone()),
            documentation: var.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: if prefix.is_empty() {
                None
            } else {
                Some(format!("{}{}", prefix, var.name))
            },
            ..Default::default()
        });
    }

    // Add signals (Signal type)
    for signal in &symbols.signals {
        let prefix = sort_prefix(Some("Signal"));
        items.push(CompletionItem {
            label: signal.name.clone(),
            kind: Some(CompletionItemKind::EVENT),
            documentation: signal.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: if prefix.is_empty() {
                None
            } else {
                Some(format!("{}{}", prefix, signal.name))
            },
            ..Default::default()
        });
    }

    // Add constants
    for constant in &symbols.constants {
        let const_type = constant.type_annotation.as_deref();
        let prefix = sort_prefix(const_type);
        items.push(CompletionItem {
            label: constant.name.clone(),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: constant.type_annotation.clone(),
            documentation: constant.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: if prefix.is_empty() {
                None
            } else {
                Some(format!("{}{}", prefix, constant.name))
            },
            ..Default::default()
        });
    }

    // Add enums (no type ranking for enum types)
    for enum_decl in &symbols.enums {
        items.push(CompletionItem {
            label: enum_decl.name.clone(),
            kind: Some(CompletionItemKind::ENUM),
            documentation: enum_decl.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: Some(format!("0002_{}", enum_decl.name)),
            ..Default::default()
        });

        // Add enum values
        for value in &enum_decl.values {
            items.push(CompletionItem {
                label: value.clone(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(enum_decl.name.clone()),
                sort_text: Some(format!("0002_{}", value)),
                ..Default::default()
            });
        }
    }

    // Add inner classes
    for class in &symbols.inner_classes {
        items.push(CompletionItem {
            label: class.name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: class.extends.clone(),
            documentation: class.documentation.as_ref().map(|doc| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.clone(),
                })
            }),
            sort_text: Some(format!("0002_{}", class.name)),
            ..Default::default()
        });
    }
}

/// Handle signature help request.
pub async fn signature_help(
    backend: &Backend,
    params: SignatureHelpParams,
) -> Result<Option<SignatureHelp>> {
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

    let symbols = match state.get_symbols(&uri) {
        Some(s) => s,
        None => return Ok(None),
    };

    // Get the line content up to cursor to find function name
    let line_start = doc.offset_at(position.line as usize, 0).unwrap_or(0);
    let cursor = doc
        .offset_at(position.line as usize, position.character as usize)
        .unwrap_or(0);
    let content = doc.content();
    let line_content = &content[line_start..cursor];

    // Count commas to determine active parameter (done early as it's shared)
    let after_paren = line_content
        .rfind('(')
        .map(|i| &line_content[i..])
        .unwrap_or("");
    let active_param = after_paren.chars().filter(|c| *c == ',').count() as u32;

    // Find the function call - look for pattern like `receiver.method(` or `func(`
    let paren_pos = match line_content.rfind('(') {
        Some(p) => p,
        None => return Ok(None),
    };
    let before_paren = &line_content[..paren_pos];

    // Check for method call pattern (receiver.method)
    if let Some(dot_pos) = before_paren.rfind('.') {
        let method_name: String = before_paren[dot_pos + 1..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        if !method_name.is_empty() {
            // Extract receiver text (go back from dot)
            let receiver_end = dot_pos;
            let receiver_text: String = before_paren[..receiver_end]
                .chars()
                .rev()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>()
                .chars()
                .rev()
                .collect();

            if !receiver_text.is_empty() {
                // Resolve receiver type and find method
                if let Some(sig) = resolve_method_signature(
                    &state,
                    &uri,
                    &receiver_text,
                    &method_name,
                    active_param,
                ) {
                    return Ok(Some(sig));
                }
            }
        }
    }

    // No dot, try as local function call
    let func_name: String = before_paren
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    if func_name.is_empty() {
        return Ok(None);
    }

    // Find the function in current file symbols
    if let Some(func) = symbols.functions.iter().find(|f| f.name == func_name) {
        return Ok(Some(build_signature_from_func(func, active_param)));
    }

    // Try class_db utility functions (like print, str, etc.)
    let class_db = state.get_classdb();
    if let Some(util_func) = class_db.get_utility_function(&func_name) {
        return Ok(Some(build_signature_from_utility(util_func, active_param)));
    }

    // Try as a constructor (ClassName.new() or just ClassName())
    // Check if it's a user-defined class
    if let Some(file) = state.project_index.get_by_class_name(&func_name) {
        // Look for _init function
        if let Some(init_func) = file.symbols.functions.iter().find(|f| f.name == "_init") {
            return Ok(Some(build_constructor_signature(
                &func_name,
                init_func,
                active_param,
            )));
        }
    }

    Ok(None)
}

/// Resolve a method call signature from receiver type.
fn resolve_method_signature(
    state: &crate::lsp::state::ServerState,
    uri: &str,
    receiver_text: &str,
    method_name: &str,
    active_param: u32,
) -> Option<SignatureHelp> {
    // First resolve the receiver's type
    let receiver_type = state
        .resolve_variable_type(uri, receiver_text)
        .or_else(|| {
            // Also try as direct class/autoload reference
            if state
                .project_index
                .get_by_class_name(receiver_text)
                .is_some()
                || state.project_index.get_by_autoload(receiver_text).is_some()
            {
                return Some(receiver_text.to_string());
            }
            None
        })?;

    // Try user-defined class
    if let Some(file) = state
        .project_index
        .get_by_class_name(&receiver_type)
        .or_else(|| state.project_index.get_by_autoload(&receiver_type))
    {
        if let Some(func) = file
            .symbols
            .functions
            .iter()
            .find(|f| f.name == method_name)
        {
            return Some(build_signature_from_func(func, active_param));
        }
    }

    // Try class_db
    let class_db = state.get_classdb();

    // Regular class method
    if let Some(method) = class_db.get_method(&receiver_type, method_name) {
        return Some(build_signature_from_classdb_method(
            method,
            &receiver_type,
            active_param,
        ));
    }

    // Builtin type method
    if let Some(method) = class_db.get_builtin_method(&receiver_type, method_name) {
        return Some(build_signature_from_classdb_method(
            method,
            &receiver_type,
            active_param,
        ));
    }

    None
}

/// Build SignatureHelp from a user-defined function.
fn build_signature_from_func(func: &crate::symbols::FuncDecl, active_param: u32) -> SignatureHelp {
    let params_str: Vec<String> = func
        .parameters
        .iter()
        .map(|p| {
            if let Some(ref ty) = p.type_annotation {
                format!("{}: {}", p.name, ty)
            } else {
                p.name.clone()
            }
        })
        .collect();

    let return_str = func
        .return_type
        .as_ref()
        .or(func.inferred_return_type.as_ref())
        .map(|t| format!(" -> {}", t))
        .unwrap_or_default();

    let label = format!(
        "func {}({}){}",
        func.name,
        params_str.join(", "),
        return_str
    );

    let parameters: Vec<ParameterInformation> = func
        .parameters
        .iter()
        .map(|p| {
            let param_label = if let Some(ref ty) = p.type_annotation {
                format!("{}: {}", p.name, ty)
            } else {
                p.name.clone()
            };
            ParameterInformation {
                label: ParameterLabel::Simple(param_label),
                documentation: None,
            }
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: func.documentation.as_ref().map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.clone(),
                })
            }),
            parameters: Some(parameters),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    }
}

/// Build SignatureHelp from a class_db method.
fn build_signature_from_classdb_method(
    method: &crate::classdb::MethodInfo,
    class_name: &str,
    active_param: u32,
) -> SignatureHelp {
    let params_str: Vec<String> = method
        .arguments
        .iter()
        .map(|a| format!("{}: {}", a.name, a.arg_type))
        .collect();

    let return_str = if method.return_type.is_empty() {
        String::new()
    } else {
        format!(" -> {}", method.return_type)
    };

    let label = format!(
        "{}.{}({}){}",
        class_name,
        method.name,
        params_str.join(", "),
        return_str
    );

    let parameters: Vec<ParameterInformation> = method
        .arguments
        .iter()
        .map(|a| ParameterInformation {
            label: ParameterLabel::Simple(format!("{}: {}", a.name, a.arg_type)),
            documentation: None,
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    }
}

/// Build SignatureHelp from a utility function.
fn build_signature_from_utility(
    util: &crate::classdb::UtilityFunctionInfo,
    active_param: u32,
) -> SignatureHelp {
    let params_str: Vec<String> = util
        .arguments
        .iter()
        .map(|a| format!("{}: {}", a.name, a.arg_type))
        .collect();

    let return_str = if util.return_type.is_empty() {
        String::new()
    } else {
        format!(" -> {}", util.return_type)
    };

    let label = format!("{}({}){}", util.name, params_str.join(", "), return_str);

    let parameters: Vec<ParameterInformation> = util
        .arguments
        .iter()
        .map(|a| ParameterInformation {
            label: ParameterLabel::Simple(format!("{}: {}", a.name, a.arg_type)),
            documentation: None,
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: None,
            parameters: Some(parameters),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    }
}

/// Build SignatureHelp for a constructor call.
fn build_constructor_signature(
    class_name: &str,
    init_func: &crate::symbols::FuncDecl,
    active_param: u32,
) -> SignatureHelp {
    let params_str: Vec<String> = init_func
        .parameters
        .iter()
        .map(|p| {
            if let Some(ref ty) = p.type_annotation {
                format!("{}: {}", p.name, ty)
            } else {
                p.name.clone()
            }
        })
        .collect();

    let label = format!("{}.new({})", class_name, params_str.join(", "));

    let parameters: Vec<ParameterInformation> = init_func
        .parameters
        .iter()
        .map(|p| {
            let param_label = if let Some(ref ty) = p.type_annotation {
                format!("{}: {}", p.name, ty)
            } else {
                p.name.clone()
            };
            ParameterInformation {
                label: ParameterLabel::Simple(param_label),
                documentation: None,
            }
        })
        .collect();

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label,
            documentation: init_func.documentation.as_ref().map(|d| {
                Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: d.clone(),
                })
            }),
            parameters: Some(parameters),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    }
}
