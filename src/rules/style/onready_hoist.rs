use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, Fix, Rule, RuleContext, Severity, TextEdit};

const RULE_ID: &str = "style/onready-hoist";

pub struct OnreadyHoist;

impl Rule for OnreadyHoist {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Member variable initialized with node path should use @onready"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "style"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_onready_hoist(ctx.parsed, &mut diagnostics);
        diagnostics
    }
}

/// Info about a member variable that could be hoisted
struct HoistableVar {
    name: String,
    /// Byte position where @onready should be inserted (start of var statement)
    decl_start_byte: usize,
    /// Byte range of the full variable statement (for replacement)
    decl_end_byte: usize,
    /// The existing declaration text (e.g., "var player" or "var enemy: Node")
    decl_text: String,
}

/// Info about an assignment in _ready() that could be hoisted
struct ReadyAssignment {
    var_name: String,
    /// The node path being assigned (e.g., "$Player")
    node_path: String,
    /// Line of the assignment
    line: usize,
    /// Byte range of the full expression_statement to remove
    stmt_start_byte: usize,
    stmt_end_byte: usize,
}

fn check_onready_hoist(parsed: &ParsedFile, diagnostics: &mut Vec<Diagnostic>) {
    let root = parsed.root_node();
    let source = parsed.source();

    // Case 1: Find member variables initialized with $Node but without @onready
    for var_stmt in parser::find_nodes_by_kind(root, "variable_statement") {
        // Skip if inside a function (local variable)
        if is_inside_function(var_stmt) {
            continue;
        }

        // Skip if already has @onready or @export
        if has_onready_annotation(var_stmt, parsed) || has_export_annotation(var_stmt, parsed) {
            continue;
        }

        // Check if the initializer expression contains get_node ($Path).
        // Only search the initializer, not setter/getter bodies.
        if !initializer_has_get_node(var_stmt) {
            continue;
        }

        let var_name = var_stmt
            .child_by_field_name("name")
            .map(|n| parsed.node_text(n))
            .unwrap_or("unknown");

        let line = var_stmt.start_position().row + 1;

        let mut diag = Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            format!(
                "Variable `{}` initialized with node path should use @onready.",
                var_name
            ),
            line,
        )
        .with_note("Without @onready, the node may not exist yet at initialization time.");

        // Create fix: insert "@onready " at start of statement
        let fix = Fix::new(
            format!("Add @onready to `{}`", var_name),
            vec![TextEdit {
                start_byte: var_stmt.start_byte(),
                end_byte: var_stmt.start_byte(),
                replacement: "@onready ".to_string(),
            }],
        );
        diag = diag.with_fix(fix);

        diagnostics.push(diag);
    }

    // Case 2: Find variables declared at class level and assigned in _ready()
    // Collect member variable declarations without initializers
    let mut hoistable_vars: Vec<HoistableVar> = Vec::new();

    for var_stmt in parser::find_nodes_by_kind(root, "variable_statement") {
        if is_inside_function(var_stmt) {
            continue;
        }
        if has_onready_annotation(var_stmt, parsed) {
            continue;
        }

        let var_name = match var_stmt.child_by_field_name("name") {
            Some(n) => parsed.node_text(n).to_string(),
            None => continue,
        };

        // Check if it has an initializer (look for get_node or any value)
        let has_init = !parser::find_nodes_by_kind(var_stmt, "get_node").is_empty()
            || has_value_initializer(var_stmt);

        // We're interested in vars WITHOUT initializers for hoisting from _ready
        if has_init {
            continue;
        }

        hoistable_vars.push(HoistableVar {
            name: var_name,
            decl_start_byte: var_stmt.start_byte(),
            decl_end_byte: var_stmt.end_byte(),
            decl_text: parsed.node_text(var_stmt).to_string(),
        });
    }

    if hoistable_vars.is_empty() {
        return;
    }

    // Find _ready() function and look for assignments to our hoistable vars
    let ready_assignments = find_ready_assignments(root, parsed, &hoistable_vars);

    for assignment in ready_assignments {
        // Find the corresponding variable declaration
        let var_info = match hoistable_vars
            .iter()
            .find(|v| v.name == assignment.var_name)
        {
            Some(v) => v,
            None => continue,
        };

        let mut diag = Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            format!(
                "Variable `{}` assigned node path in _ready() can be hoisted to @onready.",
                assignment.var_name
            ),
            assignment.line,
        )
        .with_note("Using @onready is more idiomatic and keeps initialization with declaration.");

        // Create multi-edit fix:
        // 1. Replace the variable declaration with @onready version including initializer
        // 2. Remove the assignment from _ready()
        if let Some(fix) = make_hoist_fix(var_info, &assignment, source) {
            diag = diag.with_fix(fix);
        }

        diagnostics.push(diag);
    }
}

/// Check if a node is inside a function body
fn is_inside_function(node: tree_sitter::Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "function_definition" || parent.kind() == "constructor_definition" {
            return true;
        }
        current = parent.parent();
    }
    false
}

/// Check if a variable_statement has a specific annotation (e.g., "onready", "export").
fn has_annotation(var_stmt: tree_sitter::Node, parsed: &ParsedFile, name: &str) -> bool {
    let mut cursor = var_stmt.walk();
    for child in var_stmt.children(&mut cursor) {
        if child.kind() == "annotations" {
            let mut inner_cursor = child.walk();
            for annotation in child.children(&mut inner_cursor) {
                if annotation.kind() == "annotation" {
                    let mut ann_cursor = annotation.walk();
                    for ann_child in annotation.children(&mut ann_cursor) {
                        if ann_child.kind() == "identifier" && parsed.node_text(ann_child) == name {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

fn has_onready_annotation(var_stmt: tree_sitter::Node, parsed: &ParsedFile) -> bool {
    has_annotation(var_stmt, parsed, "onready")
}

fn has_export_annotation(var_stmt: tree_sitter::Node, parsed: &ParsedFile) -> bool {
    has_annotation(var_stmt, parsed, "export")
}

/// Check if the initializer expression of a variable_statement contains get_node ($Path).
/// Only checks direct children of the variable_statement, NOT setter/getter bodies.
fn initializer_has_get_node(var_stmt: tree_sitter::Node) -> bool {
    let mut cursor = var_stmt.walk();
    for child in var_stmt.children(&mut cursor) {
        // Skip annotations, name, type, setget (setter/getter bodies)
        match child.kind() {
            "annotations" | "name" | "type" | "setget" => continue,
            _ => {}
        }
        if child.kind() == "get_node" {
            return true;
        }
        if !parser::find_nodes_by_kind(child, "get_node").is_empty() {
            return true;
        }
    }
    false
}

/// Check if variable_statement has a value initializer (not just type annotation)
fn has_value_initializer(var_stmt: tree_sitter::Node) -> bool {
    let mut cursor = var_stmt.walk();
    for child in var_stmt.children(&mut cursor) {
        // Look for common value node types
        match child.kind() {
            "integer" | "float" | "string" | "true" | "false" | "null" | "array" | "dictionary"
            | "call" | "identifier" | "binary_operator" | "unary_operator" | "get_node" => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Find assignments to hoistable variables within _ready() function
fn find_ready_assignments(
    root: tree_sitter::Node,
    parsed: &ParsedFile,
    hoistable_vars: &[HoistableVar],
) -> Vec<ReadyAssignment> {
    let mut assignments = Vec::new();

    // Find _ready function
    let func_defs = parser::find_nodes_by_kind(root, "function_definition");
    let ready_func = func_defs.iter().find(|f| {
        f.child_by_field_name("name")
            .map(|n| parsed.node_text(n) == "_ready")
            .unwrap_or(false)
    });

    let ready_func = match ready_func {
        Some(f) => *f,
        None => return assignments,
    };

    // Find body
    let body = match ready_func.child_by_field_name("body") {
        Some(b) => b,
        None => return assignments,
    };

    // Look for expression_statements containing assignments
    let expr_stmts = parser::find_nodes_by_kind(body, "expression_statement");

    for expr_stmt in expr_stmts {
        // Find assignment within
        let assignment_nodes = parser::find_nodes_by_kind(expr_stmt, "assignment");
        for assign in assignment_nodes {
            // Get left side (should be identifier matching our var)
            let left = match assign.child(0) {
                Some(n) if n.kind() == "identifier" => parsed.node_text(n),
                _ => continue,
            };

            // Check if this is one of our hoistable vars
            if !hoistable_vars.iter().any(|v| v.name == left) {
                continue;
            }

            // Get right side - should be get_node
            let get_nodes = parser::find_nodes_by_kind(assign, "get_node");
            if get_nodes.is_empty() {
                continue;
            }

            let node_path = parsed.node_text(get_nodes[0]).to_string();

            assignments.push(ReadyAssignment {
                var_name: left.to_string(),
                node_path,
                line: expr_stmt.start_position().row + 1,
                stmt_start_byte: expr_stmt.start_byte(),
                stmt_end_byte: expr_stmt.end_byte(),
            });
        }
    }

    assignments
}

/// Create a multi-edit fix that hoists an assignment from _ready() to @onready
fn make_hoist_fix(
    var_info: &HoistableVar,
    assignment: &ReadyAssignment,
    source: &str,
) -> Option<Fix> {
    let bytes = source.as_bytes();

    // Edit 1: Replace the variable declaration with @onready version
    // e.g., "var player" -> "@onready var player = $Player"
    // or "var enemy: Node" -> "@onready var enemy: Node = $Enemy"
    let new_decl = format!("@onready {} = {}", var_info.decl_text, assignment.node_path);

    // Edit 2: Remove the assignment line from _ready()
    // Extend to include the full line (including newline)
    let mut remove_start = assignment.stmt_start_byte;
    let mut remove_end = assignment.stmt_end_byte;

    // Extend start to beginning of line (skip leading whitespace will be included)
    while remove_start > 0 && bytes[remove_start - 1] != b'\n' {
        remove_start -= 1;
    }

    // Extend end to include newline
    while remove_end < bytes.len() && bytes[remove_end] != b'\n' {
        remove_end += 1;
    }
    if remove_end < bytes.len() && bytes[remove_end] == b'\n' {
        remove_end += 1;
    }

    Some(Fix::new(
        format!(
            "Hoist `{}` assignment to @onready declaration",
            var_info.name
        ),
        vec![
            // Edit 1: Replace declaration
            TextEdit {
                start_byte: var_info.decl_start_byte,
                end_byte: var_info.decl_end_byte,
                replacement: new_decl,
            },
            // Edit 2: Remove assignment from _ready()
            TextEdit {
                start_byte: remove_start,
                end_byte: remove_end,
                replacement: String::new(),
            },
        ],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse(source: &str) -> ParsedFile {
        parser::parse_source(source).unwrap()
    }

    #[test]
    fn detects_missing_onready_simple() {
        let source = r#"
extends Node
var label = $Label
"#;
        let parsed = parse(source);
        let mut diagnostics = Vec::new();
        check_onready_hoist(&parsed, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("label"));
        assert!(diagnostics[0].fix.is_some());

        // Verify it's a single-edit fix
        let fix = diagnostics[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits.len(), 1);
        assert_eq!(fix.edits[0].replacement, "@onready ");
    }

    #[test]
    fn ignores_existing_onready() {
        let source = r#"
extends Node
@onready var sprite = $Sprite
"#;
        let parsed = parse(source);
        let mut diagnostics = Vec::new();
        check_onready_hoist(&parsed, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn detects_hoistable_from_ready() {
        let source = r#"
extends Node
var player

func _ready():
    player = $Player
    print("ready")
"#;
        let parsed = parse(source);
        let mut diagnostics = Vec::new();
        check_onready_hoist(&parsed, &mut diagnostics);

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("hoisted"));
        assert!(diagnostics[0].fix.is_some());

        // Verify it's a multi-edit fix
        let fix = diagnostics[0].fix.as_ref().unwrap();
        assert_eq!(fix.edits.len(), 2, "Should have 2 edits for hoist fix");
    }

    #[test]
    fn ignores_local_variables() {
        let source = r#"
extends Node
func test():
    var label = $Label
"#;
        let parsed = parse(source);
        let mut diagnostics = Vec::new();
        check_onready_hoist(&parsed, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_non_node_path_assignments() {
        let source = r#"
extends Node
var counter

func _ready():
    counter = 0
"#;
        let parsed = parse(source);
        let mut diagnostics = Vec::new();
        check_onready_hoist(&parsed, &mut diagnostics);

        assert!(diagnostics.is_empty());
    }
}
