use crate::classdb::ClassDb;
use crate::parser::{self, ParsedFile};
use crate::rules::helpers;
use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/orphan-node";

/// Methods that "sink" a node (prevent it from being orphaned).
const SINK_METHODS: &[&str] = &[
    "add_child",
    "add_sibling",
    "call_deferred",
    "queue_free",
    "free",
];

pub struct OrphanNode;

impl Rule for OrphanNode {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Node created but never added to the scene tree (memory leak)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_orphan_nodes(
            ctx.parsed,
            ctx.all_file_symbols,
            ctx.class_db,
            &mut diagnostics,
        );
        diagnostics
    }
}

fn check_orphan_nodes(
    parsed: &ParsedFile,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let funcs = parser::find_nodes_by_kind(root, "function_definition");

    for func in funcs {
        check_function_orphans(parsed, func, all_file_symbols, class_db, diagnostics);
    }
}

fn check_function_orphans(
    parsed: &ParsedFile,
    func: tree_sitter::Node,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut cursor = func.walk();
    let body = func.children(&mut cursor).find(|c| c.kind() == "body");

    let body = match body {
        Some(b) => b,
        None => return,
    };

    let mut body_cursor = body.walk();
    let statements: Vec<_> = body.children(&mut body_cursor).collect();

    for (i, stmt) in statements.iter().enumerate() {
        // Look for variable declarations with .new() or .instantiate() calls
        if stmt.kind() == "variable_statement"
            && has_node_creation(parsed, *stmt, all_file_symbols, class_db)
        {
            let var_name = get_var_name(parsed, *stmt);
            if let Some(name) = var_name {
                let remaining = &statements[i + 1..];
                if !has_sink(parsed, name, remaining) {
                    diagnostics.push(
                            Diagnostic::new(
                                RULE_ID,
                                Severity::Warning,
                                format!(
                                    "Node assigned to `{}` is never added to the scene tree (potential memory leak).",
                                    name
                                ),
                                stmt.start_position().row + 1,
                            )
                            .span(
                                stmt.start_position().column,
                                stmt.end_position().row + 1,
                                stmt.end_position().column,
                            ),
                        );
                }
            }
        }

        // Look for expression statements with unassigned .new()/.instantiate()
        if stmt.kind() == "expression_statement"
            && has_standalone_node_creation(parsed, *stmt, all_file_symbols, class_db)
        {
            diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        "Node created but not assigned to a variable or added to the scene tree (memory leak).".to_string(),
                        stmt.start_position().row + 1,
                    )
                    .span(
                        stmt.start_position().column,
                        stmt.end_position().row + 1,
                        stmt.end_position().column,
                    ),
                );
        }
    }
}

/// Check if a statement contains a node creation pattern (ClassName.new() or something.instantiate())
fn has_node_creation(
    parsed: &ParsedFile,
    node: tree_sitter::Node,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> bool {
    // Look for attribute nodes with attribute_call children named "new" or "instantiate"
    let attrs = parser::find_nodes_by_kind(node, "attribute");
    for attr in attrs {
        if is_node_creation_attr(parsed, attr, all_file_symbols, class_db) {
            return true;
        }
    }
    false
}

/// Check if an expression_statement has a standalone node creation
/// (not wrapped in add_child or similar)
fn has_standalone_node_creation(
    parsed: &ParsedFile,
    stmt: tree_sitter::Node,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> bool {
    // The expression_statement's direct child should be the attribute node
    let mut cursor = stmt.walk();
    for child in stmt.children(&mut cursor) {
        if child.kind() == "attribute"
            && is_node_creation_attr(parsed, child, all_file_symbols, class_db)
        {
            return true;
        }
        // Also handle calls that wrap creation (shouldn't be flagged)
        if child.kind() == "call" {
            // If the outer call is add_child etc., don't flag
            let func_name = get_call_func_name(parsed, child);
            if let Some(name) = func_name {
                if SINK_METHODS.contains(&name.as_str()) {
                    return false;
                }
            }
        }
    }
    false
}

/// Check if an attribute node represents a Node creation (ClassName.new() or .instantiate())
fn is_node_creation_attr(
    parsed: &ParsedFile,
    attr: tree_sitter::Node,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> bool {
    let mut cursor = attr.walk();
    let children: Vec<_> = attr.children(&mut cursor).collect();

    if children.len() < 2 {
        return false;
    }

    // Look for attribute_call child with "new" or "instantiate"
    for child in &children {
        if child.kind() == "attribute_call" {
            let mut call_cursor = child.walk();
            let call_children: Vec<_> = child.children(&mut call_cursor).collect();

            if call_children.is_empty() {
                continue;
            }

            let method_name = parsed.node_text(call_children[0]);

            if method_name == "new" {
                // Check if the receiver (first child of attr) is a Node subclass
                // Skip if receiver is not an identifier (e.g., could be a call result)
                if children[0].kind() != "identifier" {
                    continue;
                }
                let receiver = parsed.node_text(children[0]);
                return class_db.is_subclass_of(receiver, "Node")
                    || helpers::is_user_subclass_of(receiver, "Node", all_file_symbols, class_db);
            }

            if method_name == "instantiate" {
                return true;
            }
        }
    }

    false
}

fn get_call_func_name(parsed: &ParsedFile, call: tree_sitter::Node) -> Option<String> {
    let mut cursor = call.walk();
    let children: Vec<_> = call.children(&mut cursor).collect();
    if children.is_empty() {
        return None;
    }
    let func_node = children[0];
    if func_node.kind() == "identifier" {
        return Some(parsed.node_text(func_node).to_string());
    }
    None
}

fn get_var_name<'a>(parsed: &'a ParsedFile, stmt: tree_sitter::Node) -> Option<&'a str> {
    let mut cursor = stmt.walk();
    for child in stmt.children(&mut cursor) {
        if child.kind() == "name" {
            return Some(parsed.node_text(child));
        }
    }
    None
}

/// Collect all names that a node variable is assigned to (aliases).
/// For example, if `belt` is created and later `asteroid_mesh = belt`,
/// then `asteroid_mesh` is an alias and sinks for it count too.
fn collect_aliases<'a>(
    parsed: &'a ParsedFile,
    var_name: &str,
    remaining: &[tree_sitter::Node],
) -> Vec<&'a str> {
    let mut aliases = Vec::new();
    for stmt in remaining {
        let assignments = parser::find_nodes_by_kind(*stmt, "assignment");
        for assign in assignments {
            let mut cursor = assign.walk();
            let children: Vec<_> = assign.children(&mut cursor).collect();
            if children.len() >= 3 {
                let rhs = children.last().unwrap();
                if rhs.kind() == "identifier" && parsed.node_text(*rhs) == var_name {
                    let lhs = children[0];
                    let lhs_text = parsed.node_text(lhs);
                    // Track both simple identifiers and member assignments
                    if lhs.kind() == "identifier" {
                        aliases.push(lhs_text);
                    }
                }
            }
        }
    }
    aliases
}

/// Check if remaining statements contain a "sink" for the variable.
fn has_sink(parsed: &ParsedFile, var_name: &str, remaining: &[tree_sitter::Node]) -> bool {
    // Collect aliases (other variables this node is assigned to)
    let aliases = collect_aliases(parsed, var_name, remaining);

    // Check sinks for the original name and all aliases
    let mut names_to_check = vec![var_name];
    for alias in &aliases {
        names_to_check.push(alias);
    }

    for name in &names_to_check {
        if has_sink_for_name(parsed, name, remaining) {
            return true;
        }
    }

    false
}

/// Check if remaining statements contain a "sink" for a specific name.
fn has_sink_for_name(parsed: &ParsedFile, var_name: &str, remaining: &[tree_sitter::Node]) -> bool {
    for stmt in remaining {
        // Check for add_child(var), add_sibling(var), etc. by looking for identifier nodes
        if has_identifier_as_arg(parsed, *stmt, var_name, SINK_METHODS) {
            return true;
        }

        // Check for var.queue_free() or var.free()
        let text = parsed.node_text(*stmt);
        if text.contains(&format!("{}.queue_free()", var_name))
            || text.contains(&format!("{}.free()", var_name))
        {
            return true;
        }

        // Check for return var (by looking for identifier in return statement)
        if stmt.kind() == "return_statement" {
            let identifiers = parser::find_nodes_by_kind(*stmt, "identifier");
            for id in identifiers {
                if parsed.node_text(id) == var_name {
                    return true;
                }
            }
        }

        // Check for self.member = var (storing in member variable)
        // Look for assignment where RHS is the variable
        let assignments = parser::find_nodes_by_kind(*stmt, "assignment");
        for assign in assignments {
            let mut cursor = assign.walk();
            let children: Vec<_> = assign.children(&mut cursor).collect();
            // assignment: LHS = RHS
            // children should be [lhs, "=", rhs]
            if children.len() >= 3 {
                let rhs = children.last().unwrap();
                if rhs.kind() == "identifier" && parsed.node_text(*rhs) == var_name {
                    let lhs_text = parsed.node_text(children[0]);
                    if lhs_text.starts_with("self.") {
                        return true;
                    }
                }
            }
        }

        // Check for passing as function argument (identifier, not string literal)
        // This handles both `call` nodes (add_child(n)) and `attribute_call` nodes (obj.add_child(n))
        for kind in &["call", "attribute_call"] {
            let calls = parser::find_nodes_by_kind(*stmt, kind);
            for call in calls {
                let mut call_cursor = call.walk();
                let call_children: Vec<_> = call.children(&mut call_cursor).collect();
                if let Some(args) = call_children.iter().find(|c| c.kind() == "arguments") {
                    let identifiers = parser::find_nodes_by_kind(*args, "identifier");
                    for id in identifiers {
                        if parsed.node_text(id) == var_name {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

/// Check if a statement has a call to one of the given methods with var_name as an argument.
fn has_identifier_as_arg(
    parsed: &ParsedFile,
    stmt: tree_sitter::Node,
    var_name: &str,
    methods: &[&str],
) -> bool {
    let calls = parser::find_nodes_by_kind(stmt, "call");
    for call in calls {
        let mut cursor = call.walk();
        let children: Vec<_> = call.children(&mut cursor).collect();
        if children.is_empty() {
            continue;
        }

        // Get function name
        let func_name = get_call_func_name(parsed, call);
        if let Some(name) = func_name {
            if methods.contains(&name.as_str()) {
                // Check if var_name is passed as an identifier argument
                if let Some(args) = children.iter().find(|c| c.kind() == "arguments") {
                    let identifiers = parser::find_nodes_by_kind(*args, "identifier");
                    for id in identifiers {
                        if parsed.node_text(id) == var_name {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
