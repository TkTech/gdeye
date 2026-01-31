use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::parser::{self, ParsedFile};
use crate::project::ProjectInfo;

/// All symbols extracted from a single file.
#[derive(Debug, Clone)]
pub struct FileSymbols {
    pub path: PathBuf,
    pub class_name: Option<String>,
    pub extends: Option<String>,
    pub signals: Vec<SignalDecl>,
    pub enums: Vec<EnumDecl>,
    pub constants: Vec<ConstDecl>,
    pub variables: Vec<VarDecl>,
    pub functions: Vec<FuncDecl>,
    pub inner_classes: Vec<InnerClass>,
    /// Resolved parent file path (from extends)
    pub parent_file: Option<PathBuf>,
    /// Autoload singleton names available as globals.
    pub autoloads: HashSet<String>,
    /// Preload bindings: `var X = preload("res://...")` or `const X = preload("res://...")`
    pub preloads: Vec<PreloadEntry>,
}

#[derive(Debug, Clone)]
pub struct SignalDecl {
    pub name: String,
    pub parameters: Vec<String>,
    pub line: usize,
    pub used: bool,
    /// Byte offset of the start of the signal statement.
    pub start_byte: usize,
    /// Byte offset of the end of the signal statement.
    pub end_byte: usize,
    /// Byte offset of the start of the signal name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the signal name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this signal.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub values: Vec<String>,
    pub line: usize,
    /// Byte offset of the start of the enum definition.
    pub start_byte: usize,
    /// Byte offset of the end of the enum definition.
    pub end_byte: usize,
    /// Byte offset of the start of the enum name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the enum name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this enum.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub type_annotation: Option<String>,
    pub line: usize,
    /// Byte offset of the start of the const statement.
    pub start_byte: usize,
    /// Byte offset of the end of the const statement.
    pub end_byte: usize,
    /// Byte offset of the start of the const name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the const name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this constant.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub type_annotation: Option<String>,
    pub inferred_type: Option<String>,
    /// Type inferred from the initializer expression (literal/constructor).
    pub initializer_type: Option<String>,
    /// Function name called in the initializer (for deferred type resolution).
    /// Set when the initializer is a direct call like `get_viewport()` or `my_func()`.
    pub initializer_call: Option<String>,
    pub is_onready: bool,
    pub is_export: bool,
    pub scope: Scope,
    pub line: usize,
    pub used: bool,
    /// Byte offset of the start of the variable_statement node.
    pub start_byte: usize,
    /// Byte offset of the end of the variable_statement node.
    pub end_byte: usize,
    /// Byte offset of the start of the variable name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the variable name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this variable.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: String,
    pub parameters: Vec<ParamDecl>,
    pub return_type: Option<String>,
    /// Return type inferred from analyzing return statements.
    pub inferred_return_type: Option<String>,
    pub line: usize,
    pub end_line: usize,
    pub local_vars: Vec<VarDecl>,
    pub used: bool,
    /// Byte offset of the start of the function definition.
    pub start_byte: usize,
    /// Byte offset of the end of the function definition.
    pub end_byte: usize,
    /// Byte offset of the start of the function name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the function name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this function.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PreloadEntry {
    pub binding_name: String,
    pub res_path: String,
}

#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub name: String,
    pub type_annotation: Option<String>,
    /// Type inferred from call site argument analysis.
    pub inferred_type: Option<String>,
    pub line: usize,
    pub used: bool,
    /// Byte offset of the start of the parameter (including type if present).
    pub start_byte: usize,
    /// Byte offset of the end of the parameter (including type if present).
    pub end_byte: usize,
    /// Byte offset of the start of the parameter name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the parameter name.
    pub name_end_byte: usize,
}

#[derive(Debug, Clone)]
pub struct InnerClass {
    pub name: String,
    pub extends: Option<String>,
    pub line: usize,
    /// Byte offset of the start of the class definition.
    pub start_byte: usize,
    /// Byte offset of the end of the class definition.
    pub end_byte: usize,
    /// Byte offset of the start of the class name.
    pub name_start_byte: usize,
    /// Byte offset of the end of the class name.
    pub name_end_byte: usize,
    /// Documentation comment (if any) preceding this class.
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    File,
    Function(String),
    #[allow(dead_code)] // For nested scope tracking
    Block,
}

/// Collect all symbol declarations from a parsed file.
pub fn collect_symbols(path: &Path, parsed: &ParsedFile) -> FileSymbols {
    let mut symbols = FileSymbols {
        path: path.to_path_buf(),
        class_name: None,
        extends: None,
        signals: Vec::new(),
        enums: Vec::new(),
        constants: Vec::new(),
        variables: Vec::new(),
        functions: Vec::new(),
        inner_classes: Vec::new(),
        parent_file: None,
        autoloads: HashSet::new(),
        preloads: Vec::new(),
    };

    let root = parsed.root_node();
    collect_from_node(root, parsed, &mut symbols, &Scope::File);
    mark_used_symbols(parsed, &mut symbols);
    symbols
}

fn collect_from_node(node: Node, parsed: &ParsedFile, symbols: &mut FileSymbols, scope: &Scope) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_name_statement" => {
                if let Some(name) = find_child_name(child, parsed) {
                    symbols.class_name = Some(name);
                }
            }
            "extends_statement" => {
                // The extends value might be a dotted name or a string path
                let text = parsed.node_text(child);
                let extends_value = text
                    .trim_start_matches("extends")
                    .trim()
                    .trim_end_matches('\n');
                symbols.extends = Some(extends_value.to_string());
            }
            "signal_statement" => {
                let (name, name_start, name_end) =
                    find_child_name_with_range(child, parsed).unwrap_or_default();
                let params = extract_signal_params(child, parsed);
                let documentation = extract_documentation(child, parsed);
                symbols.signals.push(SignalDecl {
                    name,
                    parameters: params,
                    line: child.start_position().row + 1,
                    used: false,
                    start_byte: child.start_byte(),
                    end_byte: child.end_byte(),
                    name_start_byte: name_start,
                    name_end_byte: name_end,
                    documentation,
                });
            }
            "enum_definition" => {
                let (name, name_start, name_end) =
                    find_child_name_with_range(child, parsed).unwrap_or_default();
                let values = extract_enum_values(child, parsed);
                let documentation = extract_documentation(child, parsed);
                symbols.enums.push(EnumDecl {
                    name,
                    values,
                    line: child.start_position().row + 1,
                    start_byte: child.start_byte(),
                    end_byte: child.end_byte(),
                    name_start_byte: name_start,
                    name_end_byte: name_end,
                    documentation,
                });
            }
            "variable_statement" => {
                let var = extract_variable(child, parsed, scope);
                if let Some(v) = var {
                    // Check for preload in variable value
                    if let Some(res_path) = extract_preload_path(child, parsed) {
                        symbols.preloads.push(PreloadEntry {
                            binding_name: v.name.clone(),
                            res_path,
                        });
                    }
                    symbols.variables.push(v);
                }
            }
            "const_statement" => {
                let (name, name_start, name_end) =
                    find_child_name_with_range(child, parsed).unwrap_or_default();
                let type_ann = child
                    .child_by_field_name("type")
                    .map(|n| parsed.node_text(n).to_string());
                let documentation = extract_documentation(child, parsed);
                // Check for preload in const value
                if let Some(res_path) = extract_preload_path(child, parsed) {
                    symbols.preloads.push(PreloadEntry {
                        binding_name: name.clone(),
                        res_path,
                    });
                }
                symbols.constants.push(ConstDecl {
                    name,
                    type_annotation: type_ann,
                    line: child.start_position().row + 1,
                    start_byte: child.start_byte(),
                    end_byte: child.end_byte(),
                    name_start_byte: name_start,
                    name_end_byte: name_end,
                    documentation,
                });
            }
            "function_definition" => {
                let func = extract_function(child, parsed);
                if let Some(f) = func {
                    symbols.functions.push(f);
                }
            }
            "class_definition" => {
                let (name, name_start, name_end) =
                    find_child_name_with_range(child, parsed).unwrap_or_default();
                let extends = find_inner_class_extends(child, parsed);
                let documentation = extract_documentation(child, parsed);
                symbols.inner_classes.push(InnerClass {
                    name,
                    extends,
                    line: child.start_position().row + 1,
                    start_byte: child.start_byte(),
                    end_byte: child.end_byte(),
                    name_start_byte: name_start,
                    name_end_byte: name_end,
                    documentation,
                });
            }
            _ => {}
        }
    }
}

/// Extract documentation comment preceding a node.
///
/// In GDScript, documentation comments are `##` comments that appear
/// immediately before a declaration with no blank lines in between.
fn extract_documentation(node: Node, parsed: &ParsedFile) -> Option<String> {
    let source = parsed.source();
    let start_byte = node.start_byte();

    // Look backwards from the node to find preceding comments
    let prefix = &source[..start_byte];

    // Split into lines and work backwards
    let lines: Vec<&str> = prefix.lines().collect();
    let mut doc_lines: Vec<&str> = Vec::new();

    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with("##") {
            // Documentation comment - extract content after ##
            let content = trimmed.strip_prefix("##").unwrap_or("").trim_start();
            doc_lines.push(content);
        } else if trimmed.is_empty() {
            // Blank line - stop if we already have doc lines
            if !doc_lines.is_empty() {
                break;
            }
            // Otherwise continue looking
        } else if trimmed.starts_with('#') {
            // Regular comment, not documentation
            break;
        } else if trimmed.starts_with('@') {
            // Annotation like @onready, @export - skip and continue
            continue;
        } else {
            // Other content - stop looking
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    // Reverse to get correct order
    doc_lines.reverse();
    Some(doc_lines.join("\n"))
}

/// Find the name node within a declaration and return both the name string and its byte range.
fn find_child_name_with_range(node: Node, parsed: &ParsedFile) -> Option<(String, usize, usize)> {
    // Try field name first
    if let Some(n) = node.child_by_field_name("name") {
        let text = parsed.node_text(n).to_string();
        if !text.is_empty() {
            return Some((text, n.start_byte(), n.end_byte()));
        }
    }
    // Fall back to finding a child node of kind "name"
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            return Some((
                parsed.node_text(child).to_string(),
                child.start_byte(),
                child.end_byte(),
            ));
        }
    }
    None
}

/// Find the text of a child node with kind "name", falling back to
/// `child_by_field_name("name")`.
fn find_child_name(node: Node, parsed: &ParsedFile) -> Option<String> {
    // Try field name first
    if let Some(n) = node.child_by_field_name("name") {
        let text = parsed.node_text(n).to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    // Fall back to finding a child node of kind "name"
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "name" {
            return Some(parsed.node_text(child).to_string());
        }
    }
    None
}

fn extract_signal_params(node: Node, parsed: &ParsedFile) -> Vec<String> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "parameters" {
            let mut param_cursor = child.walk();
            for param_child in child.children(&mut param_cursor) {
                if param_child.kind() == "identifier" {
                    params.push(parsed.node_text(param_child).to_string());
                }
            }
        }
    }
    params
}

fn extract_enum_values(node: Node, parsed: &ParsedFile) -> Vec<String> {
    let mut values = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // Handle enumerator_list (new grammar) or enum_body (old grammar)
        if child.kind() == "enumerator_list" || child.kind() == "enum_body" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "enumerator" {
                    // Get the identifier inside the enumerator
                    let mut enum_cursor = inner.walk();
                    for enum_child in inner.children(&mut enum_cursor) {
                        if enum_child.kind() == "identifier" || enum_child.kind() == "name" {
                            let name = parsed.node_text(enum_child).to_string();
                            if !name.is_empty() {
                                values.push(name);
                            }
                            break;
                        }
                    }
                }
            }
        }
        // Also handle direct enumerator children (some grammar versions)
        if child.kind() == "enumerator" {
            let mut enum_cursor = child.walk();
            for enum_child in child.children(&mut enum_cursor) {
                if enum_child.kind() == "identifier" || enum_child.kind() == "name" {
                    let name = parsed.node_text(enum_child).to_string();
                    if !name.is_empty() {
                        values.push(name);
                    }
                    break;
                }
            }
        }
    }
    values
}

fn extract_variable(node: Node, parsed: &ParsedFile, scope: &Scope) -> Option<VarDecl> {
    let (name, name_start, name_end) = find_child_name_with_range(node, parsed)?;

    let type_annotation = node
        .child_by_field_name("type")
        .map(|n| parsed.node_text(n).to_string());

    let value_node = node.child_by_field_name("value").or_else(|| {
        // The grammar may not expose a "value" field. Fall back to finding
        // the last named child that isn't a keyword/name/type node.
        let count = node.child_count();
        let mut last_value = None;
        for i in 0..count {
            if let Some(child) = node.child(i) {
                if child.is_named() && !matches!(child.kind(), "name" | "type" | "inferred_type") {
                    last_value = Some(child);
                }
            }
        }
        last_value
    });
    let initializer_type = value_node.and_then(|v| infer_initializer_type(v, parsed));
    let initializer_call = value_node.and_then(|v| extract_call_target(v, parsed));

    let text = parsed.node_text(node);
    let is_onready = text.contains("@onready");
    let is_export = text.contains("@export");

    // Only extract documentation for file-scope variables
    let documentation = if *scope == Scope::File {
        extract_documentation(node, parsed)
    } else {
        None
    };

    Some(VarDecl {
        name,
        type_annotation,
        inferred_type: None,
        initializer_type,
        initializer_call,
        is_onready,
        is_export,
        scope: scope.clone(),
        line: node.start_position().row + 1,
        used: false,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        name_start_byte: name_start,
        name_end_byte: name_end,
        documentation,
    })
}

/// Infer the type of an initializer expression from its AST node.
fn infer_initializer_type(node: Node, parsed: &ParsedFile) -> Option<String> {
    match node.kind() {
        "integer" => Some("int".to_string()),
        "float" => Some("float".to_string()),
        "string" => Some("String".to_string()),
        "true" | "false" => Some("bool".to_string()),
        "array" => Some("Array".to_string()),
        "dictionary" => Some("Dictionary".to_string()),
        "null" => None, // null doesn't tell us a type
        "call" => {
            // Constructor calls: Vector3(), Node2D(), etc.
            // The function being called is the first child
            if let Some(func_node) = node.child(0) {
                let func_name = parsed.node_text(func_node);
                // Constructors start with uppercase (e.g., Vector3())
                if func_name.chars().next().is_some_and(|c| c.is_uppercase())
                    && !func_name.contains('.')
                {
                    return Some(func_name.to_string());
                }
            }
            None
        }
        "attribute" => {
            // ClassName.new() pattern: attribute node with an identifier and attribute_call
            let mut cursor = node.walk();
            let named_children: Vec<_> = node
                .children(&mut cursor)
                .filter(|c| c.is_named())
                .collect();
            if named_children.len() >= 2 {
                let receiver = named_children[0];
                let method = named_children[1];
                if (receiver.kind() == "identifier" || receiver.kind() == "name")
                    && method.kind() == "attribute_call"
                {
                    let receiver_name = parsed.node_text(receiver);
                    // Check if method is .new()
                    let mut method_cursor = method.walk();
                    let method_named: Vec<_> = method
                        .children(&mut method_cursor)
                        .filter(|c| c.is_named())
                        .collect();
                    if let Some(method_name_node) = method_named.first() {
                        let method_name = parsed.node_text(*method_name_node);
                        if method_name == "new"
                            && receiver_name
                                .chars()
                                .next()
                                .is_some_and(|c| c.is_uppercase())
                        {
                            return Some(receiver_name.to_string());
                        }
                    }
                }
            }
            None
        }
        "unary_operator" => {
            // -5 → int, -3.14 → float
            if let Some(operand) = node.child(1) {
                match operand.kind() {
                    "integer" => return Some("int".to_string()),
                    "float" => return Some("float".to_string()),
                    _ => {}
                }
            }
            None
        }
        _ => None,
    }
}

/// Extract the call target from an initializer expression.
/// Returns the function name for direct calls like `get_viewport()`, `my_func()`.
/// Returns None for constructors (handled by infer_initializer_type) and non-call expressions.
fn extract_call_target(node: Node, parsed: &ParsedFile) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    let func_node = node.child(0)?;
    let func_text = parsed.node_text(func_node);

    // Skip constructors (PascalCase) - already handled by infer_initializer_type
    if func_text.chars().next().is_some_and(|c| c.is_uppercase()) {
        return None;
    }

    // For simple function calls like `get_viewport()`, return the name
    if func_node.kind() == "identifier" || func_node.kind() == "name" {
        return Some(func_text.to_string());
    }

    // For method calls like `self.get_viewport()` or `obj.method()`,
    // extract just the method name if receiver is `self`
    if func_node.kind() == "attribute" {
        if let Some(obj) = func_node.child(0) {
            let obj_text = parsed.node_text(obj);
            if obj_text == "self" {
                if let Some(attr) = func_node.child_by_field_name("attribute") {
                    return Some(parsed.node_text(attr).to_string());
                }
            }
        }
    }

    None
}

/// Extract the res:// path from a `preload("res://...")` call in a statement's value.
fn extract_preload_path(node: Node, parsed: &ParsedFile) -> Option<String> {
    // Look for a `call` node in the statement's value
    let value_node = node.child_by_field_name("value")?;
    if value_node.kind() != "call" {
        return None;
    }
    let func_node = value_node.child(0)?;
    let func_name = parsed.node_text(func_node);
    if func_name != "preload" {
        return None;
    }
    // Find the string argument
    let mut cursor = value_node.walk();
    for child in value_node.children(&mut cursor) {
        if child.kind() == "arguments" {
            let mut arg_cursor = child.walk();
            for arg in child.children(&mut arg_cursor) {
                if arg.kind() == "string" {
                    let text = parsed.node_text(arg);
                    let unquoted = text.trim_matches('"').trim_matches('\'');
                    if unquoted.starts_with("res://") {
                        return Some(unquoted.to_string());
                    }
                }
            }
        }
    }
    None
}

fn extract_function(node: Node, parsed: &ParsedFile) -> Option<FuncDecl> {
    let (name, name_start, name_end) = find_child_name_with_range(node, parsed)?;

    let return_type = node
        .child_by_field_name("return_type")
        .map(|n| parsed.node_text(n).to_string());

    let documentation = extract_documentation(node, parsed);

    let mut parameters = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut cursor = params_node.walk();
        for child in params_node.children(&mut cursor) {
            match child.kind() {
                "typed_parameter" | "typed_default_parameter" => {
                    // Name is the first identifier child
                    let name_node = child.child(0);
                    let param_name = name_node
                        .map(|n| parsed.node_text(n).to_string())
                        .unwrap_or_default();
                    let param_type = child
                        .child_by_field_name("type")
                        .or_else(|| {
                            // Find child of kind "type"
                            for i in 0..child.child_count() {
                                if let Some(c) = child.child(i) {
                                    if c.kind() == "type" {
                                        return Some(c);
                                    }
                                }
                            }
                            None
                        })
                        .map(|n| parsed.node_text(n).to_string());
                    if !param_name.is_empty() {
                        let (name_start_byte, name_end_byte) = name_node
                            .map(|n| (n.start_byte(), n.end_byte()))
                            .unwrap_or((0, 0));
                        parameters.push(ParamDecl {
                            name: param_name,
                            type_annotation: param_type,
                            inferred_type: None,
                            line: child.start_position().row + 1,
                            used: false,
                            start_byte: child.start_byte(),
                            end_byte: child.end_byte(),
                            name_start_byte,
                            name_end_byte,
                        });
                    }
                }
                "identifier" => {
                    // Untyped parameter: just a bare identifier
                    let param_name = parsed.node_text(child).to_string();
                    parameters.push(ParamDecl {
                        name: param_name,
                        type_annotation: None,
                        inferred_type: None,
                        line: child.start_position().row + 1,
                        used: false,
                        start_byte: child.start_byte(),
                        end_byte: child.end_byte(),
                        name_start_byte: child.start_byte(),
                        name_end_byte: child.end_byte(),
                    });
                }
                "default_parameter" | "inferred_parameter" => {
                    // Parameter with default value: `x = 5`
                    // Try to infer type from the default value
                    let name_node = child.child(0);
                    let param_name = name_node
                        .map(|n| parsed.node_text(n).to_string())
                        .unwrap_or_default();
                    let (name_start_byte, name_end_byte) = name_node
                        .map(|n| (n.start_byte(), n.end_byte()))
                        .unwrap_or((0, 0));

                    // Find the default value and infer its type
                    let inferred_type = child
                        .child_by_field_name("value")
                        .or_else(|| {
                            // Find the value expression (skip name and '=')
                            let count = child.child_count();
                            for i in 0..count {
                                if let Some(c) = child.child(i) {
                                    if c.kind() != "identifier"
                                        && c.kind() != "name"
                                        && c.kind() != "="
                                        && c.is_named()
                                    {
                                        return Some(c);
                                    }
                                }
                            }
                            None
                        })
                        .and_then(|v| infer_initializer_type(v, parsed));

                    if !param_name.is_empty() {
                        parameters.push(ParamDecl {
                            name: param_name,
                            type_annotation: None,
                            inferred_type,
                            line: child.start_position().row + 1,
                            used: false,
                            start_byte: child.start_byte(),
                            end_byte: child.end_byte(),
                            name_start_byte,
                            name_end_byte,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    // Collect local variables from the function body
    let mut local_vars = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        collect_local_vars(body, parsed, &name, &mut local_vars);
    }

    Some(FuncDecl {
        name: name.clone(),
        parameters,
        return_type,
        inferred_return_type: None,
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        local_vars,
        used: false,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        name_start_byte: name_start,
        name_end_byte: name_end,
        documentation,
    })
}

fn collect_local_vars(node: Node, parsed: &ParsedFile, func_name: &str, vars: &mut Vec<VarDecl>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_statement" {
            if let Some(v) =
                extract_variable(child, parsed, &Scope::Function(func_name.to_string()))
            {
                vars.push(v);
            }
        }
        // Recurse into nested blocks
        collect_local_vars(child, parsed, func_name, vars);
    }
}

fn find_inner_class_extends(node: Node, parsed: &ParsedFile) -> Option<String> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "extends_statement" {
            let text = parsed.node_text(child);
            let value = text.trim_start_matches("extends").trim();
            return Some(value.to_string());
        }
    }
    None
}

/// Mark symbols as used by scanning for identifier references.
/// Uses scope-aware analysis to avoid false positives from shadowed names.
fn mark_used_symbols(parsed: &ParsedFile, symbols: &mut FileSymbols) {
    let root = parsed.root_node();

    // Build a map of function name -> set of names that shadow members (params + locals)
    let mut func_shadows: HashMap<String, HashSet<String>> = HashMap::new();
    for func in &symbols.functions {
        let mut shadows = HashSet::new();
        for param in &func.parameters {
            shadows.insert(param.name.clone());
        }
        for var in &func.local_vars {
            shadows.insert(var.name.clone());
        }
        func_shadows.insert(func.name.clone(), shadows);
    }

    // Find all function definition nodes to map positions to function names
    let func_nodes = parser::find_nodes_by_kind(root, "function_definition");
    let func_ranges: Vec<(String, usize, usize)> = func_nodes
        .iter()
        .filter_map(|node| {
            let name_node = node.child_by_field_name("name")?;
            let name = parsed.node_text(name_node).to_string();
            Some((name, node.start_byte(), node.end_byte()))
        })
        .collect();

    // Helper to find which function (if any) contains a byte position
    let find_containing_func = |byte_pos: usize| -> Option<&str> {
        func_ranges
            .iter()
            .find(|(_, start, end)| byte_pos >= *start && byte_pos < *end)
            .map(|(name, _, _)| name.as_str())
    };

    // Collect all identifier nodes with their positions
    let identifiers = parser::find_nodes_by_kind(root, "identifier");

    // Count unshadowed references to member-level symbols
    // A reference is shadowed if it's inside a function that has a local/param with the same name
    let count_unshadowed_refs = |name: &str| -> usize {
        identifiers
            .iter()
            .filter(|node| {
                if parsed.node_text(**node) != name {
                    return false;
                }
                // Check if this reference is shadowed by a local
                if let Some(func_name) = find_containing_func(node.start_byte()) {
                    if let Some(shadows) = func_shadows.get(func_name) {
                        if shadows.contains(name) {
                            return false; // Shadowed by local/param
                        }
                    }
                }
                true
            })
            .count()
    };

    // Mark signals used if referenced (unshadowed) anywhere
    // We only count identifier nodes (actual references), not name nodes (declarations)
    for sig in &mut symbols.signals {
        sig.used = count_unshadowed_refs(&sig.name) > 0;
    }

    // Mark member variables used (unshadowed references)
    for var in &mut symbols.variables {
        var.used = count_unshadowed_refs(&var.name) > 0;
    }

    // Mark functions used if referenced anywhere
    for func in &mut symbols.functions {
        func.used = count_unshadowed_refs(&func.name) > 0;
    }

    // Mark function parameters and local vars used (within function body)
    for func in &mut symbols.functions {
        let body_identifiers = if let Some(body_node) = find_function_body(parsed, &func.name) {
            let ids = parser::find_nodes_by_kind(body_node, "identifier");
            ids.iter()
                .map(|n| parsed.node_text(*n).to_string())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        for param in &mut func.parameters {
            param.used = body_identifiers.contains(&param.name);
        }

        // Mark local vars used - declarations are `name` nodes, so any
        // matching `identifier` in the body is a genuine use.
        for var in &mut func.local_vars {
            var.used = body_identifiers.contains(&var.name);
        }
    }
}

fn find_function_body<'a>(parsed: &'a ParsedFile, func_name: &str) -> Option<Node<'a>> {
    let funcs = parser::find_nodes_by_kind(parsed.root_node(), "function_definition");
    for func in funcs {
        if let Some(name_node) = func.child_by_field_name("name") {
            if parsed.node_text(name_node) == func_name {
                return func.child_by_field_name("body");
            }
        }
    }
    None
}

/// Resolve cross-file references: extends chains, class_name lookups.
pub fn resolve_cross_file(file_symbols: &mut [FileSymbols], project_info: &ProjectInfo) {
    // Build a map of class_name -> file index
    let class_map: HashMap<String, usize> = file_symbols
        .iter()
        .enumerate()
        .filter_map(|(i, fs)| fs.class_name.as_ref().map(|cn| (cn.clone(), i)))
        .collect();

    // Build a map of file path (res:// relative) -> index
    let _path_map: HashMap<String, usize> = file_symbols
        .iter()
        .enumerate()
        .map(|(i, fs)| (fs.path.display().to_string(), i))
        .collect();

    // Resolve extends references
    for i in 0..file_symbols.len() {
        if let Some(ref extends) = file_symbols[i].extends.clone() {
            // Check if extends refers to a class_name
            if let Some(&parent_idx) = class_map.get(extends) {
                let parent_path = file_symbols[parent_idx].path.clone();
                file_symbols[i].parent_file = Some(parent_path);
            }
            // Check if it's a preload path (res://...)
            if extends.starts_with("res://") || extends.contains(".gd") {
                // Would need to resolve the actual file path
            }
        }
    }

    // Inject autoload singleton names as known globals in every file
    let autoload_names: HashSet<String> = project_info.autoloads.keys().cloned().collect();
    for fs in file_symbols.iter_mut() {
        fs.autoloads = autoload_names.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectInfo;

    #[test]
    fn autoload_injection_populates_all_files() {
        let mut project_info = ProjectInfo::default();
        project_info.autoloads.insert(
            "GameManager".to_string(),
            "res://scripts/GameManager.gd".to_string(),
        );
        project_info.autoloads.insert(
            "EventBus".to_string(),
            "res://scripts/EventBus.gd".to_string(),
        );

        let mut file_symbols = vec![
            FileSymbols {
                path: PathBuf::from("a.gd"),
                class_name: None,
                extends: None,
                signals: Vec::new(),
                enums: Vec::new(),
                constants: Vec::new(),
                variables: Vec::new(),
                functions: Vec::new(),
                inner_classes: Vec::new(),
                parent_file: None,
                autoloads: HashSet::new(),
                preloads: Vec::new(),
            },
            FileSymbols {
                path: PathBuf::from("b.gd"),
                class_name: None,
                extends: None,
                signals: Vec::new(),
                enums: Vec::new(),
                constants: Vec::new(),
                variables: Vec::new(),
                functions: Vec::new(),
                inner_classes: Vec::new(),
                parent_file: None,
                autoloads: HashSet::new(),
                preloads: Vec::new(),
            },
        ];

        resolve_cross_file(&mut file_symbols, &project_info);

        for fs in &file_symbols {
            assert_eq!(fs.autoloads.len(), 2);
            assert!(fs.autoloads.contains(&"GameManager".to_string()));
            assert!(fs.autoloads.contains(&"EventBus".to_string()));
        }
    }

    #[test]
    fn autoload_injection_empty_project() {
        let project_info = ProjectInfo::default();
        let mut file_symbols = vec![FileSymbols {
            path: PathBuf::from("a.gd"),
            class_name: None,
            extends: None,
            signals: Vec::new(),
            enums: Vec::new(),
            constants: Vec::new(),
            variables: Vec::new(),
            functions: Vec::new(),
            inner_classes: Vec::new(),
            parent_file: None,
            autoloads: HashSet::new(),
            preloads: Vec::new(),
        }];

        resolve_cross_file(&mut file_symbols, &project_info);
        assert!(file_symbols[0].autoloads.is_empty());
    }
}
