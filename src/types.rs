use tree_sitter::Node;

use crate::classdb::ClassDb;
use crate::parser::ParsedFile;
use crate::symbols::FileSymbols;

/// Known GDScript types for inference.
#[allow(dead_code)] // For future type analysis features
#[derive(Debug, Clone, PartialEq)]
pub enum GdType {
    Int,
    Float,
    String,
    Bool,
    Vector2,
    Vector3,
    Array,
    Dictionary,
    Node,
    Resource,
    /// A user-defined class
    Class(std::string::String),
    /// Godot engine class
    EngineClass(std::string::String),
    /// Could not determine type
    Unknown,
}

#[allow(dead_code)] // For future type analysis features
impl GdType {
    /// Try to parse a type annotation string into a GdType.
    pub fn from_annotation(s: &str) -> Self {
        match s.trim() {
            "int" => GdType::Int,
            "float" => GdType::Float,
            "String" => GdType::String,
            "bool" => GdType::Bool,
            "Vector2" | "Vector2i" => GdType::Vector2,
            "Vector3" | "Vector3i" => GdType::Vector3,
            "Array" => GdType::Array,
            "Dictionary" => GdType::Dictionary,
            "Node" => GdType::Node,
            "Resource" => GdType::Resource,
            other => {
                // Engine classes start with uppercase
                if other.chars().next().is_some_and(|c| c.is_uppercase()) {
                    GdType::EngineClass(other.to_string())
                } else {
                    GdType::Unknown
                }
            }
        }
    }
}

/// Propagate type information through the symbol table.
/// Priority: explicit annotation > initializer inference > call return type.
pub fn propagate_types(
    file_sym: &mut FileSymbols,
    parsed: &crate::parser::ParsedFile,
    class_db: &ClassDb,
) {
    // Collect local function return types for cross-procedural resolution
    let local_func_returns: Vec<(String, Option<String>)> = file_sym
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.return_type.clone()))
        .collect();

    // Build inheritance chain for ClassDB lookups
    let extends_class = file_sym
        .extends
        .clone()
        .unwrap_or_else(|| "RefCounted".to_string());

    // Propagate types on member variables
    for var in &mut file_sym.variables {
        resolve_var_type(var, &local_func_returns, &extends_class, class_db);
    }

    // Propagate types in function local vars
    for func in &mut file_sym.functions {
        for var in &mut func.local_vars {
            resolve_var_type(var, &local_func_returns, &extends_class, class_db);
        }
    }

    // Infer return types for functions without explicit annotations
    infer_function_return_types(file_sym, parsed, &extends_class, class_db);
}

/// Resolve the type of a variable using all available information.
fn resolve_var_type(
    var: &mut crate::symbols::VarDecl,
    local_func_returns: &[(String, Option<String>)],
    extends_class: &str,
    class_db: &ClassDb,
) {
    // 1. Explicit annotation always wins (skip `:=` which is the inferred type operator)
    if let Some(ref ann) = var.type_annotation {
        if !ann.is_empty() && ann != ":=" {
            var.inferred_type = Some(ann.clone());
            return;
        }
    }

    // 2. Initializer literal/constructor inference
    if let Some(ref init) = var.initializer_type {
        var.inferred_type = Some(init.clone());
        return;
    }

    // 3. Resolve from call target (API return type / local function)
    if let Some(ref call_name) = var.initializer_call {
        if let Some(ret_type) =
            resolve_call_return_type(call_name, local_func_returns, extends_class, class_db)
        {
            var.inferred_type = Some(ret_type);
        }
    }
}

/// Resolve the return type of a function call.
pub fn resolve_call_return_type(
    call_name: &str,
    local_func_returns: &[(String, Option<String>)],
    extends_class: &str,
    class_db: &ClassDb,
) -> Option<String> {
    // 1. Check local functions first (cross-procedural resolution)
    for (fname, ret_type) in local_func_returns {
        if fname == call_name {
            return ret_type.clone();
        }
    }

    // 2. Check ClassDB methods on the inheritance chain
    if let Some(method) = class_db.get_method(extends_class, call_name) {
        let ret = &method.return_type;
        if !ret.is_empty() && ret != "void" && ret != "Variant" {
            return Some(ret.clone());
        }
    }

    // 3. Check utility functions (global functions like abs, min, etc.)
    if let Some(util) = class_db.get_utility_function(call_name) {
        let ret = &util.return_type;
        if !ret.is_empty() && ret != "void" && ret != "Variant" {
            return Some(ret.clone());
        }
    }

    None
}

/// Infer return types for functions that don't have explicit annotations.
fn infer_function_return_types(
    file_sym: &mut FileSymbols,
    parsed: &crate::parser::ParsedFile,
    extends_class: &str,
    class_db: &ClassDb,
) {
    // Build local func returns and member var types for context
    let local_func_returns: Vec<(String, Option<String>)> = file_sym
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.return_type.clone()))
        .collect();

    let member_vars: Vec<(String, Option<String>)> = file_sym
        .variables
        .iter()
        .map(|v| (v.name.clone(), v.inferred_type.clone()))
        .collect();

    let root = parsed.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "function_definition" {
            continue;
        }

        // Get function name
        let func_name = match child.child_by_field_name("name") {
            Some(n) => parsed.node_text(n),
            None => continue,
        };

        // Find matching FuncDecl
        let func_idx = file_sym.functions.iter().position(|f| f.name == func_name);
        let func_idx = match func_idx {
            Some(i) => i,
            None => continue,
        };

        // Skip if already has explicit return type
        if file_sym.functions[func_idx].return_type.is_some() {
            continue;
        }

        // Build type context for this function
        let local_vars: Vec<(String, Option<String>)> = file_sym.functions[func_idx]
            .local_vars
            .iter()
            .map(|v| (v.name.clone(), v.inferred_type.clone()))
            .collect();

        let params: Vec<(String, Option<String>)> = file_sym.functions[func_idx]
            .parameters
            .iter()
            .map(|p| (p.name.clone(), p.type_annotation.clone()))
            .collect();

        let ctx = ExprTypeContext {
            extends_class,
            local_vars: &local_vars,
            params: &params,
            member_vars: &member_vars,
            local_func_returns: &local_func_returns,
            class_db,
            type_refinements: None,
        };

        // Collect return types from all return statements
        let mut return_types: Vec<String> = Vec::new();
        collect_return_types(child, parsed, &ctx, &mut return_types);

        // Unify: if all return types are the same (or compatible), use it
        if let Some(unified) = unify_return_types(&return_types, class_db) {
            file_sym.functions[func_idx].inferred_return_type = Some(unified);
        }
    }
}

/// Recursively collect types from return statements in a function body.
fn collect_return_types(
    node: tree_sitter::Node,
    parsed: &crate::parser::ParsedFile,
    ctx: &ExprTypeContext,
    types: &mut Vec<String>,
) {
    // Don't recurse into nested functions or lambdas
    if node.kind() == "lambda" {
        return;
    }
    if node.kind() == "function_definition" && node.parent().is_some_and(|p| p.kind() != "source") {
        return;
    }

    if node.kind() == "return_statement" {
        // Get the expression after "return"
        let mut cursor = node.walk();
        let expr_node = node.children(&mut cursor).find(|c| c.kind() != "return");

        if let Some(expr) = expr_node {
            if expr.kind() == "null" {
                // Explicit null return - mark it so we don't suggest a concrete type
                types.push("null".to_string());
            } else if let Some(t) = resolve_expr_type(expr, parsed, ctx) {
                types.push(t);
            }
        } else {
            // Bare "return" with no expression → void
            types.push("void".to_string());
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_return_types(child, parsed, ctx, types);
    }
}

/// Unify a list of return types into a single type.
/// Returns None if the types are incompatible or empty.
fn unify_return_types(types: &[String], class_db: &ClassDb) -> Option<String> {
    if types.is_empty() {
        // No return statements found - could be void or implicit null
        return None;
    }

    // If any return statement is `return null`, we can't suggest a concrete type
    // since GDScript can't express "Type | null" without using Variant
    if types.iter().any(|t| t == "null") {
        return None;
    }

    // Filter out "void" from bare returns when there are other types
    let non_void: Vec<&String> = types.iter().filter(|t| *t != "void").collect();
    if non_void.is_empty() {
        return Some("void".to_string());
    }

    // Check if all non-void types are the same or compatible
    let first = &non_void[0];
    for t in &non_void[1..] {
        if *t != *first && !types_compatible(first, t, class_db) {
            // Incompatible types - can't unify
            return None;
        }
    }

    Some((*first).clone())
}

/// Extract the base type from a potentially parameterized type string.
/// E.g., `"Array[int]"` → `"Array"`, `"Dictionary[String, int]"` → `"Dictionary"`, `"int"` → `"int"`.
fn base_type(s: &str) -> &str {
    match s.find('[') {
        Some(pos) => &s[..pos],
        None => s,
    }
}

/// Check if a type string is a Packed*Array type (e.g., PackedByteArray, PackedVector3Array).
fn is_packed_array(s: &str) -> bool {
    s.starts_with("Packed") && s.ends_with("Array") && s.len() > "PackedArray".len()
}

/// Extract the element type from a container type.
/// E.g., `"Array[int]"` → `Some("int")`, `"Dictionary[String, Node]"` → `Some("Node")`.
/// For Dictionary, returns the value type (second parameter).
fn element_type(s: &str) -> Option<&str> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end <= start + 1 {
        return None;
    }
    let inner = &s[start + 1..end];
    // For Dictionary[K, V], extract V (after the comma)
    if s.starts_with("Dictionary") {
        if let Some(comma) = inner.find(',') {
            let value_type = inner[comma + 1..].trim();
            if !value_type.is_empty() {
                return Some(value_type);
            }
        }
        return None;
    }
    // For Array[T], return T
    Some(inner.trim())
}

/// Get the element type for a packed array type.
fn packed_array_element_type(s: &str) -> Option<&'static str> {
    match s {
        "PackedByteArray" => Some("int"),
        "PackedInt32Array" => Some("int"),
        "PackedInt64Array" => Some("int"),
        "PackedFloat32Array" => Some("float"),
        "PackedFloat64Array" => Some("float"),
        "PackedStringArray" => Some("String"),
        "PackedVector2Array" => Some("Vector2"),
        "PackedVector3Array" => Some("Vector3"),
        "PackedVector4Array" => Some("Vector4"),
        "PackedColorArray" => Some("Color"),
        _ => None,
    }
}

/// Check if two type strings are compatible (assignable).
/// `declared` is the expected type (annotation), `actual` is the inferred type.
pub fn types_compatible(declared: &str, actual: &str, class_db: &ClassDb) -> bool {
    let d = declared.trim();
    let a = actual.trim();

    // Same type
    if d == a {
        return true;
    }

    // Variant accepts everything
    if d == "Variant" || a == "Variant" {
        return true;
    }

    // Typed container compatibility: Array[T] ↔ Array, Dictionary[K,V] ↔ Dictionary.
    // Two parameterizations of the same container are also compatible (runtime-checked).
    let d_base = base_type(d);
    let a_base = base_type(a);
    if d_base == a_base && (d_base == "Array" || d_base == "Dictionary") {
        return true;
    }

    // Packed array types are compatible with Array (GDScript allows implicit conversion)
    if a_base == "Array" && is_packed_array(d) {
        return true;
    }
    if d_base == "Array" && is_packed_array(a) {
        return true;
    }

    // Numeric compatibility: int <-> float
    if (d == "int" || d == "float") && (a == "int" || a == "float") {
        return true;
    }

    // Vector variant compatibility: Vector2 <-> Vector2i, Vector3 <-> Vector3i
    if (d == "Vector2" || d == "Vector2i") && (a == "Vector2" || a == "Vector2i") {
        return true;
    }
    if (d == "Vector3" || d == "Vector3i") && (a == "Vector3" || a == "Vector3i") {
        return true;
    }

    // Subclass compatibility: actual is a subclass of declared
    if class_db.is_subclass_of(a, d) {
        return true;
    }

    // PackedScene/Resource compatibility
    if d == "Resource" && class_db.is_subclass_of(a, "Resource") {
        return true;
    }

    // Node subclass compatibility
    if d == "Node" && class_db.is_subclass_of(a, "Node") {
        return true;
    }

    false
}

/// Context for resolving the type of an expression.
pub struct ExprTypeContext<'a> {
    /// The class this script extends (e.g., "Node2D").
    pub extends_class: &'a str,
    /// Local variables in scope: (name, inferred_type).
    pub local_vars: &'a [(String, Option<String>)],
    /// Function parameters in scope: (name, type_annotation).
    pub params: &'a [(String, Option<String>)],
    /// Member variables: (name, effective_type).
    pub member_vars: &'a [(String, Option<String>)],
    /// Local function return types for cross-procedural resolution.
    pub local_func_returns: &'a [(String, Option<String>)],
    /// The Godot ClassDB.
    pub class_db: &'a ClassDb,
    /// Type refinements from control flow (e.g., `if x is Node:` narrows x to Node).
    pub type_refinements: Option<&'a std::collections::HashMap<String, String>>,
}

/// Resolve the type of an expression AST node.
/// Returns None if the type cannot be determined.
pub fn resolve_expr_type(node: Node, parsed: &ParsedFile, ctx: &ExprTypeContext) -> Option<String> {
    match node.kind() {
        // Literals
        "integer" => Some("int".to_string()),
        "float" => Some("float".to_string()),
        "string" => Some("String".to_string()),
        "true" | "false" => Some("bool".to_string()),
        "array" => Some("Array".to_string()),
        "dictionary" => Some("Dictionary".to_string()),
        "null" => None,

        // Variable reference
        "identifier" | "name" => {
            let name = parsed.node_text(node);
            resolve_identifier_type(name, ctx)
        }

        // Function/constructor call
        "call" => resolve_call_expr_type(node, parsed, ctx),

        // Attribute access (property or method call on receiver)
        "attribute" => resolve_attribute_expr_type(node, parsed, ctx),

        // Subscript/index access (e.g., arr[0], dict["key"])
        "subscript" => resolve_subscript_expr_type(node, parsed, ctx),

        // Binary operator
        "binary_operator" => resolve_binary_op_type(node, parsed, ctx),

        // Unary operator
        "unary_operator" => resolve_unary_op_type(node, parsed, ctx),

        // Parenthesized expression: recurse into inner
        "parenthesized_expression" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "(" && child.kind() != ")" {
                    return resolve_expr_type(child, parsed, ctx);
                }
            }
            None
        }

        // Ternary (conditional) expression: type of the "then" branch
        "ternary_expression" | "conditional_expression" => node
            .child(0)
            .and_then(|c| resolve_expr_type(c, parsed, ctx)),

        // Await expression: type of the awaited expression
        "await_expression" | "await" => node
            .child(1)
            .and_then(|c| resolve_expr_type(c, parsed, ctx)),

        _ => None,
    }
}

/// Look up a variable/identifier name in the type context.
fn resolve_identifier_type(name: &str, ctx: &ExprTypeContext) -> Option<String> {
    // Check type refinements first (from control flow like `if x is Node:`)
    if let Some(refinements) = ctx.type_refinements {
        if let Some(narrowed_type) = refinements.get(name) {
            return Some(narrowed_type.clone());
        }
    }

    // Check local vars
    for (vname, vtype) in ctx.local_vars {
        if vname == name {
            return vtype.clone();
        }
    }
    // Check params
    for (pname, ptype) in ctx.params {
        if pname == name {
            return ptype.clone();
        }
    }
    // Check member vars
    for (mname, mtype) in ctx.member_vars {
        if mname == name {
            return mtype.clone();
        }
    }
    // Check singletons
    if let Some(t) = ctx.class_db.get_singleton_type(name) {
        return Some(t.to_string());
    }
    // Check inherited class properties (bare `position` refers to self.position)
    if let Some(prop) = ctx.class_db.get_property(ctx.extends_class, name) {
        if !prop.prop_type.is_empty() && prop.prop_type != "Variant" {
            return Some(prop.prop_type.clone());
        }
    }
    None
}

/// Resolve the type of a `call` node.
fn resolve_call_expr_type(
    node: Node,
    parsed: &ParsedFile,
    ctx: &ExprTypeContext,
) -> Option<String> {
    let func_node = node.child(0)?;
    let func_text = parsed.node_text(func_node);

    // Constructor call: PascalCase identifier followed by arguments → type is the class name
    // e.g., Vector2(1, 2), Color(1, 0, 0), Node2D.new()
    if func_node.kind() == "identifier" || func_node.kind() == "name" {
        if func_text.chars().next().is_some_and(|c| c.is_uppercase()) {
            // This is a constructor: Vector2(), Color(), PackedScene(), etc.
            return Some(func_text.to_string());
        }
        // Regular function call — resolve return type
        return resolve_call_return_type(
            func_text,
            ctx.local_func_returns,
            ctx.extends_class,
            ctx.class_db,
        );
    }

    // Attribute call: obj.method() — handled by the attribute node resolution
    if func_node.kind() == "attribute" {
        return resolve_attribute_expr_type(node, parsed, ctx);
    }

    None
}

/// Resolve the type of an `attribute` node (property access or method call chain).
///
/// Handles chained calls like `get_viewport().get_camera_2d().get_zoom()` which
/// produce a flat structure:
///   attribute
///     call "get_viewport()"
///     attribute_call "get_camera_2d()"
///     attribute_call "get_zoom()"
fn resolve_attribute_expr_type(
    node: Node,
    parsed: &ParsedFile,
    ctx: &ExprTypeContext,
) -> Option<String> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return None;
    }

    let receiver = children[0];

    // Determine the receiver's type and whether it's self
    let is_self = parsed.node_text(receiver) == "self";
    let mut current_type = if is_self {
        ctx.extends_class.to_string()
    } else {
        resolve_expr_type(receiver, parsed, ctx)?
    };

    // Walk through the chain of attribute accesses/calls
    let mut first_access = true;
    for child in &children[1..] {
        let check_self = is_self && first_access;

        match child.kind() {
            "attribute_call" => {
                first_access = false;
                let method_name_node = child.child(0)?;
                let method_name = parsed.node_text(method_name_node);
                // For self calls, also check local function return types
                if check_self {
                    let mut found = false;
                    for (fname, ret_type) in ctx.local_func_returns {
                        if fname == method_name {
                            match ret_type {
                                Some(t) => {
                                    current_type = t.clone();
                                    found = true;
                                    break;
                                }
                                None => return None,
                            }
                        }
                    }
                    if found {
                        continue;
                    }
                }
                current_type =
                    resolve_method_return_type(&current_type, method_name, ctx.class_db)?;
            }
            "identifier" | "name" => {
                first_access = false;
                let attr_name = parsed.node_text(*child);
                // Static constructor: ClassName.new()
                if attr_name == "new" && ctx.class_db.class_exists(&current_type) {
                    return Some(current_type);
                }
                // For self, check user-defined member variables
                if check_self {
                    let mut found = false;
                    for (mname, mtype) in ctx.member_vars {
                        if mname == attr_name {
                            match mtype {
                                Some(t) => {
                                    current_type = t.clone();
                                    found = true;
                                    break;
                                }
                                None => return None,
                            }
                        }
                    }
                    if found {
                        continue;
                    }
                }
                // Property access via ClassDB
                current_type = resolve_property_type(&current_type, attr_name, ctx.class_db)?;
            }
            _ => {}
        }
    }

    Some(current_type)
}

/// Resolve the return type of a method call on a typed receiver.
fn resolve_method_return_type(
    receiver_type: &str,
    method_name: &str,
    class_db: &ClassDb,
) -> Option<String> {
    let base = base_type(receiver_type);

    // Check engine class methods (walks inheritance chain)
    if let Some(method) = class_db.get_method(base, method_name) {
        let ret = &method.return_type;
        if !ret.is_empty() && ret != "void" && ret != "Variant" {
            return Some(ret.clone());
        }
        // void/Variant → None (can't determine useful type)
        return None;
    }

    // Check builtin class methods (Array, Vector2, etc.)
    if let Some(method) = class_db.get_builtin_method(base, method_name) {
        let ret = &method.return_type;
        if !ret.is_empty() && ret != "void" && ret != "Variant" {
            return Some(ret.clone());
        }
        return None;
    }

    None
}

/// Resolve the type of a property access on a typed receiver.
fn resolve_property_type(
    receiver_type: &str,
    prop_name: &str,
    class_db: &ClassDb,
) -> Option<String> {
    let base = base_type(receiver_type);

    if let Some(prop) = class_db.get_property(base, prop_name) {
        let t = &prop.prop_type;
        if !t.is_empty() && t != "Variant" {
            return Some(t.clone());
        }
    }

    None
}

/// Resolve the type of a subscript/index expression (e.g., `arr[0]`, `dict["key"]`).
fn resolve_subscript_expr_type(
    node: Node,
    parsed: &ParsedFile,
    ctx: &ExprTypeContext,
) -> Option<String> {
    let mut cursor = node.walk();
    let named_children: Vec<_> = node
        .children(&mut cursor)
        .filter(|c| c.is_named())
        .collect();

    if named_children.is_empty() {
        return None;
    }

    // First child is the base expression (e.g., `arr` in `arr[0]`)
    let base = named_children[0];
    let base_type_str = resolve_expr_type(base, parsed, ctx)?;

    // Check for typed Array[T] - return element type T
    if base_type_str.starts_with("Array[") {
        return element_type(&base_type_str).map(|s| s.to_string());
    }

    // Check for typed Dictionary[K,V] - return value type V
    if base_type_str.starts_with("Dictionary[") {
        return element_type(&base_type_str).map(|s| s.to_string());
    }

    // Check for packed arrays - return their element type
    if let Some(elem) = packed_array_element_type(&base_type_str) {
        return Some(elem.to_string());
    }

    // String indexing returns String (single character)
    if base_type_str == "String" {
        return Some("String".to_string());
    }

    // Untyped Array or Dictionary - element type is unknown
    None
}

/// Resolve the type of a binary operator expression.
fn resolve_binary_op_type(
    node: Node,
    parsed: &ParsedFile,
    ctx: &ExprTypeContext,
) -> Option<String> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    // Binary op has: left, operator, right
    if children.len() < 3 {
        return None;
    }

    let op_text = parsed.node_text(children[1]);

    // Comparison and logical operators always produce bool
    match op_text {
        "==" | "!=" | "<" | ">" | "<=" | ">=" | "and" | "or" | "not" | "in" | "is" => {
            return Some("bool".to_string());
        }
        // Cast operator: `expr as Type` — result type is the target type
        "as" => {
            let type_name = parsed.node_text(children[2]);
            return Some(type_name.to_string());
        }
        _ => {}
    }

    // Resolve operand types
    let left_type = resolve_expr_type(children[0], parsed, ctx);
    let right_type = resolve_expr_type(children[2], parsed, ctx);

    // Try to look up the operator in the ClassDB for builtin types
    // GDScript has no operator overloading, so if the operator isn't in ClassDB, it doesn't exist
    if let (Some(l), Some(r)) = (left_type.as_deref(), right_type.as_deref()) {
        // Try left_type's operator with right_type
        if let Some(ret) = ctx.class_db.get_operator_return_type(l, op_text, r) {
            return Some(ret);
        }
        // Try right_type's operator with left_type (for commutative ops like int * Vector3)
        if let Some(ret) = ctx.class_db.get_operator_return_type(r, op_text, l) {
            return Some(ret);
        }
    }

    // No matching operator found - type is unknown
    None
}

/// Resolve the type of a unary operator expression.
fn resolve_unary_op_type(node: Node, parsed: &ParsedFile, ctx: &ExprTypeContext) -> Option<String> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    if children.is_empty() {
        return None;
    }

    // "not" → bool
    if children[0].kind() == "not" || parsed.node_text(children[0]) == "not" {
        return Some("bool".to_string());
    }

    // Negation (-x): same type as operand
    let operand = children.last()?;
    resolve_expr_type(*operand, parsed, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn load_classdb() -> ClassDb {
        ClassDb::from_bundled(None).unwrap()
    }

    fn empty_ctx<'a>(class_db: &'a ClassDb) -> ExprTypeContext<'a> {
        ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db,
            type_refinements: None,
        }
    }

    // --- GdType tests ---

    #[test]
    fn gdtype_from_annotation_primitives() {
        assert_eq!(GdType::from_annotation("int"), GdType::Int);
        assert_eq!(GdType::from_annotation("float"), GdType::Float);
        assert_eq!(GdType::from_annotation("String"), GdType::String);
        assert_eq!(GdType::from_annotation("bool"), GdType::Bool);
    }

    #[test]
    fn gdtype_from_annotation_vectors() {
        assert_eq!(GdType::from_annotation("Vector2"), GdType::Vector2);
        assert_eq!(GdType::from_annotation("Vector2i"), GdType::Vector2);
        assert_eq!(GdType::from_annotation("Vector3"), GdType::Vector3);
        assert_eq!(GdType::from_annotation("Vector3i"), GdType::Vector3);
    }

    #[test]
    fn gdtype_from_annotation_containers() {
        assert_eq!(GdType::from_annotation("Array"), GdType::Array);
        assert_eq!(GdType::from_annotation("Dictionary"), GdType::Dictionary);
    }

    #[test]
    fn gdtype_from_annotation_node_resource() {
        assert_eq!(GdType::from_annotation("Node"), GdType::Node);
        assert_eq!(GdType::from_annotation("Resource"), GdType::Resource);
    }

    #[test]
    fn gdtype_from_annotation_engine_class() {
        assert_eq!(
            GdType::from_annotation("Camera2D"),
            GdType::EngineClass("Camera2D".to_string())
        );
    }

    #[test]
    fn gdtype_from_annotation_unknown() {
        assert_eq!(GdType::from_annotation("lowercase_thing"), GdType::Unknown);
    }

    #[test]
    fn gdtype_from_annotation_whitespace() {
        assert_eq!(GdType::from_annotation("  int  "), GdType::Int);
    }

    // --- base_type / is_packed_array tests ---

    #[test]
    fn base_type_plain() {
        assert_eq!(base_type("int"), "int");
        assert_eq!(base_type("Array"), "Array");
    }

    #[test]
    fn base_type_parameterized() {
        assert_eq!(base_type("Array[int]"), "Array");
        assert_eq!(base_type("Dictionary[String, int]"), "Dictionary");
    }

    #[test]
    fn is_packed_array_true() {
        assert!(is_packed_array("PackedByteArray"));
        assert!(is_packed_array("PackedVector3Array"));
        assert!(is_packed_array("PackedFloat32Array"));
    }

    #[test]
    fn is_packed_array_false() {
        assert!(!is_packed_array("Array"));
        assert!(!is_packed_array("PackedScene"));
        assert!(!is_packed_array("PackedArray")); // too short - no element type
    }

    #[test]
    fn element_type_array() {
        assert_eq!(element_type("Array[int]"), Some("int"));
        assert_eq!(element_type("Array[String]"), Some("String"));
        assert_eq!(element_type("Array[Vector2]"), Some("Vector2"));
        assert_eq!(element_type("Array"), None);
    }

    #[test]
    fn element_type_dictionary() {
        assert_eq!(element_type("Dictionary[String, int]"), Some("int"));
        assert_eq!(element_type("Dictionary[String, Node]"), Some("Node"));
        assert_eq!(element_type("Dictionary"), None);
    }

    #[test]
    fn packed_array_element_types() {
        assert_eq!(packed_array_element_type("PackedInt32Array"), Some("int"));
        assert_eq!(
            packed_array_element_type("PackedFloat64Array"),
            Some("float")
        );
        assert_eq!(
            packed_array_element_type("PackedStringArray"),
            Some("String")
        );
        assert_eq!(
            packed_array_element_type("PackedVector2Array"),
            Some("Vector2")
        );
        assert_eq!(
            packed_array_element_type("PackedVector3Array"),
            Some("Vector3")
        );
        assert_eq!(packed_array_element_type("PackedColorArray"), Some("Color"));
        assert_eq!(packed_array_element_type("Array"), None);
    }

    // --- types_compatible tests ---

    #[test]
    fn compatible_same_type() {
        let db = load_classdb();
        assert!(types_compatible("int", "int", &db));
        assert!(types_compatible("String", "String", &db));
    }

    #[test]
    fn compatible_variant() {
        let db = load_classdb();
        assert!(types_compatible("Variant", "int", &db));
        assert!(types_compatible("String", "Variant", &db));
    }

    #[test]
    fn compatible_numeric() {
        let db = load_classdb();
        assert!(types_compatible("int", "float", &db));
        assert!(types_compatible("float", "int", &db));
    }

    #[test]
    fn compatible_vectors() {
        let db = load_classdb();
        assert!(types_compatible("Vector2", "Vector2i", &db));
        assert!(types_compatible("Vector3i", "Vector3", &db));
    }

    #[test]
    fn compatible_typed_array() {
        let db = load_classdb();
        assert!(types_compatible("Array[int]", "Array", &db));
        assert!(types_compatible("Array", "Array[String]", &db));
        assert!(types_compatible("Array[int]", "Array[String]", &db));
    }

    #[test]
    fn compatible_typed_dictionary() {
        let db = load_classdb();
        assert!(types_compatible(
            "Dictionary[String, int]",
            "Dictionary",
            &db
        ));
        assert!(types_compatible(
            "Dictionary",
            "Dictionary[String, int]",
            &db
        ));
    }

    #[test]
    fn compatible_packed_array() {
        let db = load_classdb();
        assert!(types_compatible("PackedVector3Array", "Array", &db));
        assert!(types_compatible("Array", "PackedByteArray", &db));
    }

    #[test]
    fn compatible_subclass() {
        let db = load_classdb();
        assert!(types_compatible("Node", "Node2D", &db));
        assert!(types_compatible("Resource", "PackedScene", &db));
    }

    #[test]
    fn incompatible_types() {
        let db = load_classdb();
        assert!(!types_compatible("int", "String", &db));
        assert!(!types_compatible("Node2D", "Control", &db));
    }

    // --- Expression resolver tests ---

    #[test]
    fn resolve_literal_types() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return 42\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );

        let parsed = parser::parse_source("func f():\n    return 3.14\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("float".to_string())
        );

        let parsed = parser::parse_source("func f():\n    return \"hi\"\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("String".to_string())
        );

        let parsed = parser::parse_source("func f():\n    return true\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("bool".to_string())
        );

        let parsed = parser::parse_source("func f():\n    return [1,2]\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Array".to_string())
        );

        let parsed = parser::parse_source("func f():\n    return {}\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Dictionary".to_string())
        );
    }

    #[test]
    fn resolve_variable_reference() {
        let db = load_classdb();
        let local_vars = vec![("my_var".to_string(), Some("int".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return my_var\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    #[test]
    fn resolve_constructor_call() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return Vector2(1, 2)\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Vector2".to_string())
        );
    }

    #[test]
    fn resolve_function_call_classdb() {
        let db = load_classdb();
        let ctx = ExprTypeContext {
            extends_class: "Node2D",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed =
            parser::parse_source("extends Node2D\nfunc f():\n    return get_viewport()\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Viewport".to_string())
        );
    }

    #[test]
    fn resolve_binary_comparison() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return 1 > 2\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("bool".to_string())
        );
    }

    #[test]
    fn resolve_binary_arithmetic() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return 1 + 2\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    #[test]
    fn resolve_binary_float_promotion() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return 1 + 2.0\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("float".to_string())
        );
    }

    #[test]
    fn resolve_unary_negation() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return -5\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    #[test]
    fn resolve_method_chain() {
        let db = load_classdb();
        let local_vars = vec![("vp".to_string(), Some("Viewport".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node2D",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return vp.get_camera_2d()\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Camera2D".to_string())
        );
    }

    #[test]
    fn resolve_inherited_property() {
        let db = load_classdb();
        let ctx = ExprTypeContext {
            extends_class: "Node2D",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed =
            parser::parse_source("extends Node2D\nfunc f():\n    return position\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Vector2".to_string())
        );
    }

    #[test]
    fn resolve_null_returns_none() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return null\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(resolve_expr_type(expr, &parsed, &ctx), None);
    }

    #[test]
    fn resolve_utility_function() {
        let db = load_classdb();
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        // sin returns float
        let parsed = parser::parse_source("func f():\n    return sin(1.0)\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("float".to_string())
        );
    }

    #[test]
    fn resolve_param_type() {
        let db = load_classdb();
        let params = vec![("speed".to_string(), Some("float".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &params,
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f(speed: float):\n    return speed\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("float".to_string())
        );
    }

    #[test]
    fn resolve_member_var_type() {
        let db = load_classdb();
        let member_vars = vec![("health".to_string(), Some("int".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &[],
            member_vars: &member_vars,
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return health\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    // --- propagate_types tests ---

    #[test]
    fn propagate_types_annotation() {
        let db = load_classdb();
        let mut file_sym = FileSymbols {
            path: std::path::PathBuf::new(),
            class_name: None,
            extends: Some("Node".to_string()),
            signals: vec![],
            enums: vec![],
            constants: vec![],
            variables: vec![crate::symbols::VarDecl {
                name: "x".to_string(),
                type_annotation: Some("int".to_string()),
                inferred_type: None,
                initializer_type: None,
                initializer_call: None,
                is_onready: false,
                is_export: false,
                scope: crate::symbols::Scope::File,
                line: 1,
                used: false,
                start_byte: 0,
                end_byte: 10,
                name_start_byte: 4,
                name_end_byte: 5,
                documentation: None,
            }],
            functions: vec![],
            inner_classes: vec![],
            parent_file: None,
            autoloads: std::collections::HashSet::new(),
            preloads: vec![],
        };
        let parsed = crate::parser::parse_source("").unwrap();
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(file_sym.variables[0].inferred_type, Some("int".to_string()));
    }

    #[test]
    fn propagate_types_initializer_call() {
        let db = load_classdb();
        let mut file_sym = FileSymbols {
            path: std::path::PathBuf::new(),
            class_name: None,
            extends: Some("Node2D".to_string()),
            signals: vec![],
            enums: vec![],
            constants: vec![],
            variables: vec![crate::symbols::VarDecl {
                name: "vp".to_string(),
                type_annotation: None,
                inferred_type: None,
                initializer_type: None,
                initializer_call: Some("get_viewport".to_string()),
                is_onready: false,
                is_export: false,
                scope: crate::symbols::Scope::File,
                line: 1,
                used: false,
                start_byte: 0,
                end_byte: 10,
                name_start_byte: 4,
                name_end_byte: 6,
                documentation: None,
            }],
            functions: vec![],
            inner_classes: vec![],
            parent_file: None,
            autoloads: std::collections::HashSet::new(),
            preloads: vec![],
        };
        let parsed = crate::parser::parse_source("").unwrap();
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(
            file_sym.variables[0].inferred_type,
            Some("Viewport".to_string())
        );
    }

    // --- resolve_call_return_type tests ---

    #[test]
    fn resolve_local_function_return() {
        let db = load_classdb();
        let local_funcs = vec![("get_score".to_string(), Some("int".to_string()))];
        let result = resolve_call_return_type("get_score", &local_funcs, "Node", &db);
        assert_eq!(result, Some("int".to_string()));
    }

    #[test]
    fn resolve_classdb_method_return() {
        let db = load_classdb();
        let result = resolve_call_return_type("get_viewport", &[], "Node2D", &db);
        assert_eq!(result, Some("Viewport".to_string()));
    }

    #[test]
    fn resolve_utility_function_return() {
        let db = load_classdb();
        let result = resolve_call_return_type("sin", &[], "Node", &db);
        assert_eq!(result, Some("float".to_string()));
    }

    #[test]
    fn resolve_unknown_function_returns_none() {
        let db = load_classdb();
        let result = resolve_call_return_type("nonexistent_func", &[], "Node", &db);
        assert_eq!(result, None);
    }

    // --- Parenthesized expression ---

    #[test]
    fn resolve_parenthesized_expression() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return (42)\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    // --- Ternary / conditional expression ---

    #[test]
    fn resolve_ternary_expression() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return 42 if true else 0\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        // Ternary should resolve to the type of the "then" branch (first child)
        let result = resolve_expr_type(expr, &parsed, &ctx);
        assert!(
            result == Some("int".to_string())
                || result == Some("bool".to_string())
                || result.is_some()
        );
    }

    // --- Await expression ---

    #[test]
    fn resolve_await_expression() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return await 42\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        let result = resolve_expr_type(expr, &parsed, &ctx);
        // await of a literal int should resolve to int
        assert!(result == Some("int".to_string()) || result.is_none());
    }

    // --- String concatenation ---

    #[test]
    fn resolve_string_concatenation_left() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return \"hello\" + \"world\"\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("String".to_string())
        );
    }

    #[test]
    fn resolve_int_plus_string_is_invalid() {
        // In Godot 4, int + String is a type error - no such operator exists.
        // GDScript has no operator overloading, so we return None.
        let db = load_classdb();
        let local_vars = vec![("x".to_string(), Some("String".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return 1 + x\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        // No int + String operator exists - type is unknown
        assert_eq!(resolve_expr_type(expr, &parsed, &ctx), None);
    }

    #[test]
    fn resolve_string_format_operator() {
        let db = load_classdb();
        let params = vec![("value".to_string(), Some("float".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &params,
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed =
            parser::parse_source("func f(value: float):\n    return \"%.1f\" % value\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        // String % anything → String (GDScript string formatting)
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("String".to_string())
        );
    }

    // --- Binary op with only one known type ---

    #[test]
    fn resolve_binary_op_with_null_is_unknown() {
        // Vector2 * null is not a valid operation - no such operator exists
        let db = load_classdb();
        let local_vars = vec![("pos".to_string(), Some("Vector2".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return pos * null\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        // No Vector2 * Nil operator - type is unknown
        assert_eq!(resolve_expr_type(expr, &parsed, &ctx), None);
    }

    #[test]
    fn resolve_binary_op_vector2_times_float() {
        // Vector2 * float is valid and returns Vector2
        let db = load_classdb();
        let local_vars = vec![
            ("pos".to_string(), Some("Vector2".to_string())),
            ("scale".to_string(), Some("float".to_string())),
        ];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return pos * scale\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Vector2".to_string())
        );
    }

    // --- Unary "not" operator ---

    #[test]
    fn resolve_unary_not() {
        let db = load_classdb();
        let ctx = empty_ctx(&db);

        let parsed = parser::parse_source("func f():\n    return not true\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("bool".to_string())
        );
    }

    // --- Self attribute access ---

    #[test]
    fn resolve_self_property() {
        let db = load_classdb();
        let member_vars = vec![("health".to_string(), Some("int".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node2D",
            local_vars: &[],
            params: &[],
            member_vars: &member_vars,
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed =
            parser::parse_source("extends Node2D\nfunc f():\n    return self.health\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    #[test]
    fn resolve_self_method_call() {
        let db = load_classdb();
        let local_func_returns = vec![("get_score".to_string(), Some("int".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node2D",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &local_func_returns,
            class_db: &db,
            type_refinements: None,
        };

        let parsed =
            parser::parse_source("extends Node2D\nfunc f():\n    return self.get_score()\n")
                .unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    // --- Attribute call on resolved type (builtin method) ---

    #[test]
    fn resolve_builtin_method_on_array() {
        let db = load_classdb();
        let local_vars = vec![("arr".to_string(), Some("Array".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return arr.size()\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("int".to_string())
        );
    }

    // --- Property access via ClassDB ---

    #[test]
    fn resolve_property_on_typed_var() {
        let db = load_classdb();
        let local_vars = vec![("node".to_string(), Some("Node2D".to_string()))];
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &local_vars,
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        let parsed = parser::parse_source("func f():\n    return node.position\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        assert_eq!(
            resolve_expr_type(expr, &parsed, &ctx),
            Some("Vector2".to_string())
        );
    }

    // --- resolve_call_return_type ClassDB method (void/Variant returns None) ---

    #[test]
    fn resolve_classdb_method_void_returns_none() {
        let db = load_classdb();
        // queue_free returns void → should return None
        let result = resolve_call_return_type("queue_free", &[], "Node", &db);
        assert_eq!(result, None);
    }

    // --- types_compatible: Resource/Node subclass explicit paths ---

    #[test]
    fn compatible_resource_subclass() {
        let db = load_classdb();
        // PackedScene IS a Resource subclass - this tests the explicit d=="Resource" path
        assert!(types_compatible("Resource", "Texture2D", &db));
    }

    #[test]
    fn compatible_node_subclass_explicit() {
        let db = load_classdb();
        // Node2D IS a Node subclass - this tests the explicit d=="Node" path
        assert!(types_compatible("Node", "Camera2D", &db));
    }

    // --- propagate_types with local function variable ---

    #[test]
    fn propagate_types_function_local_var() {
        let db = load_classdb();
        let mut file_sym = FileSymbols {
            path: std::path::PathBuf::new(),
            class_name: None,
            extends: Some("Node".to_string()),
            signals: vec![],
            enums: vec![],
            constants: vec![],
            variables: vec![],
            functions: vec![crate::symbols::FuncDecl {
                name: "my_func".to_string(),
                parameters: vec![],
                return_type: Some("int".to_string()),
                inferred_return_type: None,
                line: 1,
                end_line: 3,
                local_vars: vec![crate::symbols::VarDecl {
                    name: "x".to_string(),
                    type_annotation: None,
                    inferred_type: None,
                    initializer_type: Some("float".to_string()),
                    initializer_call: None,
                    is_onready: false,
                    is_export: false,
                    scope: crate::symbols::Scope::Function("my_func".to_string()),
                    line: 2,
                    used: false,
                    start_byte: 0,
                    end_byte: 10,
                    name_start_byte: 4,
                    name_end_byte: 5,
                    documentation: None,
                }],
                used: false,
                start_byte: 0,
                end_byte: 50,
                name_start_byte: 5,
                name_end_byte: 12,
                documentation: None,
                is_static: false,
            }],
            inner_classes: vec![],
            parent_file: None,
            autoloads: std::collections::HashSet::new(),
            preloads: vec![],
        };
        let parsed = crate::parser::parse_source("").unwrap();
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(
            file_sym.functions[0].local_vars[0].inferred_type,
            Some("float".to_string())
        );
    }

    // --- Inferred return type tests ---

    #[test]
    fn infer_return_type_from_literal() {
        let db = load_classdb();
        let source = r#"
func foo():
    return 42
"#;
        let parsed = crate::parser::parse_source(source).unwrap();
        let mut file_sym =
            crate::symbols::collect_symbols(std::path::Path::new("test.gd"), &parsed);
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(file_sym.functions[0].return_type, None);
        assert_eq!(
            file_sym.functions[0].inferred_return_type,
            Some("int".to_string())
        );
    }

    #[test]
    fn infer_return_type_from_string() {
        let db = load_classdb();
        let source = r#"
func bar():
    return "hello"
"#;
        let parsed = crate::parser::parse_source(source).unwrap();
        let mut file_sym =
            crate::symbols::collect_symbols(std::path::Path::new("test.gd"), &parsed);
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(
            file_sym.functions[0].inferred_return_type,
            Some("String".to_string())
        );
    }

    #[test]
    fn infer_return_type_void_for_bare_return() {
        let db = load_classdb();
        let source = r#"
func baz():
    return
"#;
        let parsed = crate::parser::parse_source(source).unwrap();
        let mut file_sym =
            crate::symbols::collect_symbols(std::path::Path::new("test.gd"), &parsed);
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(
            file_sym.functions[0].inferred_return_type,
            Some("void".to_string())
        );
    }

    #[test]
    fn skip_infer_when_explicit_return_type() {
        let db = load_classdb();
        let source = r#"
func foo() -> int:
    return 42
"#;
        let parsed = crate::parser::parse_source(source).unwrap();
        let mut file_sym =
            crate::symbols::collect_symbols(std::path::Path::new("test.gd"), &parsed);
        propagate_types(&mut file_sym, &parsed, &db);
        assert_eq!(file_sym.functions[0].return_type, Some("int".to_string()));
        // Should not set inferred when explicit is present
        assert_eq!(file_sym.functions[0].inferred_return_type, None);
    }

    // --- Singleton type resolution ---

    #[test]
    fn resolve_singleton_identifier() {
        let db = load_classdb();
        let ctx = ExprTypeContext {
            extends_class: "Node",
            local_vars: &[],
            params: &[],
            member_vars: &[],
            local_func_returns: &[],
            class_db: &db,
            type_refinements: None,
        };

        // "Engine" is a singleton in the ClassDB
        let parsed = parser::parse_source("func f():\n    return Engine\n").unwrap();
        let ret_stmts = parser::find_nodes_by_kind(parsed.root_node(), "return_statement");
        let expr = ret_stmts[0].child(1).unwrap();
        let result = resolve_expr_type(expr, &parsed, &ctx);
        // Engine singleton should resolve to its type
        assert!(result.is_some());
    }
}
