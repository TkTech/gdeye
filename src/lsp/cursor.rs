//! Cursor context for LSP handlers.
//!
//! This module provides a unified way to determine what's at the cursor position,
//! avoiding repeated AST traversal in each handler.

use tree_sitter::Node;

use crate::parser::ParsedFile;

/// What kind of construct the cursor is on.
#[derive(Debug, Clone)]
pub enum CursorKind {
    /// On an identifier that references something (variable, function, class, etc.).
    Reference {
        /// The name being referenced.
        name: String,
    },
    /// On a type annotation (e.g., `: StateStore`, `-> int`).
    TypeAnnotation {
        /// The type name.
        type_name: String,
    },
    /// On a function definition name.
    FunctionDef {
        /// The function name.
        name: String,
    },
    /// On a variable definition.
    VariableDef {
        /// The variable name.
        name: String,
    },
    /// On a member access (e.g., `.get_state`).
    MemberAccess {
        /// The receiver expression text.
        receiver_text: String,
        /// Byte range of the receiver.
        receiver_range: (usize, usize),
        /// The member being accessed.
        member: String,
    },
    /// On a method call (e.g., `.get_state()`).
    MethodCall {
        /// The receiver expression text (None for bare calls).
        receiver_text: Option<String>,
        /// Byte range of the receiver (if present).
        receiver_range: Option<(usize, usize)>,
        /// The method name.
        method: String,
        /// Number of arguments.
        arg_count: usize,
    },
    /// On a string literal.
    StringLiteral {
        /// The string value (without quotes).
        value: String,
    },
    /// Inside a comment.
    Comment,
    /// On a signal definition name.
    SignalDef {
        /// The signal name.
        name: String,
    },
    /// On a constant definition name.
    ConstantDef {
        /// The constant name.
        name: String,
    },
    /// On an enum definition name.
    EnumDef {
        /// The enum name.
        name: String,
    },
    /// On a class definition name.
    ClassDef {
        /// The class name.
        name: String,
    },
    /// On an annotation (e.g., `@export`, `@onready`).
    Annotation {
        /// The annotation name (without @).
        name: String,
    },
    /// On a parameter definition.
    ParameterDef {
        /// The parameter name.
        name: String,
        /// The containing function name.
        function_name: String,
    },
    /// Unknown/whitespace/unrecognized.
    Unknown,
}

/// Context about what's at the cursor position.
#[derive(Debug)]
pub struct CursorContext<'a> {
    /// The node directly at the cursor.
    pub node: Node<'a>,
    /// The parent node (if any).
    pub parent: Option<Node<'a>>,
    /// All ancestor nodes from immediate parent to root.
    pub ancestors: Vec<Node<'a>>,
    /// What kind of construct this is.
    pub kind: CursorKind,
    /// Byte range of the cursor node.
    pub range: (usize, usize),
    /// The source text.
    source: &'a str,
    /// The expected type at this position (if known).
    /// E.g., inside a return statement, this is the function's return type.
    pub expected_type: Option<String>,
}

impl<'a> CursorContext<'a> {
    /// Create a cursor context at the given byte offset.
    pub fn at_offset(parsed: &'a ParsedFile, offset: usize) -> Option<Self> {
        let root = parsed.root_node();
        let source = parsed.source();

        // Find the deepest node containing the offset
        let node = Self::find_deepest_node(root, offset)?;
        let parent = node.parent();
        let ancestors = Self::collect_ancestors(node);
        let range = (node.start_byte(), node.end_byte());

        // Determine the kind based on context
        let mut kind = Self::determine_kind(node, parent, &ancestors, source);

        // If we got Unknown, check for incomplete member access patterns
        // This handles cases like "store." or "store.g" where cursor is on/after the dot
        if matches!(kind, CursorKind::Unknown) {
            // Case 1: Node is an "attribute" - this is a valid parse of "obj.member"
            if node.kind() == "attribute" {
                // Get the first named child (the receiver)
                let receiver = (0..node.child_count())
                    .filter_map(|i| node.child(i))
                    .find(|c| c.is_named());
                if let Some(receiver) = receiver {
                    let receiver_text = Self::node_text(receiver, source).to_string();
                    let receiver_range = (receiver.start_byte(), receiver.end_byte());
                    // Get the member if it exists (second named child)
                    let member = (0..node.child_count())
                        .filter_map(|i| node.child(i))
                        .filter(|c| c.is_named())
                        .nth(1)
                        .filter(|n| n.kind() == "identifier")
                        .map(|n| Self::node_text(n, source).to_string())
                        .unwrap_or_default();
                    kind = CursorKind::MemberAccess {
                        receiver_text,
                        receiver_range,
                        member,
                    };
                }
            }
            // Case 2: Cursor is on a '.' node whose parent is ERROR or attribute
            else if node.kind() == "." {
                if let Some(p) = parent {
                    if p.kind() == "ERROR" || p.kind() == "attribute" {
                        if let Some(ident) = p.child(0) {
                            if ident.kind() == "identifier" {
                                let receiver_text = Self::node_text(ident, source).to_string();
                                let receiver_range = (ident.start_byte(), ident.end_byte());
                                kind = CursorKind::MemberAccess {
                                    receiver_text,
                                    receiver_range,
                                    member: String::new(),
                                };
                            }
                        }
                    }
                }
            }
            // Case 3: Cursor is right after a dot (e.g., at newline after "store.")
            else if offset > 0 && source.as_bytes().get(offset - 1).copied() == Some(b'.') {
                if let Some(prev_node) = Self::find_deepest_node(root, offset - 1) {
                    let mut check_node = Some(prev_node);
                    while let Some(n) = check_node {
                        if n.kind() == "ERROR" || n.kind() == "attribute" {
                            if let Some(ident) = n.child(0) {
                                if ident.kind() == "identifier" {
                                    let receiver_text = Self::node_text(ident, source).to_string();
                                    let receiver_range = (ident.start_byte(), ident.end_byte());
                                    kind = CursorKind::MemberAccess {
                                        receiver_text,
                                        receiver_range,
                                        member: String::new(),
                                    };
                                    break;
                                }
                            }
                        }
                        check_node = n.parent();
                    }
                }
            }
        }

        // Determine expected type from context (e.g., return statement -> function return type)
        let expected_type = Self::compute_expected_type(node, &ancestors, source, offset);

        Some(Self {
            node,
            parent,
            ancestors,
            kind,
            range,
            source,
            expected_type,
        })
    }

    /// Compute the expected type at this position based on context.
    /// Returns the expected type if we're in a context where a specific type is expected.
    fn compute_expected_type(
        node: Node<'a>,
        ancestors: &[Node<'a>],
        source: &'a str,
        offset: usize,
    ) -> Option<String> {
        // Check if we're inside a return statement OR right after "return "
        let in_return = ancestors.iter().any(|n| n.kind() == "return_statement")
            || Self::is_after_return_keyword(node, ancestors, source, offset);

        if in_return {
            // Find the enclosing function_definition
            for ancestor in ancestors {
                if ancestor.kind() == "function_definition" {
                    // Look for the return type (child with kind "type")
                    for i in 0..ancestor.child_count() {
                        if let Some(child) = ancestor.child(i) {
                            if child.kind() == "type" {
                                let type_text = Self::node_text(child, source).trim().to_string();
                                if !type_text.is_empty() {
                                    return Some(type_text);
                                }
                            }
                        }
                    }
                    break;
                }
            }
        }

        None
    }

    /// Check if cursor is right after a "return" keyword.
    fn is_after_return_keyword(
        _node: Node<'a>,
        _ancestors: &[Node<'a>],
        source: &'a str,
        offset: usize,
    ) -> bool {
        if offset > source.len() {
            return false;
        }

        // Get the current line up to the cursor
        let text_before = &source[..offset];
        let current_line = text_before.lines().last().unwrap_or("");

        // Check if the line contains "return" followed by whitespace (and optionally a partial expression)
        // Pattern: "return" followed by whitespace, then maybe some identifier chars
        let trimmed = current_line.trim_start();
        if let Some(after_return) = trimmed.strip_prefix("return") {
            // "return" must be followed by whitespace or end of line
            if after_return.is_empty() || after_return.starts_with(char::is_whitespace) {
                return true;
            }
        }

        false
    }

    /// Create a cursor context from LSP position (line/column).
    pub fn at_position(
        parsed: &'a ParsedFile,
        line: usize,
        character: usize,
        doc: &crate::document::Document,
    ) -> Option<Self> {
        let offset = doc.offset_at(line, character)?;
        Self::at_offset(parsed, offset)
    }

    /// Find the deepest node containing the offset.
    fn find_deepest_node(root: Node<'a>, offset: usize) -> Option<Node<'a>> {
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

    /// Collect all ancestor nodes from immediate parent to root.
    fn collect_ancestors(node: Node<'a>) -> Vec<Node<'a>> {
        let mut ancestors = Vec::new();
        let mut current = node.parent();
        while let Some(parent) = current {
            ancestors.push(parent);
            current = parent.parent();
        }
        ancestors
    }

    /// Determine what kind of cursor position this is.
    fn determine_kind(
        node: Node<'a>,
        parent: Option<Node<'a>>,
        ancestors: &[Node<'a>],
        source: &'a str,
    ) -> CursorKind {
        let node_kind = node.kind();
        let node_text = Self::node_text(node, source);

        // Check for comments
        if node_kind == "comment" {
            return CursorKind::Comment;
        }

        // Check for string literals
        if node_kind == "string" {
            let value = node_text.trim_matches('"').trim_matches('\'').to_string();
            return CursorKind::StringLiteral { value };
        }

        // Check for annotations
        if node_kind == "annotation" {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = Self::node_text(name_node, source)
                    .trim_start_matches('@')
                    .to_string();
                return CursorKind::Annotation { name };
            }
        }

        // For identifiers and names, check the parent context
        if node_kind == "identifier" || node_kind == "name" {
            if let Some(parent) = parent {
                return Self::kind_from_parent(node, parent, ancestors, source);
            }
            // Standalone identifier - likely a reference
            return CursorKind::Reference {
                name: node_text.to_string(),
            };
        }

        // Check for type nodes
        if node_kind == "type" {
            // Get the type name from the first identifier child
            if let Some(name_node) = node.child(0) {
                let type_name = Self::node_text(name_node, source).to_string();
                return CursorKind::TypeAnnotation { type_name };
            }
        }

        // Check for ERROR nodes that look like incomplete member access (e.g., "store.")
        if node_kind == "ERROR" {
            // Look for pattern: identifier followed by a dot
            if let Some(ident) = node.child(0) {
                if ident.kind() == "identifier" {
                    let error_text = node_text;
                    let ident_text = Self::node_text(ident, source);
                    // Check if the ERROR node text is "identifier." pattern
                    if error_text.starts_with(ident_text)
                        && error_text[ident_text.len()..].trim_start().starts_with('.')
                    {
                        let receiver_text = ident_text.to_string();
                        let receiver_range = (ident.start_byte(), ident.end_byte());
                        return CursorKind::MemberAccess {
                            receiver_text,
                            receiver_range,
                            member: String::new(), // No member typed yet
                        };
                    }
                }
            }
        }

        CursorKind::Unknown
    }

    /// Determine kind based on the parent node context.
    fn kind_from_parent(
        node: Node<'a>,
        parent: Node<'a>,
        ancestors: &[Node<'a>],
        source: &'a str,
    ) -> CursorKind {
        let parent_kind = parent.kind();
        let node_text_ref = Self::node_text(node, source);
        let node_text = node_text_ref.to_string();

        match parent_kind {
            // Type annotations
            "type" => CursorKind::TypeAnnotation {
                type_name: node_text,
            },

            // Function definitions
            "function_definition" => {
                // Check if we're on the function name
                if let Some(name_node) = parent.child_by_field_name("name") {
                    if name_node.id() == node.id() {
                        return CursorKind::FunctionDef { name: node_text };
                    }
                }
                CursorKind::Reference { name: node_text }
            }

            // Variable statements
            "variable_statement" => {
                // Check if we're on the variable name
                if let Some(name_node) = parent.child_by_field_name("name") {
                    if name_node.id() == node.id() {
                        return CursorKind::VariableDef { name: node_text };
                    }
                }
                CursorKind::Reference { name: node_text }
            }

            // Signal statements
            "signal_statement" => CursorKind::SignalDef { name: node_text },

            // Constant statements
            "const_statement" => CursorKind::ConstantDef { name: node_text },

            // Enum definitions
            "enum_definition" => CursorKind::EnumDef { name: node_text },

            // Class definitions
            "class_definition" => {
                if let Some(name_node) = parent.child_by_field_name("name") {
                    if name_node.id() == node.id() {
                        return CursorKind::ClassDef { name: node_text };
                    }
                }
                CursorKind::Reference { name: node_text }
            }

            // Parameter definitions
            "parameter" | "typed_parameter" => {
                // Find the containing function name
                let func_name = ancestors
                    .iter()
                    .find(|n| n.kind() == "function_definition")
                    .and_then(|f| f.child_by_field_name("name"))
                    .map(|n| Self::node_text(n, source).to_string())
                    .unwrap_or_default();
                CursorKind::ParameterDef {
                    name: node_text,
                    function_name: func_name,
                }
            }

            // Attribute access (e.g., obj.member)
            "attribute" => {
                // Get the receiver (first named child)
                if let Some(receiver) = Self::get_first_named_child(parent) {
                    if receiver.id() == node.id() {
                        // We're on the receiver, not the member
                        return CursorKind::Reference { name: node_text };
                    }
                    // We're on the member
                    let receiver_text = Self::node_text(receiver, source).to_string();
                    let receiver_range = (receiver.start_byte(), receiver.end_byte());
                    return CursorKind::MemberAccess {
                        receiver_text,
                        receiver_range,
                        member: node_text,
                    };
                }
                CursorKind::Reference { name: node_text }
            }

            // Attribute call (e.g., obj.method())
            "attribute_call" => {
                // Method name is the first identifier
                let method = node_text.clone();
                // Find the parent "attribute" node to get the receiver
                if let Some(attr_parent) = ancestors.iter().find(|n| n.kind() == "attribute") {
                    if let Some(receiver) = Self::get_first_named_child(*attr_parent) {
                        let receiver_text = Self::node_text(receiver, source).to_string();
                        let receiver_range = (receiver.start_byte(), receiver.end_byte());
                        // Count arguments
                        let arg_count = Self::count_arguments(parent, source);
                        return CursorKind::MethodCall {
                            receiver_text: Some(receiver_text),
                            receiver_range: Some(receiver_range),
                            method,
                            arg_count,
                        };
                    }
                }
                CursorKind::Reference { name: node_text }
            }

            // Regular function call
            "call" => {
                // Check if we're on the function name (first child)
                if let Some(first_child) = parent.child(0) {
                    if first_child.id() == node.id() {
                        let arg_count = Self::count_arguments(parent, source);
                        return CursorKind::MethodCall {
                            receiver_text: None,
                            receiver_range: None,
                            method: node_text,
                            arg_count,
                        };
                    }
                }
                CursorKind::Reference { name: node_text }
            }

            // Arguments - the identifier is being passed as an argument
            "arguments" => CursorKind::Reference { name: node_text },

            // Default: treat as reference
            _ => CursorKind::Reference { name: node_text },
        }
    }

    /// Get the first named child of a node.
    fn get_first_named_child(node: Node<'a>) -> Option<Node<'a>> {
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

    /// Count arguments in a call/attribute_call node.
    fn count_arguments(node: Node<'a>, _source: &str) -> usize {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "arguments" {
                return child
                    .children(&mut child.walk())
                    .filter(|c| c.is_named())
                    .count();
            }
        }
        0
    }

    /// Get the text of a node.
    fn node_text(node: Node<'a>, source: &'a str) -> &'a str {
        &source[node.start_byte()..node.end_byte()]
    }

    /// Get the text of the cursor node.
    pub fn text(&self) -> &str {
        &self.source[self.range.0..self.range.1]
    }

    /// Get the name if this is any kind of definition or reference.
    pub fn name(&self) -> Option<&str> {
        match &self.kind {
            CursorKind::Reference { name }
            | CursorKind::TypeAnnotation { type_name: name }
            | CursorKind::FunctionDef { name }
            | CursorKind::VariableDef { name }
            | CursorKind::SignalDef { name }
            | CursorKind::ConstantDef { name }
            | CursorKind::EnumDef { name }
            | CursorKind::ClassDef { name }
            | CursorKind::Annotation { name } => Some(name),
            CursorKind::MemberAccess { member, .. }
            | CursorKind::MethodCall { method: member, .. } => Some(member),
            CursorKind::ParameterDef { name, .. } => Some(name),
            CursorKind::StringLiteral { .. } | CursorKind::Comment | CursorKind::Unknown => None,
        }
    }

    /// Check if this is a definition (not a reference).
    pub fn is_definition(&self) -> bool {
        matches!(
            self.kind,
            CursorKind::FunctionDef { .. }
                | CursorKind::VariableDef { .. }
                | CursorKind::SignalDef { .. }
                | CursorKind::ConstantDef { .. }
                | CursorKind::EnumDef { .. }
                | CursorKind::ClassDef { .. }
                | CursorKind::ParameterDef { .. }
        )
    }

    /// Check if this is a reference to something.
    pub fn is_reference(&self) -> bool {
        matches!(
            self.kind,
            CursorKind::Reference { .. }
                | CursorKind::TypeAnnotation { .. }
                | CursorKind::MemberAccess { .. }
                | CursorKind::MethodCall { .. }
        )
    }

    /// Check if the cursor is in a position where completions make sense.
    pub fn is_completion_position(&self) -> bool {
        matches!(
            self.kind,
            CursorKind::Reference { .. }
                | CursorKind::MemberAccess { .. }
                | CursorKind::MethodCall { .. }
                | CursorKind::Unknown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn make_context(source: &str, offset: usize) -> Option<CursorContext<'static>> {
        // Note: This leaks memory in tests, which is fine for testing
        let source = Box::leak(source.to_string().into_boxed_str());
        let parsed = Box::leak(Box::new(parser::parse_source(source).unwrap()));
        CursorContext::at_offset(parsed, offset)
    }

    #[test]
    fn reference_on_identifier() {
        let source = "func foo():\n\tvar x = bar\n";
        // Position on "bar"
        let ctx = make_context(source, source.find("bar").unwrap()).unwrap();
        assert!(matches!(ctx.kind, CursorKind::Reference { ref name } if name == "bar"));
    }

    #[test]
    fn function_definition() {
        let source = "func my_func():\n\tpass\n";
        // Position on "my_func"
        let ctx = make_context(source, source.find("my_func").unwrap()).unwrap();
        assert!(matches!(ctx.kind, CursorKind::FunctionDef { ref name } if name == "my_func"));
    }

    #[test]
    fn variable_definition() {
        let source = "var my_var = 10\n";
        // Position on "my_var"
        let ctx = make_context(source, source.find("my_var").unwrap()).unwrap();
        assert!(matches!(ctx.kind, CursorKind::VariableDef { ref name } if name == "my_var"));
    }

    #[test]
    fn type_annotation() {
        let source = "var x: MyType = null\n";
        // Position on "MyType"
        let ctx = make_context(source, source.find("MyType").unwrap()).unwrap();
        assert!(
            matches!(ctx.kind, CursorKind::TypeAnnotation { ref type_name } if type_name == "MyType")
        );
    }

    #[test]
    fn member_access() {
        let source = "func foo():\n\tvar x = obj.member\n";
        // Position on "member"
        let ctx = make_context(source, source.find("member").unwrap()).unwrap();
        assert!(
            matches!(ctx.kind, CursorKind::MemberAccess { ref member, ref receiver_text, .. } if member == "member" && receiver_text == "obj")
        );
    }

    #[test]
    fn incomplete_member_access_after_dot() {
        // "store." with cursor right after the dot - incomplete member access
        let source = "func foo():\n\tstore.\n";
        // Position right after the dot (which is at the end of "store.")
        let dot_pos = source.find('.').unwrap();
        let ctx = make_context(source, dot_pos + 1).unwrap();
        assert!(
            matches!(ctx.kind, CursorKind::MemberAccess { ref member, ref receiver_text, .. } if member.is_empty() && receiver_text == "store"),
            "Expected MemberAccess with receiver 'store', got {:?}",
            ctx.kind
        );
    }

    #[test]
    fn incomplete_member_access_on_dot() {
        // "store." with cursor on the dot itself
        let source = "func foo():\n\tstore.\n";
        let dot_pos = source.find('.').unwrap();
        let ctx = make_context(source, dot_pos).unwrap();
        assert!(
            matches!(ctx.kind, CursorKind::MemberAccess { ref member, ref receiver_text, .. } if member.is_empty() && receiver_text == "store"),
            "Expected MemberAccess with receiver 'store', got {:?}",
            ctx.kind
        );
    }

    #[test]
    fn string_literal() {
        let source = "var x = \"hello\"\n";
        // Position inside the string
        let ctx = make_context(source, source.find("hello").unwrap()).unwrap();
        assert!(matches!(ctx.kind, CursorKind::StringLiteral { ref value } if value == "hello"));
    }
}
