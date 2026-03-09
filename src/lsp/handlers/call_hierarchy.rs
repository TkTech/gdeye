//! Call hierarchy handlers.

use std::collections::HashSet;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::lsp::Backend;
use crate::symbol_index::SymbolAtResult;

/// Handle prepare call hierarchy request.
pub async fn prepare_call_hierarchy(
    backend: &Backend,
    params: CallHierarchyPrepareParams,
) -> Result<Option<Vec<CallHierarchyItem>>> {
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

    let index = match state.get_index(&uri) {
        Some(i) => i,
        None => return Ok(None),
    };

    // Convert position to byte offset
    let offset = match doc.offset_at(position.line as usize, position.character as usize) {
        Some(o) => o,
        None => return Ok(None),
    };

    // Find function at position
    let symbol_result = index.symbol_at(offset);

    let func_name = match symbol_result {
        Some(SymbolAtResult::Definition(def)) if def.kind.as_str() == "function" => {
            def.name.clone()
        }
        Some(SymbolAtResult::Reference(ref_)) => {
            // Check if it references a function
            if let Some(def) = index.resolve_reference(ref_) {
                if def.kind.as_str() == "function" {
                    def.name.clone()
                } else {
                    return Ok(None);
                }
            } else {
                // Maybe it's a function name
                ref_.name.clone()
            }
        }
        _ => return Ok(None),
    };

    // Find the function in symbols
    let func = match symbols.functions.iter().find(|f| f.name == func_name) {
        Some(f) => f,
        None => return Ok(None),
    };

    // Create the call hierarchy item
    let (start_line, start_col) = doc.position_at(func.start_byte);
    let (end_line, end_col) = doc.position_at(func.end_byte);
    let (name_start_line, name_start_col) = doc.position_at(func.name_start_byte);
    let name_end_byte = func.name_start_byte + func.name.len();
    let (name_end_line, name_end_col) = doc.position_at(name_end_byte);

    Ok(Some(vec![CallHierarchyItem {
        name: func.name.clone(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: func
            .return_type
            .clone()
            .or(func.inferred_return_type.clone()),
        uri: params
            .text_document_position_params
            .text_document
            .uri
            .clone(),
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
        selection_range: Range {
            start: Position {
                line: name_start_line as u32,
                character: name_start_col as u32,
            },
            end: Position {
                line: name_end_line as u32,
                character: name_end_col as u32,
            },
        },
        data: None,
    }]))
}

/// Handle incoming calls request.
pub async fn incoming_calls(
    backend: &Backend,
    params: CallHierarchyIncomingCallsParams,
) -> Result<Option<Vec<CallHierarchyIncomingCall>>> {
    let target_name = &params.item.name;
    let target_uri = &params.item.uri;
    let state = backend.state.read().await;

    let mut calls: Vec<CallHierarchyIncomingCall> = Vec::new();
    let mut seen_callers: HashSet<(String, String)> = HashSet::new(); // (file, func)

    // Use call graph for accurate call site information
    let call_graph = state.call_graph_arc();

    // Get the target file path
    let target_path = target_uri.to_file_path().ok();

    // Look for call sites to this function across all files where it might be defined
    if let Some(ref target_path) = target_path {
        if let Some(call_sites) = call_graph.get_call_sites(target_path, target_name) {
            for site in call_sites {
                // Skip if no caller function (file-level calls)
                let caller_func_name = match &site.caller_func {
                    Some(name) => name,
                    None => continue,
                };

                // Skip duplicates
                let key = (
                    site.caller_file.display().to_string(),
                    caller_func_name.clone(),
                );
                if seen_callers.contains(&key) {
                    continue;
                }
                seen_callers.insert(key);

                // Get caller function details
                let (caller_uri, caller_func) =
                    match find_function_details(&state, &site.caller_file, caller_func_name) {
                        Some(details) => details,
                        None => continue,
                    };

                calls.push(CallHierarchyIncomingCall {
                    from: CallHierarchyItem {
                        name: caller_func.name.clone(),
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        detail: caller_func
                            .return_type
                            .clone()
                            .or(caller_func.inferred_return_type.clone()),
                        uri: caller_uri,
                        range: Range {
                            start: Position {
                                line: caller_func.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: caller_func.end_line.saturating_sub(1) as u32,
                                character: 0,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: caller_func.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: caller_func.line.saturating_sub(1) as u32,
                                character: caller_func.name.len() as u32,
                            },
                        },
                        data: None,
                    },
                    from_ranges: vec![Range {
                        start: Position {
                            line: site.line.saturating_sub(1) as u32,
                            character: 0,
                        },
                        end: Position {
                            line: site.line.saturating_sub(1) as u32,
                            character: target_name.len() as u32,
                        },
                    }],
                });
            }
        }
    }

    // Also check all files with this function name (for cross-file calls)
    if let Some(files) = call_graph.functions_by_name.get(target_name) {
        for file in files {
            // Skip if same as target
            if target_path.as_ref() == Some(file) {
                continue;
            }
            if let Some(sites) = call_graph.get_call_sites(file, target_name) {
                for site in sites {
                    let caller_func_name = match &site.caller_func {
                        Some(name) => name,
                        None => continue,
                    };

                    let key = (
                        site.caller_file.display().to_string(),
                        caller_func_name.clone(),
                    );
                    if seen_callers.contains(&key) {
                        continue;
                    }
                    seen_callers.insert(key);

                    let (caller_uri, caller_func) =
                        match find_function_details(&state, &site.caller_file, caller_func_name) {
                            Some(details) => details,
                            None => continue,
                        };

                    calls.push(CallHierarchyIncomingCall {
                        from: CallHierarchyItem {
                            name: caller_func.name.clone(),
                            kind: SymbolKind::FUNCTION,
                            tags: None,
                            detail: caller_func
                                .return_type
                                .clone()
                                .or(caller_func.inferred_return_type.clone()),
                            uri: caller_uri,
                            range: Range {
                                start: Position {
                                    line: caller_func.line.saturating_sub(1) as u32,
                                    character: 0,
                                },
                                end: Position {
                                    line: caller_func.end_line.saturating_sub(1) as u32,
                                    character: 0,
                                },
                            },
                            selection_range: Range {
                                start: Position {
                                    line: caller_func.line.saturating_sub(1) as u32,
                                    character: 0,
                                },
                                end: Position {
                                    line: caller_func.line.saturating_sub(1) as u32,
                                    character: caller_func.name.len() as u32,
                                },
                            },
                            data: None,
                        },
                        from_ranges: vec![Range {
                            start: Position {
                                line: site.line.saturating_sub(1) as u32,
                                character: 0,
                            },
                            end: Position {
                                line: site.line.saturating_sub(1) as u32,
                                character: target_name.len() as u32,
                            },
                        }],
                    });
                }
            }
        }
    }

    if calls.is_empty() {
        Ok(None)
    } else {
        Ok(Some(calls))
    }
}

/// Find function details by file path and function name.
fn find_function_details(
    state: &crate::lsp::state::ServerState,
    file: &std::path::Path,
    func_name: &str,
) -> Option<(Url, crate::symbols::FuncDecl)> {
    let file = state.project_index.get(file)?;
    let func = file
        .symbols
        .functions
        .iter()
        .find(|f| f.name == func_name)?;
    let uri = Url::from_file_path(&file.symbols.path).ok()?;
    Some((uri, func.clone()))
}

/// Handle outgoing calls request.
pub async fn outgoing_calls(
    backend: &Backend,
    params: CallHierarchyOutgoingCallsParams,
) -> Result<Option<Vec<CallHierarchyOutgoingCall>>> {
    let func_uri = &params.item.uri;
    let func_name = &params.item.name;

    let state = backend.state.read().await;
    let call_graph = state.call_graph_arc();

    // Get the file path
    let file_path = match func_uri.to_file_path() {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };

    // Use call graph forward edges for outgoing calls
    let caller_key = (file_path.clone(), func_name.to_string());

    let mut calls: Vec<CallHierarchyOutgoingCall> = Vec::new();

    if let Some(callees) = call_graph.calls.get(&caller_key) {
        for (callee_file, callee_name) in callees {
            // Get callee function details
            let (target_uri, target_func) =
                match find_function_details(&state, callee_file, callee_name) {
                    Some(details) => details,
                    None => {
                        // Function not found in index - use placeholder
                        let uri = Url::from_file_path(callee_file)
                            .unwrap_or_else(|_| Url::parse("file:///unknown").unwrap());
                        (uri, create_placeholder_func(callee_name))
                    }
                };

            // Find the call site line (look up in the call graph)
            let call_line = find_call_line(&call_graph, &file_path, func_name, callee_name);

            calls.push(CallHierarchyOutgoingCall {
                to: CallHierarchyItem {
                    name: callee_name.clone(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    detail: target_func
                        .return_type
                        .clone()
                        .or(target_func.inferred_return_type.clone()),
                    uri: target_uri,
                    range: Range {
                        start: Position {
                            line: target_func.line.saturating_sub(1) as u32,
                            character: 0,
                        },
                        end: Position {
                            line: target_func.end_line.saturating_sub(1) as u32,
                            character: 0,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: target_func.line.saturating_sub(1) as u32,
                            character: 0,
                        },
                        end: Position {
                            line: target_func.line.saturating_sub(1) as u32,
                            character: callee_name.len() as u32,
                        },
                    },
                    data: None,
                },
                from_ranges: vec![Range {
                    start: Position {
                        line: call_line.saturating_sub(1) as u32,
                        character: 0,
                    },
                    end: Position {
                        line: call_line.saturating_sub(1) as u32,
                        character: callee_name.len() as u32,
                    },
                }],
            });
        }
    }

    if calls.is_empty() {
        Ok(None)
    } else {
        Ok(Some(calls))
    }
}

/// Find the line where a call from caller to callee occurs.
fn find_call_line(
    call_graph: &crate::call_graph::CallGraph,
    callee_file: &std::path::Path,
    caller_func: &str,
    callee_name: &str,
) -> usize {
    // Look through call sites to find where caller calls callee
    if let Some(target) = call_graph
        .targets
        .get(&(callee_file.to_path_buf(), callee_name.to_string()))
    {
        for site in &target.call_sites {
            if site.caller_func.as_deref() == Some(caller_func) {
                return site.line;
            }
        }
    }
    1 // Default if not found
}

/// Create a placeholder function for unknown callees.
fn create_placeholder_func(name: &str) -> crate::symbols::FuncDecl {
    crate::symbols::FuncDecl {
        name: name.to_string(),
        parameters: Vec::new(),
        return_type: None,
        inferred_return_type: None,
        line: 1,
        end_line: 1,
        local_vars: Vec::new(),
        used: false,
        start_byte: 0,
        end_byte: 0,
        name_start_byte: 0,
        name_end_byte: 0,
        documentation: None,
        is_static: false,
    }
}
