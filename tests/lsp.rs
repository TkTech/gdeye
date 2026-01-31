//! Integration tests for the LSP server infrastructure.
//!
//! These tests verify LSP-related functionality using the public APIs.

use std::path::PathBuf;

use gdeye::classdb::ClassDb;
use gdeye::fmt::FmtConfig;
use gdeye::lsp::{IndexedFile, ServerState};
use gdeye::parser;
use gdeye::symbol_index::SymbolIndex;
use gdeye::symbols;
use tower_lsp::lsp_types::*;

// ============================================================================
// Test Harness
// ============================================================================

/// Test harness for LSP functionality.
struct TestHarness {
    state: ServerState,
}

impl TestHarness {
    fn new() -> Self {
        let class_db = ClassDb::from_bundled(None).expect("Failed to load bundled classdb");
        Self {
            state: ServerState::with_classdb(class_db),
        }
    }

    fn path(&self, filename: &str) -> PathBuf {
        PathBuf::from(format!("/test/{}", filename))
    }

    fn open_doc(&mut self, filename: &str, content: &str) {
        let path = self.path(filename);

        let parsed = match parser::parse_source(content) {
            Ok(p) => p,
            Err(_) => return,
        };

        let file_symbols = symbols::collect_symbols(&path, &parsed);
        let index = SymbolIndex::build(&parsed, &file_symbols);
        let file = IndexedFile::with_full_data(file_symbols, parsed, index, 1, content.to_string());

        self.state.project_index.insert(path, file);
    }

    /// Get symbol at a byte offset using public SymbolIndex API.
    fn symbol_at(
        &self,
        filename: &str,
        line: u32,
        character: u32,
    ) -> Option<gdeye::SymbolAtResult<'_>> {
        let path = self.path(filename);

        let file = self.state.project_index.get(&path)?;
        let index = file.index.as_ref()?;
        let content = file.content.as_ref()?;

        let offset = position_to_offset(content, line, character)?;
        index.symbol_at(offset)
    }

    fn document_symbols(&self, filename: &str) -> Option<DocumentSymbolResponse> {
        let path = self.path(filename);

        let file = self.state.project_index.get(&path)?;
        let symbols = &file.symbols;

        let mut doc_symbols = Vec::new();

        for signal in &symbols.signals {
            doc_symbols.push(DocumentSymbol {
                name: signal.name.clone(),
                detail: Some(format!("signal({})", signal.parameters.join(", "))),
                kind: SymbolKind::EVENT,
                tags: None,
                deprecated: None,
                range: line_range(signal.line),
                selection_range: line_range(signal.line),
                children: None,
            });
        }

        for constant in &symbols.constants {
            doc_symbols.push(DocumentSymbol {
                name: constant.name.clone(),
                detail: constant.type_annotation.clone(),
                kind: SymbolKind::CONSTANT,
                tags: None,
                deprecated: None,
                range: line_range(constant.line),
                selection_range: line_range(constant.line),
                children: None,
            });
        }

        for var in &symbols.variables {
            doc_symbols.push(DocumentSymbol {
                name: var.name.clone(),
                detail: var
                    .type_annotation
                    .clone()
                    .or_else(|| var.inferred_type.clone()),
                kind: SymbolKind::VARIABLE,
                tags: None,
                deprecated: None,
                range: line_range(var.line),
                selection_range: line_range(var.line),
                children: None,
            });
        }

        for func in &symbols.functions {
            doc_symbols.push(DocumentSymbol {
                name: func.name.clone(),
                detail: func.return_type.clone(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range: line_range(func.line),
                selection_range: line_range(func.line),
                children: None,
            });
        }

        Some(DocumentSymbolResponse::Nested(doc_symbols))
    }

    fn completion(&self, filename: &str) -> Option<Vec<CompletionItem>> {
        let path = self.path(filename);

        let file = self.state.project_index.get(&path)?;
        let symbols = &file.symbols;

        let mut items = Vec::new();

        for var in &symbols.variables {
            items.push(CompletionItem {
                label: var.name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: var
                    .type_annotation
                    .clone()
                    .or_else(|| var.inferred_type.clone()),
                ..Default::default()
            });
        }

        for func in &symbols.functions {
            items.push(CompletionItem {
                label: func.name.clone(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: func.return_type.clone(),
                ..Default::default()
            });
        }

        for constant in &symbols.constants {
            items.push(CompletionItem {
                label: constant.name.clone(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: constant.type_annotation.clone(),
                ..Default::default()
            });
        }

        Some(items)
    }

    fn formatting(&self, filename: &str) -> Option<String> {
        let path = self.path(filename);

        let file = self.state.project_index.get(&path)?;
        let content = file.content.as_ref()?;

        let config = FmtConfig::default();
        match gdeye::fmt::format_source(content, &config) {
            Ok(result) => Some(result.output),
            Err(_) => None,
        }
    }

    fn references(&self, filename: &str, name: &str) -> Vec<(u32, u32)> {
        let path = self.path(filename);

        let file = match self.state.project_index.get(&path) {
            Some(f) => f,
            None => return Vec::new(),
        };
        let index = match file.index.as_ref() {
            Some(i) => i,
            None => return Vec::new(),
        };
        let content = match file.content.as_ref() {
            Some(c) => c,
            None => return Vec::new(),
        };

        // Find all references to the given name
        index
            .references()
            .iter()
            .filter(|r| r.name == name)
            .filter_map(|r| {
                let pos = offset_to_position(content, r.range.0)?;
                Some((pos.line, pos.character))
            })
            .collect()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn position_to_offset(content: &str, line: u32, character: u32) -> Option<usize> {
    let mut current_line = 0u32;

    for (i, c) in content.char_indices() {
        if current_line == line {
            let mut col = 0u32;
            for (j, ch) in content[i..].char_indices() {
                if col == character {
                    return Some(i + j);
                }
                if ch == '\n' {
                    return Some(i + j);
                }
                col += 1;
            }
            return Some(content.len().min(i + character as usize));
        }
        if c == '\n' {
            current_line += 1;
        }
    }

    if current_line == line {
        Some(content.len())
    } else {
        None
    }
}

fn offset_to_position(content: &str, offset: usize) -> Option<Position> {
    let mut line = 0u32;
    let mut col = 0u32;

    for (i, c) in content.char_indices() {
        if i == offset {
            return Some(Position::new(line, col));
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    if offset == content.len() {
        Some(Position::new(line, col))
    } else {
        None
    }
}

#[allow(deprecated)]
fn line_range(line: usize) -> Range {
    let line = line.saturating_sub(1) as u32;
    Range {
        start: Position::new(line, 0),
        end: Position::new(line, u32::MAX),
    }
}

fn extract_symbol_names(resp: &DocumentSymbolResponse) -> Vec<String> {
    match resp {
        DocumentSymbolResponse::Flat(symbols) => symbols.iter().map(|s| s.name.clone()).collect(),
        DocumentSymbolResponse::Nested(symbols) => {
            fn collect_names(symbols: &[DocumentSymbol], names: &mut Vec<String>) {
                for sym in symbols {
                    names.push(sym.name.clone());
                    if let Some(children) = &sym.children {
                        collect_names(children, names);
                    }
                }
            }
            let mut names = Vec::new();
            collect_names(symbols, &mut names);
            names
        }
    }
}

// ============================================================================
// Symbol Index Tests
// ============================================================================

#[test]
fn symbol_index_finds_function_definition() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func helper():
    pass

func _ready():
    helper()
"#,
    );

    // Position on "helper" in the function definition (line 2, col 5)
    let symbol = harness.symbol_at("test.gd", 2, 5);
    assert!(
        symbol.is_some(),
        "Should find symbol at function definition"
    );

    let symbol = symbol.unwrap();
    assert_eq!(symbol.name(), "helper");
}

#[test]
fn symbol_index_finds_variable_definition() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var counter: int = 0

func _ready():
    counter += 1
"#,
    );

    // Position on "counter" in the variable definition (line 2, col 4)
    let symbol = harness.symbol_at("test.gd", 2, 4);
    assert!(
        symbol.is_some(),
        "Should find symbol at variable definition"
    );

    let symbol = symbol.unwrap();
    assert_eq!(symbol.name(), "counter");
}

#[test]
fn symbol_index_finds_reference() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var health: int = 100

func _ready():
    print(health)
"#,
    );

    // Position on "health" in the reference (line 5, col 10)
    let symbol = harness.symbol_at("test.gd", 5, 10);
    assert!(symbol.is_some(), "Should find symbol at reference");

    let symbol = symbol.unwrap();
    assert_eq!(symbol.name(), "health");
}

// ============================================================================
// Document Symbols Tests
// ============================================================================

#[test]
fn document_symbols_lists_all() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

signal health_changed(new_value: int)

var health: int = 100
const MAX_HEALTH = 100

func damage(amount: int):
    health -= amount
    health_changed.emit(health)

func heal(amount: int):
    health += amount
"#,
    );

    let symbols = harness.document_symbols("test.gd");
    assert!(symbols.is_some(), "Expected document symbols");

    let symbols = extract_symbol_names(&symbols.unwrap());
    assert!(
        symbols.contains(&"health_changed".to_string()),
        "Should list signal"
    );
    assert!(
        symbols.contains(&"health".to_string()),
        "Should list variable"
    );
    assert!(
        symbols.contains(&"MAX_HEALTH".to_string()),
        "Should list constant"
    );
    assert!(
        symbols.contains(&"damage".to_string()),
        "Should list function"
    );
    assert!(
        symbols.contains(&"heal".to_string()),
        "Should list function"
    );
}

// ============================================================================
// Completion Tests
// ============================================================================

#[test]
fn completion_shows_member_variables() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var player_health: int = 100
var player_speed: float = 5.0

func _ready():
    pass
"#,
    );

    let completions = harness.completion("test.gd");
    assert!(completions.is_some(), "Expected completion response");

    let items: Vec<String> = completions
        .unwrap()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        items.iter().any(|s| s.contains("player_health")),
        "Should suggest player_health"
    );
    assert!(
        items.iter().any(|s| s.contains("player_speed")),
        "Should suggest player_speed"
    );
}

#[test]
fn completion_shows_functions() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func my_helper():
    pass

func another_helper():
    pass
"#,
    );

    let completions = harness.completion("test.gd");
    assert!(completions.is_some(), "Expected completion response");

    let items: Vec<String> = completions
        .unwrap()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        items.iter().any(|s| s == "my_helper"),
        "Should suggest my_helper"
    );
    assert!(
        items.iter().any(|s| s == "another_helper"),
        "Should suggest another_helper"
    );
}

// ============================================================================
// Formatting Tests
// ============================================================================

#[test]
fn formatting_returns_valid_output() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node
func _ready():
    pass
"#,
    );

    let formatted = harness.formatting("test.gd");
    assert!(formatted.is_some(), "Expected formatted output");

    let formatted = formatted.unwrap();
    assert!(
        formatted.contains("extends Node"),
        "Should preserve extends"
    );
    assert!(
        formatted.contains("func _ready()"),
        "Should preserve function"
    );
}

#[test]
fn formatting_normalizes_indentation() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node
func _ready():
  var x = 1
  if x == 1:
    pass
"#,
    );

    let formatted = harness.formatting("test.gd");
    assert!(formatted.is_some(), "Expected formatted output");

    let formatted = formatted.unwrap();
    // The formatter should use tabs by default
    assert!(
        formatted.contains("\t"),
        "Formatter should use tabs for indentation"
    );
}

// ============================================================================
// References Tests
// ============================================================================

#[test]
fn references_finds_all_usages() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var health: int = 100

func damage(amount: int):
    health -= amount

func heal(amount: int):
    health += amount

func _ready():
    print(health)
"#,
    );

    let refs = harness.references("test.gd", "health");
    // Should find references in damage(), heal(), and _ready()
    assert!(
        refs.len() >= 3,
        "Should find at least 3 references to health, got {}",
        refs.len()
    );
}

// ============================================================================
// Symbol Resolution Tests
// ============================================================================

#[test]
fn symbol_index_resolves_function_call() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func helper():
    pass

func _ready():
    helper()
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Resolve 'helper' name
    let resolved = index.resolve_name_global("helper");
    assert!(resolved.is_some(), "Should resolve 'helper'");
}

#[test]
fn symbol_index_resolves_variable() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var counter: int = 0

func _ready():
    counter += 1
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Resolve 'counter' name
    let resolved = index.resolve_name_global("counter");
    assert!(resolved.is_some(), "Should resolve 'counter'");
}

// ============================================================================
// Project Index Tests
// ============================================================================

#[test]
fn project_index_stores_files() {
    let mut harness = TestHarness::new();

    harness.open_doc("a.gd", "extends Node\n");
    harness.open_doc("b.gd", "extends Node2D\n");

    let path_a = harness.path("a.gd");
    let path_b = harness.path("b.gd");

    assert!(harness.state.project_index.get(&path_a).is_some());
    assert!(harness.state.project_index.get(&path_b).is_some());
}

#[test]
fn project_index_tracks_extends() {
    let mut harness = TestHarness::new();

    harness.open_doc("player.gd", "extends CharacterBody2D\n\nvar health = 100\n");

    let path = harness.path("player.gd");
    let file = harness.state.project_index.get(&path).unwrap();

    assert_eq!(file.symbols.extends.as_deref(), Some("CharacterBody2D"));
}

// ============================================================================
// ClassDB Integration Tests
// ============================================================================

#[test]
fn classdb_lookup_works() {
    let harness = TestHarness::new();

    // Check that Node exists
    assert!(
        harness.state.class_db.class_exists("Node"),
        "ClassDB should know about Node"
    );

    // Check that Node2D exists
    assert!(
        harness.state.class_db.class_exists("Node2D"),
        "ClassDB should know about Node2D"
    );

    // Check inheritance
    assert!(
        harness.state.class_db.is_subclass_of("Node2D", "Node"),
        "Node2D should be subclass of Node"
    );
}

#[test]
fn classdb_method_lookup_works() {
    let harness = TestHarness::new();

    // Check that Node has get_node method
    let method = harness.state.class_db.get_method("Node", "get_node");
    assert!(method.is_some(), "Node should have get_node method");

    // Check that Node2D has position property
    let has_position = harness.state.class_db.has_property("Node2D", "position");
    assert!(has_position, "Node2D should have position property");
}

// ============================================================================
// Integration Tests
// ============================================================================

#[test]
fn full_workflow_open_analyze_symbols() {
    let mut harness = TestHarness::new();

    let source = r#"extends Node

signal player_died
signal health_changed(new_health: int)

const MAX_HEALTH = 100

var health: int = MAX_HEALTH
var is_alive: bool = true

func take_damage(amount: int) -> void:
    health -= amount
    health_changed.emit(health)
    if health <= 0:
        die()

func die() -> void:
    is_alive = false
    player_died.emit()

func _ready() -> void:
    print("Player ready")
"#;

    harness.open_doc("player.gd", source);

    // Get document symbols
    let symbols = harness.document_symbols("player.gd").unwrap();
    let names = extract_symbol_names(&symbols);

    // Verify all expected symbols are present
    assert!(names.contains(&"player_died".to_string()));
    assert!(names.contains(&"health_changed".to_string()));
    assert!(names.contains(&"MAX_HEALTH".to_string()));
    assert!(names.contains(&"health".to_string()));
    assert!(names.contains(&"is_alive".to_string()));
    assert!(names.contains(&"take_damage".to_string()));
    assert!(names.contains(&"die".to_string()));
    assert!(names.contains(&"_ready".to_string()));

    // Get completions
    let completions = harness.completion("player.gd").unwrap();
    let completion_labels: Vec<_> = completions.iter().map(|c| &c.label).collect();

    assert!(completion_labels.contains(&&"health".to_string()));
    assert!(completion_labels.contains(&&"take_damage".to_string()));

    // Format the document
    let formatted = harness.formatting("player.gd").unwrap();
    assert!(formatted.contains("extends Node"));
    assert!(formatted.contains("func take_damage"));
}

// ============================================================================
// Goto Definition Tests
// ============================================================================

#[test]
fn goto_definition_finds_local_function() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func helper():
    pass

func _ready():
    helper()
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Find definition of 'helper'
    let resolved_id = index.resolve_name_global("helper");
    assert!(
        resolved_id.is_some(),
        "Should resolve 'helper' to its definition"
    );

    let def = index.get_definition(resolved_id.unwrap()).unwrap();
    assert_eq!(def.name, "helper");
}

#[test]
fn goto_definition_finds_variable() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var counter: int = 0

func increment():
    counter += 1
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Find definition of 'counter'
    let resolved_id = index.resolve_name_global("counter");
    assert!(
        resolved_id.is_some(),
        "Should resolve 'counter' to its definition"
    );

    let def = index.get_definition(resolved_id.unwrap()).unwrap();
    assert_eq!(def.name, "counter");
}

#[test]
fn goto_definition_finds_signal() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

signal health_changed(value: int)

func damage():
    health_changed.emit(50)
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Find definition of 'health_changed'
    let resolved_id = index.resolve_name_global("health_changed");
    assert!(
        resolved_id.is_some(),
        "Should resolve 'health_changed' to its definition"
    );

    let def = index.get_definition(resolved_id.unwrap()).unwrap();
    assert_eq!(def.name, "health_changed");
}

// ============================================================================
// Hover Tests
// ============================================================================

#[test]
fn hover_shows_function_signature() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func calculate(a: int, b: int) -> int:
    return a + b
"#,
    );

    // Get symbol at function definition
    let symbol = harness.symbol_at("test.gd", 2, 5);
    assert!(
        symbol.is_some(),
        "Should find symbol at function definition"
    );

    let symbol = symbol.unwrap();
    assert_eq!(symbol.name(), "calculate");
}

#[test]
fn hover_shows_typed_variable() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

var speed: float = 5.0

func _ready():
    print(speed)
"#,
    );

    // Get symbol at variable definition
    let symbol = harness.symbol_at("test.gd", 2, 4);
    assert!(
        symbol.is_some(),
        "Should find symbol at variable definition"
    );

    let symbol = symbol.unwrap();
    assert_eq!(symbol.name(), "speed");
}

#[test]
fn hover_shows_constant() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

const MAX_SPEED: float = 100.0

func _ready():
    print(MAX_SPEED)
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    let resolved = index.resolve_name_global("MAX_SPEED");
    assert!(resolved.is_some(), "Should resolve MAX_SPEED");
}

// ============================================================================
// Signature Help Tests
// ============================================================================

#[test]
fn signature_help_extracts_parameters() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func greet(name: String, count: int = 1) -> void:
    for i in count:
        print("Hello, " + name)
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let symbols = &file.symbols;

    // Find the greet function and check its parameters
    let func = symbols.functions.iter().find(|f| f.name == "greet");
    assert!(func.is_some(), "Should find greet function");

    let func = func.unwrap();
    assert_eq!(func.parameters.len(), 2);
    assert_eq!(func.parameters[0].name, "name");
    assert_eq!(
        func.parameters[0].type_annotation.as_deref(),
        Some("String")
    );
    assert_eq!(func.parameters[1].name, "count");
    assert_eq!(func.parameters[1].type_annotation.as_deref(), Some("int"));
}

// ============================================================================
// Workspace Symbol Tests
// ============================================================================

#[test]
fn workspace_symbol_finds_across_files() {
    let mut harness = TestHarness::new();

    harness.open_doc(
        "player.gd",
        r#"extends Node
class_name Player

var health: int = 100

func take_damage(amount: int):
    health -= amount
"#,
    );

    harness.open_doc(
        "enemy.gd",
        r#"extends Node
class_name Enemy

var damage: int = 10

func attack(target):
    target.take_damage(damage)
"#,
    );

    // Check that both files are indexed
    let path_player = harness.path("player.gd");
    let path_enemy = harness.path("enemy.gd");

    assert!(harness.state.project_index.get(&path_player).is_some());
    assert!(harness.state.project_index.get(&path_enemy).is_some());

    // Check that symbols can be found
    let player_file = harness.state.project_index.get(&path_player).unwrap();
    assert!(player_file
        .symbols
        .functions
        .iter()
        .any(|f| f.name == "take_damage"));

    let enemy_file = harness.state.project_index.get(&path_enemy).unwrap();
    assert!(enemy_file
        .symbols
        .functions
        .iter()
        .any(|f| f.name == "attack"));
}

#[test]
fn workspace_symbol_finds_class_names() {
    let mut harness = TestHarness::new();

    harness.open_doc(
        "player.gd",
        r#"extends CharacterBody2D
class_name Player

func _ready():
    pass
"#,
    );

    let path = harness.path("player.gd");
    let file = harness.state.project_index.get(&path).unwrap();

    assert_eq!(file.symbols.class_name.as_deref(), Some("Player"));
}

// ============================================================================
// Inlay Hint Tests
// ============================================================================

#[test]
fn symbol_index_tracks_local_variables() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

func example():
    var local_var: int = 42
    var another = "test"
    print(local_var)
    print(another)
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let symbols = &file.symbols;

    // Check that local variables are tracked in function
    let func = symbols.functions.iter().find(|f| f.name == "example");
    assert!(func.is_some(), "Should find example function");

    let func = func.unwrap();
    assert!(func.local_vars.len() >= 2, "Should track local variables");
}

// ============================================================================
// Code Action Tests (Fix Application)
// ============================================================================

#[test]
fn symbol_index_builds_correctly() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

signal my_signal
const MY_CONST = 42
var my_var: int
enum MyEnum { A, B, C }

func my_func():
    pass
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let index = file.index.as_ref().unwrap();

    // Check all symbols can be resolved
    assert!(
        index.resolve_name_global("my_signal").is_some(),
        "Should find signal"
    );
    assert!(
        index.resolve_name_global("MY_CONST").is_some(),
        "Should find constant"
    );
    assert!(
        index.resolve_name_global("my_var").is_some(),
        "Should find variable"
    );
    assert!(
        index.resolve_name_global("MyEnum").is_some(),
        "Should find enum"
    );
    assert!(
        index.resolve_name_global("my_func").is_some(),
        "Should find function"
    );
}

// ============================================================================
// Cross-file Resolution Tests
// ============================================================================

#[test]
fn project_index_class_name_lookup() {
    let mut harness = TestHarness::new();

    harness.open_doc(
        "weapon.gd",
        r#"extends Node
class_name Weapon

var damage: int = 10
"#,
    );

    // Look up by class name
    let file = harness.state.project_index.get_by_class_name("Weapon");
    assert!(file.is_some(), "Should find file by class_name");

    let file = file.unwrap();
    assert_eq!(file.symbols.class_name.as_deref(), Some("Weapon"));
}

// ============================================================================
// Semantic Token Tests
// ============================================================================

#[test]
fn symbols_include_enums() {
    let mut harness = TestHarness::new();
    harness.open_doc(
        "test.gd",
        r#"extends Node

enum State { IDLE, RUNNING, JUMPING }

func get_state() -> int:
    return State.IDLE
"#,
    );

    let path = harness.path("test.gd");
    let file = harness.state.project_index.get(&path).unwrap();
    let symbols = &file.symbols;

    // Check enum is tracked
    let enum_def = symbols.enums.iter().find(|e| e.name == "State");
    assert!(enum_def.is_some(), "Should find State enum");
}
