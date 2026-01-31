use std::collections::HashMap;
use std::path::PathBuf;

use tree_sitter::Node;

use crate::classdb::ClassDb;
use crate::parser::{self, ParsedFile};
use crate::project_index::ProjectIndex;
use crate::scene::SceneFile;
use crate::symbols::FileSymbols;

/// A pending usage mark: (target_file_index, member_name).
struct UsageMark {
    file_idx: usize,
    name: String,
}

/// Run all cross-file usage marking passes.
pub fn mark_cross_file_usage(
    file_symbols: &mut [FileSymbols],
    parsed_files: &[(PathBuf, ParsedFile)],
    index: &ProjectIndex,
    scenes: &HashMap<PathBuf, SceneFile>,
    _class_db: &ClassDb,
) {
    // Sub-pass 0: Mark autoload public functions as used (callable from anywhere)
    for (_, path) in index.autoloads() {
        let Some(file_idx) = index.index_for_path(path) else {
            continue;
        };
        if file_idx >= file_symbols.len() {
            continue;
        }
        let func_names: Vec<String> = file_symbols[file_idx]
            .functions
            .iter()
            .filter(|f| !f.name.starts_with('_'))
            .map(|f| f.name.clone())
            .collect();
        for name in func_names {
            file_symbols[file_idx]
                .functions
                .iter_mut()
                .filter(|f| f.name == name)
                .for_each(|f| f.used = true);
        }
    }

    // Sub-pass 1: Inheritance usage
    mark_inheritance_usage(file_symbols, parsed_files);

    // Sub-pass 2: Attribute access usage
    mark_attribute_usage(file_symbols, parsed_files, index);

    // Sub-pass 3: Scene usage (signals, properties)
    mark_scene_usage(file_symbols, scenes, index);

    // Sub-pass 4: Connect/emit call detection
    mark_connect_emit_usage(file_symbols, parsed_files, index);
}

// --- Sub-pass 1: Inheritance usage ---

/// Mark parent members as used if they're referenced by child files.
fn mark_inheritance_usage(
    file_symbols: &mut [FileSymbols],
    parsed_files: &[(PathBuf, ParsedFile)],
) {
    // Build child->parent map (direct parent only)
    let mut child_to_parent: HashMap<usize, usize> = HashMap::new();
    for (i, fs) in file_symbols.iter().enumerate() {
        if let Some(ref parent_path) = fs.parent_file {
            for (j, other) in file_symbols.iter().enumerate() {
                if j == i {
                    continue;
                }
                let matches = if let (Ok(a), Ok(b)) =
                    (other.path.canonicalize(), parent_path.canonicalize())
                {
                    a == b
                } else {
                    other.path == *parent_path
                };
                if matches {
                    child_to_parent.insert(i, j);
                    break;
                }
            }
        }
    }

    // Build parent->children map
    let mut parent_children: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&child, &parent) in &child_to_parent {
        parent_children.entry(parent).or_default().push(child);
    }

    // Collect all ancestor function names for each file (walk up the chain)
    let mut ancestor_funcs: HashMap<usize, Vec<String>> = HashMap::new();
    for i in 0..file_symbols.len() {
        let mut funcs = Vec::new();
        let mut current = i;
        while let Some(&parent_idx) = child_to_parent.get(&current) {
            for f in &file_symbols[parent_idx].functions {
                if !funcs.contains(&f.name) {
                    funcs.push(f.name.clone());
                }
            }
            current = parent_idx;
        }
        if !funcs.is_empty() {
            ancestor_funcs.insert(i, funcs);
        }
    }

    let mut marks: Vec<UsageMark> = Vec::new();

    for (&parent_idx, children) in &parent_children {
        // Collect parent member names (variables + functions + signals)
        let parent_member_names: Vec<String> = file_symbols[parent_idx]
            .variables
            .iter()
            .map(|v| v.name.clone())
            .chain(
                file_symbols[parent_idx]
                    .functions
                    .iter()
                    .map(|f| f.name.clone()),
            )
            .chain(
                file_symbols[parent_idx]
                    .signals
                    .iter()
                    .map(|s| s.name.clone()),
            )
            .collect();

        for &child_idx in children {
            if child_idx >= parsed_files.len() {
                continue;
            }
            let parsed = &parsed_files[child_idx].1;
            let root = parsed.root_node();

            // Collect all identifiers in the child file
            let identifiers = parser::find_nodes_by_kind(root, "identifier");
            let names = parser::find_nodes_by_kind(root, "name");

            let all_refs: Vec<&str> = identifiers
                .iter()
                .chain(names.iter())
                .map(|n| parsed.node_text(*n))
                .collect();

            for member_name in &parent_member_names {
                if all_refs.contains(&member_name.as_str()) {
                    marks.push(UsageMark {
                        file_idx: parent_idx,
                        name: member_name.clone(),
                    });
                }
            }
        }
    }

    // Mark child functions as used if they override any ancestor function
    for (file_idx, anc_funcs) in &ancestor_funcs {
        for func_name in anc_funcs {
            if file_symbols[*file_idx]
                .functions
                .iter()
                .any(|f| f.name == *func_name)
            {
                marks.push(UsageMark {
                    file_idx: *file_idx,
                    name: func_name.clone(),
                });
            }
        }
    }

    apply_marks(file_symbols, &marks);
}

// --- Sub-pass 2: Attribute access usage ---

/// Mark members as used when accessed via typed attribute access (e.g., `player.health`).
fn mark_attribute_usage(
    file_symbols: &mut [FileSymbols],
    parsed_files: &[(PathBuf, ParsedFile)],
    index: &ProjectIndex,
) {
    let mut marks: Vec<UsageMark> = Vec::new();

    for (file_idx, (_, parsed)) in parsed_files.iter().enumerate() {
        let root = parsed.root_node();
        let file_sym = &file_symbols[file_idx];

        // Walk AST for attribute nodes
        collect_attribute_marks(root, parsed, file_sym, file_symbols, index, &mut marks);
    }

    apply_marks(file_symbols, &marks);
}

fn collect_attribute_marks(
    node: Node,
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    index: &ProjectIndex,
    marks: &mut Vec<UsageMark>,
) {
    if node.kind() == "attribute" {
        // attribute node children can be:
        //   [identifier "obj", identifier "prop"]        → property access: obj.prop
        //   [identifier "obj", attribute_call "method(args)"] → method call: obj.method(args)
        let receiver = get_attribute_receiver(node);
        if let Some(receiver) = receiver {
            let receiver_text = parsed.node_text(receiver);

            if let Some(target_idx) =
                resolve_receiver_type(receiver_text, file_sym, all_file_symbols, index)
            {
                // Determine the accessed member name
                if let Some(member_name) = get_attribute_member_name(node, parsed) {
                    marks.push(UsageMark {
                        file_idx: target_idx,
                        name: member_name,
                    });
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_attribute_marks(child, parsed, file_sym, all_file_symbols, index, marks);
    }
}

/// Get the receiver node of an attribute access (the object before the dot).
fn get_attribute_receiver(node: Node) -> Option<Node> {
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

/// Get the member name being accessed on an attribute node.
/// Handles both property access (a.b → "b") and method calls (a.method() → "method").
fn get_attribute_member_name(attr_node: Node, parsed: &ParsedFile) -> Option<String> {
    let mut cursor = attr_node.walk();
    let named_children: Vec<_> = attr_node
        .children(&mut cursor)
        .filter(|c| c.is_named())
        .collect();

    if named_children.len() < 2 {
        return None;
    }

    let second = named_children[1];
    match second.kind() {
        "identifier" | "name" => Some(parsed.node_text(second).to_string()),
        "attribute_call" => {
            // The method name is the first named child (identifier) of attribute_call
            let mut inner_cursor = second.walk();
            for child in second.children(&mut inner_cursor) {
                if child.is_named() && (child.kind() == "identifier" || child.kind() == "name") {
                    return Some(parsed.node_text(child).to_string());
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve a receiver identifier to a file index in the index.
fn resolve_receiver_type(
    receiver_name: &str,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    index: &ProjectIndex,
) -> Option<usize> {
    // 1. Check if receiver is an autoload singleton
    if let Some(idx) = index.index_for_autoload(receiver_name) {
        return Some(idx);
    }

    // 2. Check if receiver is a class_name (static access like ClassName.member)
    if let Some(idx) = index.index_for_class_name(receiver_name) {
        return Some(idx);
    }

    // 3. Check if receiver is a local variable with a known type
    // Check member variables
    for var in &file_sym.variables {
        if var.name == receiver_name {
            if let Some(ref itype) = var.inferred_type {
                if let Some(idx) = index.index_for_class_name(itype) {
                    return Some(idx);
                }
            }
            if let Some(ref itype) = var.initializer_type {
                if let Some(idx) = index.index_for_class_name(itype) {
                    return Some(idx);
                }
            }
        }
    }

    // Check function local variables and parameters
    for func in &file_sym.functions {
        for var in &func.local_vars {
            if var.name == receiver_name {
                if let Some(ref itype) = var.inferred_type {
                    if let Some(idx) = index.index_for_class_name(itype) {
                        return Some(idx);
                    }
                }
                if let Some(ref itype) = var.initializer_type {
                    if let Some(idx) = index.index_for_class_name(itype) {
                        return Some(idx);
                    }
                }
                if let Some(ref ta) = var.type_annotation {
                    if let Some(idx) = index.index_for_class_name(ta) {
                        return Some(idx);
                    }
                }
            }
        }
        for param in &func.parameters {
            if param.name == receiver_name {
                if let Some(ref ta) = param.type_annotation {
                    if let Some(idx) = index.index_for_class_name(ta) {
                        return Some(idx);
                    }
                }
            }
        }
    }

    // 4. Check if receiver is a preload binding
    for preload in &file_sym.preloads {
        if preload.binding_name == receiver_name {
            if let Some(idx) = index.index_for_res_path(&preload.res_path) {
                return Some(idx);
            }
        }
    }

    // 5. Check all files for ClassName.new() pattern
    //    If the receiver's type annotation or inferred type matches a user class
    for other_sym in all_file_symbols {
        if let Some(ref cn) = other_sym.class_name {
            // Check if receiver has type annotation matching this class
            for var in &file_sym.variables {
                if var.name == receiver_name {
                    if let Some(ref ta) = var.type_annotation {
                        if ta == cn {
                            return index.index_for_class_name(cn);
                        }
                    }
                }
            }
        }
    }

    None
}

// --- Sub-pass 3: Scene usage ---

/// Mark members as used based on scene file data (signal connections, properties).
fn mark_scene_usage(
    file_symbols: &mut [FileSymbols],
    scenes: &HashMap<PathBuf, SceneFile>,
    index: &ProjectIndex,
) {
    let mut marks: Vec<UsageMark> = Vec::new();

    for scene in scenes.values() {
        // For each signal connection
        for conn in &scene.connections {
            // Resolve source node's script
            let from_node = if conn.from_node == "." {
                scene.nodes.iter().find(|n| n.parent.is_empty())
            } else {
                scene.nodes.iter().find(|n| n.node_path == conn.from_node)
            };

            if let Some(from_node) = from_node {
                if let Some(source_file_idx) = resolve_node_script(from_node, scene, index) {
                    // Mark signal as used in source file
                    marks.push(UsageMark {
                        file_idx: source_file_idx,
                        name: conn.signal.clone(),
                    });
                }
            }

            // Resolve target node's script
            let to_node = if conn.to_node == "." {
                scene.nodes.iter().find(|n| n.parent.is_empty())
            } else {
                scene.nodes.iter().find(|n| n.node_path == conn.to_node)
            };

            if let Some(to_node) = to_node {
                if let Some(target_file_idx) = resolve_node_script(to_node, scene, index) {
                    // Mark handler method as used in target file
                    marks.push(UsageMark {
                        file_idx: target_file_idx,
                        name: conn.method.clone(),
                    });
                }
            }
        }

        // For each node with a script attached
        for node in &scene.nodes {
            if let Some(file_idx) = resolve_node_script(node, scene, index) {
                // Mark all public functions as potentially used (called via untyped references)
                let func_names: Vec<String> = file_symbols[file_idx]
                    .functions
                    .iter()
                    .filter(|f| !f.name.starts_with('_'))
                    .map(|f| f.name.clone())
                    .collect();
                for name in func_names {
                    marks.push(UsageMark { file_idx, name });
                }

                // Mark properties set on this node
                for prop in &node.properties {
                    marks.push(UsageMark {
                        file_idx,
                        name: prop.clone(),
                    });
                }
            }
        }
    }

    apply_marks(file_symbols, &marks);
}

/// Resolve a scene node's attached script to a file index.
fn resolve_node_script(
    node: &crate::scene::SceneNode,
    scene: &SceneFile,
    index: &ProjectIndex,
) -> Option<usize> {
    let script_id = node.script_id.as_ref()?;
    let ext_res = scene.ext_resources.iter().find(|r| r.id == *script_id)?;
    // Try res:// path lookup
    index.index_for_res_path(&ext_res.path).or_else(|| {
        // Try matching by path suffix
        let relative = ext_res.path.strip_prefix("res://").unwrap_or(&ext_res.path);
        for (path, _) in index.iter() {
            let path_str = path.to_string_lossy();
            if path_str.ends_with(relative) {
                return index.index_for_path(path);
            }
        }
        None
    })
}

// --- Sub-pass 4: Connect/emit call detection ---

/// Mark signals and methods as used from connect/emit/call_deferred patterns.
fn mark_connect_emit_usage(
    file_symbols: &mut [FileSymbols],
    parsed_files: &[(PathBuf, ParsedFile)],
    index: &ProjectIndex,
) {
    let mut marks: Vec<UsageMark> = Vec::new();

    for (file_idx, (_, parsed)) in parsed_files.iter().enumerate() {
        let root = parsed.root_node();
        let file_sym = &file_symbols[file_idx];
        collect_connect_emit_marks(
            root,
            parsed,
            file_sym,
            file_symbols,
            index,
            file_idx,
            &mut marks,
        );
    }

    apply_marks(file_symbols, &marks);
}

fn collect_connect_emit_marks(
    node: Node,
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    index: &ProjectIndex,
    current_file_idx: usize,
    marks: &mut Vec<UsageMark>,
) {
    if node.kind() == "attribute" {
        // Handle signal.emit/connect and call_deferred patterns
        // Structure: attribute { receiver, attribute_call "method(args)" }
        let mut cursor = node.walk();
        let named_children: Vec<_> = node
            .children(&mut cursor)
            .filter(|c| c.is_named())
            .collect();

        if named_children.len() >= 2 && named_children[1].kind() == "attribute_call" {
            let receiver = named_children[0];
            let attr_call = named_children[1];

            // Get the method name from attribute_call
            let method_name = {
                let mut inner_cursor = attr_call.walk();
                let node = attr_call
                    .children(&mut inner_cursor)
                    .find(|c| c.is_named() && (c.kind() == "identifier" || c.kind() == "name"));
                node.map(|n| parsed.node_text(n).to_string())
            };

            if let Some(ref method_name) = method_name {
                // Godot 4: signal_name.emit() or obj.signal_name.emit()
                if method_name == "emit" {
                    if let Some((signal_name, owner_idx)) = extract_signal_from_receiver(
                        receiver,
                        parsed,
                        file_sym,
                        all_file_symbols,
                        index,
                        current_file_idx,
                    ) {
                        marks.push(UsageMark {
                            file_idx: owner_idx,
                            name: signal_name,
                        });
                    }
                }

                // Godot 4: signal_name.connect(handler) or obj.signal_name.connect(handler)
                // Check if there's NO string first argument (Godot 4 style)
                if method_name == "connect" {
                    let has_string_arg =
                        get_string_arg_from_attr_call(attr_call, parsed, 0).is_some();
                    if !has_string_arg {
                        // Godot 4 style: the signal is the receiver
                        if let Some((signal_name, owner_idx)) = extract_signal_from_receiver(
                            receiver,
                            parsed,
                            file_sym,
                            all_file_symbols,
                            index,
                            current_file_idx,
                        ) {
                            marks.push(UsageMark {
                                file_idx: owner_idx,
                                name: signal_name,
                            });
                        }
                    }
                }

                if method_name == "call_deferred" || method_name == "call" {
                    if let Some(called_method) = get_string_arg_from_attr_call(attr_call, parsed, 0)
                    {
                        let receiver_text = parsed.node_text(receiver);
                        if let Some(target_idx) =
                            resolve_receiver_type(receiver_text, file_sym, all_file_symbols, index)
                        {
                            marks.push(UsageMark {
                                file_idx: target_idx,
                                name: called_method,
                            });
                        } else {
                            marks.push(UsageMark {
                                file_idx: current_file_idx,
                                name: called_method,
                            });
                        }
                    }
                }
            }
        }
    }

    // Handle bare call_deferred("method_name") / call("method_name") without receiver
    if node.kind() == "call" {
        if let Some(func_node) = node.child(0) {
            if func_node.kind() == "identifier" || func_node.kind() == "name" {
                let func_text = parsed.node_text(func_node);
                if func_text == "call_deferred" || func_text == "call" {
                    if let Some(first_arg) = get_string_arg(node, parsed, 0) {
                        marks.push(UsageMark {
                            file_idx: current_file_idx,
                            name: first_arg,
                        });
                    }
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_connect_emit_marks(
            child,
            parsed,
            file_sym,
            all_file_symbols,
            index,
            current_file_idx,
            marks,
        );
    }
}

/// Extract signal name and owner file index from a receiver node.
///
/// Handles both patterns:
/// - `signal_name.emit()` → signal_name is an identifier, owner is current file
/// - `obj.signal_name.emit()` → signal_name is last part of attribute, owner resolved from obj
///
/// Returns (signal_name, owner_file_idx) if successful.
fn extract_signal_from_receiver(
    receiver: Node,
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    index: &ProjectIndex,
    current_file_idx: usize,
) -> Option<(String, usize)> {
    match receiver.kind() {
        // signal_name.emit() - bare identifier, signal is on self
        "identifier" | "name" => {
            let signal_name = parsed.node_text(receiver).to_string();
            Some((signal_name, current_file_idx))
        }
        // obj.signal_name.emit() - attribute chain
        "attribute" => {
            let mut cursor = receiver.walk();
            let named_children: Vec<_> = receiver
                .children(&mut cursor)
                .filter(|c| c.is_named())
                .collect();

            if named_children.len() >= 2 {
                // First child is the object, second is the signal name
                let obj = named_children[0];
                let signal_node = named_children[1];

                // Get signal name from the last part
                let signal_name =
                    if signal_node.kind() == "identifier" || signal_node.kind() == "name" {
                        parsed.node_text(signal_node).to_string()
                    } else {
                        return None;
                    };

                // Try to resolve the object to a file index
                let obj_text = parsed.node_text(obj);
                if let Some(target_idx) =
                    resolve_receiver_type(obj_text, file_sym, all_file_symbols, index)
                {
                    Some((signal_name, target_idx))
                } else {
                    // Can't resolve object, assume it's on current file
                    Some((signal_name, current_file_idx))
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Get the nth string argument from an attribute_call node's arguments.
fn get_string_arg_from_attr_call(
    attr_call: Node,
    parsed: &ParsedFile,
    arg_index: usize,
) -> Option<String> {
    let mut cursor = attr_call.walk();
    for child in attr_call.children(&mut cursor) {
        if child.kind() == "arguments" {
            let mut arg_cursor = child.walk();
            let mut idx = 0;
            for arg in child.children(&mut arg_cursor) {
                if !arg.is_named() {
                    continue;
                }
                if idx == arg_index && arg.kind() == "string" {
                    let text = parsed.node_text(arg);
                    let unquoted = text.trim_matches('"').trim_matches('\'');
                    return Some(unquoted.to_string());
                }
                idx += 1;
            }
        }
    }
    None
}

/// Get the nth string argument from a call node's arguments.
fn get_string_arg(call_node: Node, parsed: &ParsedFile, arg_index: usize) -> Option<String> {
    let mut cursor = call_node.walk();
    for child in call_node.children(&mut cursor) {
        if child.kind() == "arguments" {
            let mut arg_cursor = child.walk();
            let mut idx = 0;
            for arg in child.children(&mut arg_cursor) {
                if !arg.is_named() {
                    continue;
                }
                if idx == arg_index && arg.kind() == "string" {
                    let text = parsed.node_text(arg);
                    let unquoted = text.trim_matches('"').trim_matches('\'');
                    return Some(unquoted.to_string());
                }
                idx += 1;
            }
        }
    }
    None
}

// --- Helpers ---

/// Apply a set of usage marks to file symbols.
fn apply_marks(file_symbols: &mut [FileSymbols], marks: &[UsageMark]) {
    for mark in marks {
        if mark.file_idx >= file_symbols.len() {
            continue;
        }
        let fs = &mut file_symbols[mark.file_idx];

        // Mark matching variables
        for var in &mut fs.variables {
            if var.name == mark.name {
                var.used = true;
            }
        }

        // Mark matching functions
        for func in &mut fs.functions {
            if func.name == mark.name {
                func.used = true;
            }
        }

        // Mark matching signals
        for sig in &mut fs.signals {
            if sig.name == mark.name {
                sig.used = true;
            }
        }
    }
}

/// Check if a file has dynamic access patterns that make it unsafe to flag unused members.
/// Returns true if the file uses `get()`, `set()`, or `call()` with non-literal arguments.
pub fn has_dynamic_access(parsed: &ParsedFile) -> bool {
    let root = parsed.root_node();
    check_dynamic_access(root, parsed)
}

fn check_dynamic_access(node: Node, parsed: &ParsedFile) -> bool {
    if node.kind() == "call" {
        if let Some(func_node) = node.child(0) {
            if func_node.kind() == "identifier" || func_node.kind() == "name" {
                let func_text = parsed.node_text(func_node);
                // Direct get/set/call with variable args
                if matches!(func_text, "get" | "set" | "call")
                    && get_string_arg(node, parsed, 0).is_none()
                {
                    return true;
                }
            }
        }
    }

    // Method calls: obj.get(), obj.set(), obj.call() — attribute with attribute_call
    if node.kind() == "attribute" {
        let mut cursor = node.walk();
        let named_children: Vec<_> = node
            .children(&mut cursor)
            .filter(|c| c.is_named())
            .collect();
        if named_children.len() >= 2 && named_children[1].kind() == "attribute_call" {
            let attr_call = named_children[1];
            let method_name = {
                let mut inner_cursor = attr_call.walk();
                let node = attr_call
                    .children(&mut inner_cursor)
                    .find(|c| c.is_named() && (c.kind() == "identifier" || c.kind() == "name"));
                node.map(|n| parsed.node_text(n).to_string())
            };
            if let Some(ref method) = method_name {
                if matches!(method.as_str(), "get" | "set" | "call")
                    && get_string_arg_from_attr_call(attr_call, parsed, 0).is_none()
                {
                    return true;
                }
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if check_dynamic_access(child, parsed) {
            return true;
        }
    }
    false
}
