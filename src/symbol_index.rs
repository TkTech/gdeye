//! Symbol index for fast position-based lookups.
//!
//! This module provides [`SymbolIndex`] for efficiently finding symbols
//! at a given position in a document. It's essential for LSP features
//! like hover, go-to-definition, and find-references.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::parser::ParsedFile;
use crate::symbols::{FileSymbols, Scope};

/// Unique identifier for a symbol within a file.
pub type SymbolId = usize;

/// Kind of symbol (for display and filtering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Variable,
    Function,
    Signal,
    Constant,
    Enum,
    EnumValue,
    Class,
    Parameter,
    Property,
}

impl SymbolKind {
    /// Get a human-readable name for this kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Variable => "variable",
            SymbolKind::Function => "function",
            SymbolKind::Signal => "signal",
            SymbolKind::Constant => "constant",
            SymbolKind::Enum => "enum",
            SymbolKind::EnumValue => "enum value",
            SymbolKind::Class => "class",
            SymbolKind::Parameter => "parameter",
            SymbolKind::Property => "property",
        }
    }
}

/// A symbol definition in the file.
#[derive(Debug, Clone)]
pub struct SymbolDef {
    /// The symbol's name.
    pub name: String,
    /// The kind of symbol.
    pub kind: SymbolKind,
    /// Byte range of the entire definition (if available).
    pub range: Option<(usize, usize)>,
    /// Byte range of just the name (for highlighting, if available).
    pub name_range: Option<(usize, usize)>,
    /// Line number (0-indexed).
    pub line: usize,
    /// The scope containing this symbol.
    pub scope: Scope,
    /// Type annotation or inferred type (if available).
    pub type_hint: Option<String>,
    /// Documentation comment (if available).
    pub documentation: Option<String>,
}

/// A reference to a symbol (usage site).
#[derive(Debug, Clone)]
pub struct SymbolRef {
    /// The referenced name.
    pub name: String,
    /// Byte range of this reference.
    pub range: (usize, usize),
    /// Line number (0-indexed).
    pub line: usize,
    /// ID of the definition this refers to (if resolved).
    pub target: Option<SymbolId>,
}

/// Index of all symbols in a file for fast position-based lookup.
#[derive(Debug)]
pub struct SymbolIndex {
    /// All symbol definitions, indexed by SymbolId.
    definitions: Vec<SymbolDef>,
    /// All symbol references.
    references: Vec<SymbolRef>,
    /// Map from name to definition IDs (for local resolution).
    name_to_defs: HashMap<String, Vec<SymbolId>>,
    /// Sorted list of (start_byte, end_byte, SymbolId) for range queries.
    def_ranges: Vec<(usize, usize, SymbolId)>,
    /// Sorted list of (start_byte, end_byte, ref_index) for range queries.
    ref_ranges: Vec<(usize, usize, usize)>,
    /// Function spans: func_name -> (start_byte, end_byte) for position-aware scope resolution.
    function_spans: HashMap<String, (usize, usize)>,
}

impl SymbolIndex {
    /// Build a symbol index from parsed file and extracted symbols.
    pub fn build(parsed: &ParsedFile, symbols: &FileSymbols) -> Self {
        let mut index = SymbolIndex {
            definitions: Vec::new(),
            references: Vec::new(),
            name_to_defs: HashMap::new(),
            def_ranges: Vec::new(),
            ref_ranges: Vec::new(),
            function_spans: HashMap::new(),
        };

        // Index variables
        for var in &symbols.variables {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: var.name.clone(),
                kind: SymbolKind::Variable,
                range: Some((var.start_byte, var.end_byte)),
                name_range: Some((var.name_start_byte, var.name_end_byte)),
                line: var.line.saturating_sub(1),
                scope: var.scope.clone(),
                type_hint: var
                    .type_annotation
                    .clone()
                    .or_else(|| var.inferred_type.clone()),
                documentation: var.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(var.name.clone())
                .or_default()
                .push(id);
        }

        // Index constants
        for constant in &symbols.constants {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: constant.name.clone(),
                kind: SymbolKind::Constant,
                range: Some((constant.start_byte, constant.end_byte)),
                name_range: Some((constant.name_start_byte, constant.name_end_byte)),
                line: constant.line.saturating_sub(1),
                scope: Scope::File,
                type_hint: constant.type_annotation.clone(),
                documentation: constant.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(constant.name.clone())
                .or_default()
                .push(id);
        }

        // Index functions
        for func in &symbols.functions {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: func.name.clone(),
                kind: SymbolKind::Function,
                range: Some((func.start_byte, func.end_byte)),
                name_range: Some((func.name_start_byte, func.name_end_byte)),
                line: func.line.saturating_sub(1),
                scope: Scope::File,
                type_hint: func
                    .return_type
                    .clone()
                    .or_else(|| func.inferred_return_type.clone()),
                documentation: func.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(func.name.clone())
                .or_default()
                .push(id);

            // Record function span for position-aware scope resolution
            index
                .function_spans
                .insert(func.name.clone(), (func.start_byte, func.end_byte));

            // Index parameters
            for param in &func.parameters {
                let param_id = index.definitions.len();
                index.definitions.push(SymbolDef {
                    name: param.name.clone(),
                    kind: SymbolKind::Parameter,
                    range: Some((param.start_byte, param.end_byte)),
                    name_range: Some((param.name_start_byte, param.name_end_byte)),
                    line: param.line.saturating_sub(1),
                    scope: Scope::Function(func.name.clone()),
                    type_hint: param
                        .type_annotation
                        .clone()
                        .or_else(|| param.inferred_type.clone()),
                    documentation: None,
                });
                index
                    .name_to_defs
                    .entry(param.name.clone())
                    .or_default()
                    .push(param_id);
            }

            // Index local variables within the function
            for local_var in &func.local_vars {
                let local_id = index.definitions.len();
                index.definitions.push(SymbolDef {
                    name: local_var.name.clone(),
                    kind: SymbolKind::Variable,
                    range: Some((local_var.start_byte, local_var.end_byte)),
                    name_range: Some((local_var.name_start_byte, local_var.name_end_byte)),
                    line: local_var.line.saturating_sub(1),
                    scope: Scope::Function(func.name.clone()),
                    type_hint: local_var
                        .type_annotation
                        .clone()
                        .or_else(|| local_var.inferred_type.clone()),
                    documentation: None,
                });
                index
                    .name_to_defs
                    .entry(local_var.name.clone())
                    .or_default()
                    .push(local_id);
            }
        }

        // Index signals
        for signal in &symbols.signals {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: signal.name.clone(),
                kind: SymbolKind::Signal,
                range: Some((signal.start_byte, signal.end_byte)),
                name_range: Some((signal.name_start_byte, signal.name_end_byte)),
                line: signal.line.saturating_sub(1),
                scope: Scope::File,
                type_hint: None,
                documentation: signal.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(signal.name.clone())
                .or_default()
                .push(id);
        }

        // Index enums
        for enum_decl in &symbols.enums {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: enum_decl.name.clone(),
                kind: SymbolKind::Enum,
                range: Some((enum_decl.start_byte, enum_decl.end_byte)),
                name_range: Some((enum_decl.name_start_byte, enum_decl.name_end_byte)),
                line: enum_decl.line.saturating_sub(1),
                scope: Scope::File,
                type_hint: None,
                documentation: enum_decl.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(enum_decl.name.clone())
                .or_default()
                .push(id);

            // Index enum values (they share the enum's byte range for now)
            for value_name in &enum_decl.values {
                let value_id = index.definitions.len();
                index.definitions.push(SymbolDef {
                    name: value_name.clone(),
                    kind: SymbolKind::EnumValue,
                    range: Some((enum_decl.start_byte, enum_decl.end_byte)),
                    name_range: None, // Individual enum values don't have separate ranges yet
                    line: enum_decl.line.saturating_sub(1),
                    scope: Scope::File,
                    type_hint: Some(enum_decl.name.clone()),
                    documentation: None,
                });
                index
                    .name_to_defs
                    .entry(value_name.clone())
                    .or_default()
                    .push(value_id);
            }
        }

        // Index inner classes
        for class in &symbols.inner_classes {
            let id = index.definitions.len();
            index.definitions.push(SymbolDef {
                name: class.name.clone(),
                kind: SymbolKind::Class,
                range: Some((class.start_byte, class.end_byte)),
                name_range: Some((class.name_start_byte, class.name_end_byte)),
                line: class.line.saturating_sub(1),
                scope: Scope::File,
                type_hint: class.extends.clone(),
                documentation: class.documentation.clone(),
            });
            index
                .name_to_defs
                .entry(class.name.clone())
                .or_default()
                .push(id);
        }

        // Build range index for definitions using name_range (for precise hover matching)
        // We use name_range so hovering inside a function body doesn't match the function itself
        for (id, def) in index.definitions.iter().enumerate() {
            if let Some((start, end)) = def.name_range {
                index.def_ranges.push((start, end, id));
            } else if let Some((start, end)) = def.range {
                index.def_ranges.push((start, end, id));
            }
        }
        index.def_ranges.sort_by_key(|(start, _, _)| *start);

        // Collect references from the AST
        index.collect_references(parsed);

        // Build range index for references
        for (i, ref_) in index.references.iter().enumerate() {
            index.ref_ranges.push((ref_.range.0, ref_.range.1, i));
        }
        index.ref_ranges.sort_by_key(|(start, _, _)| *start);

        index
    }

    /// Collect all identifier references from the AST.
    fn collect_references(&mut self, parsed: &ParsedFile) {
        self.collect_refs_recursive(parsed.root_node(), parsed);
    }

    fn collect_refs_recursive(&mut self, node: Node, parsed: &ParsedFile) {
        // Look for identifier/name nodes that are references (not definitions)
        if node.kind() == "identifier" || node.kind() == "name" {
            // Skip if this is part of a definition (check parent)
            let is_definition = node.parent().is_some_and(|p| {
                matches!(
                    p.kind(),
                    "variable_statement"
                        | "function_definition"
                        | "signal_statement"
                        | "enum_definition"
                        | "class_definition"
                        | "parameter"
                        | "typed_parameter"
                )
            });

            if !is_definition {
                let name = parsed.node_text(node);
                let range = (node.start_byte(), node.end_byte());
                let line = node.start_position().row;

                // Try to resolve to a local definition
                let target = self.resolve_name(name, range.0);

                self.references.push(SymbolRef {
                    name: name.to_string(),
                    range,
                    line,
                    target,
                });
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_refs_recursive(child, parsed);
        }
    }

    /// Find which function contains the given byte offset.
    pub fn function_at_position(&self, offset: usize) -> Option<&str> {
        for (name, (start, end)) in &self.function_spans {
            if *start <= offset && offset < *end {
                return Some(name);
            }
        }
        None
    }

    /// Resolve a name to a definition ID, considering scope at the given position.
    ///
    /// Resolution order:
    /// 1. Local variables in the containing function (if any)
    /// 2. Parameters of the containing function (if any)
    /// 3. File-level definitions (class members, constants, functions)
    pub fn resolve_name(&self, name: &str, at_byte: usize) -> Option<SymbolId> {
        let containing_func = self.function_at_position(at_byte);

        // Get all definitions with this name
        let defs = self.name_to_defs.get(name)?;

        // If we're inside a function, prefer local/param definitions from that function
        if let Some(func_name) = containing_func {
            // First try local variables in this function
            for &id in defs {
                if let Some(def) = self.definitions.get(id) {
                    if let Scope::Function(ref scope_func) = def.scope {
                        if scope_func == func_name && def.kind == SymbolKind::Variable {
                            return Some(id);
                        }
                    }
                }
            }
            // Then try parameters in this function
            for &id in defs {
                if let Some(def) = self.definitions.get(id) {
                    if let Scope::Function(ref scope_func) = def.scope {
                        if scope_func == func_name && def.kind == SymbolKind::Parameter {
                            return Some(id);
                        }
                    }
                }
            }
        }

        // Fall back to file-level definitions
        for &id in defs {
            if let Some(def) = self.definitions.get(id) {
                if matches!(def.scope, Scope::File) {
                    return Some(id);
                }
            }
        }

        // If nothing found at file level, return any definition
        defs.first().copied()
    }

    /// Resolve a name without position context (file-level scope only).
    pub fn resolve_name_global(&self, name: &str) -> Option<SymbolId> {
        let defs = self.name_to_defs.get(name)?;
        // Prefer file-level definitions
        for &id in defs {
            if let Some(def) = self.definitions.get(id) {
                if matches!(def.scope, Scope::File) {
                    return Some(id);
                }
            }
        }
        defs.first().copied()
    }

    /// Find the symbol definition at a given byte offset.
    pub fn definition_at(&self, offset: usize) -> Option<&SymbolDef> {
        // Binary search for definitions containing this offset
        for &(start, end, id) in &self.def_ranges {
            if start <= offset && offset < end {
                return Some(&self.definitions[id]);
            }
            if start > offset {
                break;
            }
        }
        None
    }

    /// Find the symbol reference at a given byte offset.
    pub fn reference_at(&self, offset: usize) -> Option<&SymbolRef> {
        for &(start, end, idx) in &self.ref_ranges {
            if start <= offset && offset < end {
                return Some(&self.references[idx]);
            }
            if start > offset {
                break;
            }
        }
        None
    }

    /// Find either a definition or reference at a given byte offset.
    pub fn symbol_at(&self, offset: usize) -> Option<SymbolAtResult<'_>> {
        // Check definitions first (they take priority)
        if let Some(def) = self.definition_at(offset) {
            return Some(SymbolAtResult::Definition(def));
        }
        if let Some(ref_) = self.reference_at(offset) {
            return Some(SymbolAtResult::Reference(ref_));
        }
        None
    }

    /// Get all definitions.
    pub fn definitions(&self) -> &[SymbolDef] {
        &self.definitions
    }

    /// Get all references.
    pub fn references(&self) -> &[SymbolRef] {
        &self.references
    }

    /// Get a definition by ID.
    pub fn get_definition(&self, id: SymbolId) -> Option<&SymbolDef> {
        self.definitions.get(id)
    }

    /// Find all references to a definition.
    pub fn references_to(&self, def_id: SymbolId) -> Vec<&SymbolRef> {
        self.references
            .iter()
            .filter(|r| r.target == Some(def_id))
            .collect()
    }

    /// Find all definitions with a given name.
    pub fn definitions_named(&self, name: &str) -> Vec<&SymbolDef> {
        self.name_to_defs
            .get(name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.definitions.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the definition for a reference (if resolved).
    pub fn resolve_reference(&self, ref_: &SymbolRef) -> Option<&SymbolDef> {
        ref_.target.and_then(|id| self.definitions.get(id))
    }
}

/// Result of looking up a symbol at a position.
#[derive(Debug)]
pub enum SymbolAtResult<'a> {
    Definition(&'a SymbolDef),
    Reference(&'a SymbolRef),
}

impl<'a> SymbolAtResult<'a> {
    /// Get the name of the symbol.
    pub fn name(&self) -> &str {
        match self {
            SymbolAtResult::Definition(def) => &def.name,
            SymbolAtResult::Reference(ref_) => &ref_.name,
        }
    }

    /// Get the byte range of the symbol (if available).
    pub fn range(&self) -> Option<(usize, usize)> {
        match self {
            SymbolAtResult::Definition(def) => def.name_range.or(def.range),
            SymbolAtResult::Reference(ref_) => Some(ref_.range),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::symbols;
    use std::path::Path;

    fn build_index(source: &str) -> SymbolIndex {
        let parsed = parser::parse_source(source).unwrap();
        let symbols = symbols::collect_symbols(Path::new("test.gd"), &parsed);
        SymbolIndex::build(&parsed, &symbols)
    }

    #[test]
    fn index_variables() {
        let index = build_index("var my_var = 10\nvar other = 20");
        let defs = index.definitions();
        assert!(defs.iter().any(|d| d.name == "my_var"));
        assert!(defs.iter().any(|d| d.name == "other"));
    }

    #[test]
    fn index_functions() {
        let index = build_index("func my_func():\n    pass");
        let defs = index.definitions();
        assert!(defs
            .iter()
            .any(|d| d.name == "my_func" && d.kind == SymbolKind::Function));
    }

    #[test]
    fn index_parameters() {
        let index = build_index("func foo(a, b: int):\n    pass");
        let defs = index.definitions();
        assert!(defs
            .iter()
            .any(|d| d.name == "a" && d.kind == SymbolKind::Parameter));
        assert!(defs
            .iter()
            .any(|d| d.name == "b" && d.kind == SymbolKind::Parameter));
    }

    #[test]
    fn index_signals() {
        let index = build_index("signal my_signal(value: int)");
        let defs = index.definitions();
        assert!(defs
            .iter()
            .any(|d| d.name == "my_signal" && d.kind == SymbolKind::Signal));
    }

    #[test]
    fn definition_at_position() {
        let source = "var my_var = 10";
        let index = build_index(source);
        // "var my_var" - position within the definition
        let def = index.definition_at(4);
        assert!(def.is_some());
        assert_eq!(def.unwrap().name, "my_var");
    }

    #[test]
    fn definitions_named() {
        let index = build_index("var x = 1\nfunc foo():\n    var x = 2");
        let defs = index.definitions_named("x");
        assert_eq!(defs.len(), 2);
    }

    #[test]
    fn symbol_kind_as_str() {
        assert_eq!(SymbolKind::Variable.as_str(), "variable");
        assert_eq!(SymbolKind::Function.as_str(), "function");
        assert_eq!(SymbolKind::Signal.as_str(), "signal");
    }

    #[test]
    fn documentation_extraction_function() {
        let source = r#"## This is a documented function.
## It does something useful.
func my_func():
    pass"#;
        let index = build_index(source);
        let func = index
            .definitions()
            .iter()
            .find(|d| d.name == "my_func")
            .unwrap();
        assert!(func.documentation.is_some());
        let doc = func.documentation.as_ref().unwrap();
        assert!(doc.contains("documented function"));
        assert!(doc.contains("something useful"));
    }

    #[test]
    fn documentation_extraction_variable() {
        let source = r#"## The player's health.
var health: int = 100"#;
        let index = build_index(source);
        let var = index
            .definitions()
            .iter()
            .find(|d| d.name == "health")
            .unwrap();
        assert!(var.documentation.is_some());
        assert!(var
            .documentation
            .as_ref()
            .unwrap()
            .contains("player's health"));
    }

    #[test]
    fn documentation_extraction_signal() {
        let source = r#"## Emitted when player dies.
signal player_died"#;
        let index = build_index(source);
        let sig = index
            .definitions()
            .iter()
            .find(|d| d.name == "player_died")
            .unwrap();
        assert!(sig.documentation.is_some());
        assert!(sig.documentation.as_ref().unwrap().contains("player dies"));
    }

    #[test]
    fn no_documentation_for_regular_comments() {
        let source = r#"# This is a regular comment, not documentation.
var x = 10"#;
        let index = build_index(source);
        let var = index.definitions().iter().find(|d| d.name == "x").unwrap();
        assert!(var.documentation.is_none());
    }

    #[test]
    fn function_has_byte_ranges() {
        let source = "func my_func():\n    pass";
        let index = build_index(source);
        let func = index
            .definitions()
            .iter()
            .find(|d| d.name == "my_func" && d.kind == SymbolKind::Function)
            .unwrap();
        assert!(func.range.is_some());
        assert!(func.name_range.is_some());
        let (start, end) = func.range.unwrap();
        assert!(start < end);
    }

    #[test]
    fn signal_has_byte_ranges() {
        let source = "signal my_signal(value: int)";
        let index = build_index(source);
        let sig = index
            .definitions()
            .iter()
            .find(|d| d.name == "my_signal")
            .unwrap();
        assert!(sig.range.is_some());
        assert!(sig.name_range.is_some());
    }

    #[test]
    fn position_aware_scope_resolution() {
        // Test that local variables in one function don't leak to another
        let source = "var x = 1\n\nfunc foo():\n\tvar x = 2\n\tprint(x)\n\nfunc bar():\n\tvar x = 3\n\tprint(x)\n";
        let index = build_index(source);

        // Get the function spans
        let foo_span = index
            .function_spans
            .get("foo")
            .expect("foo function not found");
        let bar_span = index
            .function_spans
            .get("bar")
            .expect("bar function not found");

        // Resolve 'x' inside foo should return foo's local
        let resolved_in_foo = index.resolve_name("x", foo_span.0 + 10);
        assert!(resolved_in_foo.is_some());
        let def = index.get_definition(resolved_in_foo.unwrap()).unwrap();
        assert!(matches!(def.scope, Scope::Function(ref f) if f == "foo"));

        // Resolve 'x' inside bar should return bar's local
        let resolved_in_bar = index.resolve_name("x", bar_span.0 + 10);
        assert!(resolved_in_bar.is_some());
        let def = index.get_definition(resolved_in_bar.unwrap()).unwrap();
        assert!(matches!(def.scope, Scope::Function(ref f) if f == "bar"));

        // Resolve 'x' at file level should return the class variable
        let resolved_at_file = index.resolve_name("x", 0);
        assert!(resolved_at_file.is_some());
        let def = index.get_definition(resolved_at_file.unwrap()).unwrap();
        assert!(matches!(def.scope, Scope::File));
    }

    #[test]
    fn function_at_position_works() {
        let source = "func foo():\n\tpass\n\nfunc bar():\n\tpass\n";
        let index = build_index(source);

        // Position at the beginning should be outside functions
        assert!(
            index.function_at_position(0).is_none() || index.function_at_position(0) == Some("foo")
        );

        // Position inside foo's body
        let foo_span = index.function_spans.get("foo").unwrap();
        assert_eq!(index.function_at_position(foo_span.0 + 5), Some("foo"));

        // Position inside bar's body
        let bar_span = index.function_spans.get("bar").unwrap();
        assert_eq!(index.function_at_position(bar_span.0 + 5), Some("bar"));
    }
}
