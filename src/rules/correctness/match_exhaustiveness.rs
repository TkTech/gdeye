use std::collections::HashSet;

use tree_sitter::Node;

use crate::classdb::ClassDb;
use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;
use crate::types::{resolve_expr_type, ExprTypeContext};

use super::super::helpers::effective_var_type;
use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/match-exhaustiveness";

pub struct MatchExhaustiveness;

impl Rule for MatchExhaustiveness {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Match statement on enum does not cover all variants"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_match_exhaustiveness(
            ctx.parsed,
            ctx.file_sym,
            ctx.all_file_symbols,
            ctx.class_db,
            &mut diagnostics,
        );
        diagnostics
    }
}

fn check_match_exhaustiveness(
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let match_stmts = parser::find_nodes_by_kind(root, "match_statement");

    for match_node in match_stmts {
        check_match_statement(
            match_node,
            parsed,
            file_sym,
            all_file_symbols,
            class_db,
            diagnostics,
        );
    }
}

fn check_match_statement(
    match_node: Node,
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Get the match expression (first named child that isn't match_body)
    let mut cursor = match_node.walk();
    let match_expr = match match_node
        .children(&mut cursor)
        .find(|c| c.is_named() && c.kind() != "match_body")
    {
        Some(e) => e,
        None => return,
    };

    // Build type context for expression resolution
    let extends_class = file_sym.extends.as_deref().unwrap_or("RefCounted");
    let member_vars: Vec<(String, Option<String>)> = file_sym
        .variables
        .iter()
        .map(|v| (v.name.clone(), effective_var_type(v)))
        .collect();
    let local_func_returns: Vec<(String, Option<String>)> = file_sym
        .functions
        .iter()
        .map(|f| (f.name.clone(), f.return_type.clone()))
        .collect();

    let type_ctx = ExprTypeContext {
        extends_class,
        local_vars: &[],
        params: &[],
        member_vars: &member_vars,
        local_func_returns: &local_func_returns,
        class_db,
        type_refinements: None,
    };

    // Try to resolve the type of the match expression
    let match_type = match resolve_expr_type(match_expr, parsed, &type_ctx) {
        Some(t) => t,
        None => return, // Can't determine type, skip
    };

    // Find the enum definition
    let enum_variants = find_enum_variants(&match_type, file_sym, all_file_symbols, class_db);
    if enum_variants.is_empty() {
        return; // Not an enum or unknown enum
    }

    // Collect all patterns in the match
    let mut covered_variants: HashSet<String> = HashSet::new();
    let mut has_wildcard = false;

    let mut cursor = match_node.walk();
    for child in match_node.children(&mut cursor) {
        if child.kind() == "match_body" {
            // Iterate over pattern_section/match_branch inside match_body
            let mut body_cursor = child.walk();
            for section in child.children(&mut body_cursor) {
                if section.kind() == "pattern_section" || section.kind() == "match_branch" {
                    collect_patterns_from_section(
                        section,
                        parsed,
                        &match_type,
                        &mut covered_variants,
                        &mut has_wildcard,
                    );
                }
            }
        }
        // Also handle pattern_section as direct child (grammar variations)
        if child.kind() == "pattern_section" || child.kind() == "match_branch" {
            collect_patterns_from_section(
                child,
                parsed,
                &match_type,
                &mut covered_variants,
                &mut has_wildcard,
            );
        }
    }

    // If there's a wildcard, all cases are covered
    if has_wildcard {
        return;
    }

    // Check for missing variants
    let missing: Vec<&String> = enum_variants
        .iter()
        .filter(|v| !covered_variants.contains(*v))
        .collect();

    if !missing.is_empty() {
        let start = match_node.start_position();
        let end = match_node.end_position();
        let missing_str = missing
            .iter()
            .take(3)
            .map(|s| format!("`{}.{}`", match_type, s))
            .collect::<Vec<_>>()
            .join(", ");
        let more = if missing.len() > 3 {
            format!(" and {} more", missing.len() - 3)
        } else {
            String::new()
        };

        diagnostics.push(
            Diagnostic::new(
                RULE_ID,
                Severity::Warning,
                format!(
                    "Match on `{}` is not exhaustive. Missing: {}{}",
                    match_type, missing_str, more
                ),
                start.row + 1,
            )
            .span(start.column, end.row + 1, end.column)
            .with_note("Add missing patterns or a wildcard `_` pattern to handle all cases."),
        );
    }
}

/// Check if a node kind is a pattern node.
fn is_pattern_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "integer"
            | "float"
            | "string"
            | "true"
            | "false"
            | "null"
            | "attribute"
            | "array"
            | "dictionary"
            | "pattern_binding"
    )
}

/// Collect patterns from a pattern_section or match_branch.
fn collect_patterns_from_section(
    section: Node,
    parsed: &ParsedFile,
    enum_type: &str,
    covered: &mut HashSet<String>,
    has_wildcard: &mut bool,
) {
    let mut cursor = section.walk();
    for child in section.children(&mut cursor) {
        // Skip non-named nodes and body
        if !child.is_named() || child.kind() == "body" || child.kind() == "comment" {
            continue;
        }
        // This is a pattern
        check_patterns(child, parsed, enum_type, covered, has_wildcard);
    }
}

/// Recursively check patterns and collect covered variants.
fn check_patterns(
    node: Node,
    parsed: &ParsedFile,
    enum_type: &str,
    covered: &mut HashSet<String>,
    has_wildcard: &mut bool,
) {
    match node.kind() {
        "identifier" => {
            let text = parsed.node_text(node);
            if text == "_" {
                *has_wildcard = true;
            } else {
                // Could be a bare enum variant
                covered.insert(text.to_string());
            }
        }
        "attribute" => {
            // EnumType.VARIANT pattern (e.g., State.IDLE)
            // Tree-sitter-gdscript doesn't use field names for attribute, so iterate children
            let mut cursor = node.walk();
            let mut obj_name = None;
            let mut attr_name = None;
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier" {
                    if obj_name.is_none() {
                        obj_name = Some(parsed.node_text(child).to_string());
                    } else {
                        attr_name = Some(parsed.node_text(child).to_string());
                    }
                }
            }
            if let (Some(obj), Some(attr)) = (obj_name, attr_name) {
                if obj == enum_type {
                    covered.insert(attr);
                }
            }
        }
        "pattern_list" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                check_patterns(child, parsed, enum_type, covered, has_wildcard);
            }
        }
        _ => {
            // Recurse into other pattern structures
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if is_pattern_node(child.kind()) || child.kind() == "pattern_list" {
                    check_patterns(child, parsed, enum_type, covered, has_wildcard);
                }
            }
        }
    }
}

/// Find enum variants for a given type name.
fn find_enum_variants(
    type_name: &str,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    class_db: &ClassDb,
) -> Vec<String> {
    // Check local enums
    for enum_decl in &file_sym.enums {
        if enum_decl.name == type_name {
            return enum_decl.values.clone();
        }
    }

    // Check enums in other project files
    for fs in all_file_symbols {
        for enum_decl in &fs.enums {
            if enum_decl.name == type_name {
                return enum_decl.values.clone();
            }
        }
    }

    // Check ClassDB for engine enums
    if let Some(enum_info) = class_db.get_global_enum(type_name) {
        return enum_info.values.iter().map(|v| v.name.clone()).collect();
    }

    // Check for class-scoped enums (e.g., Node.ProcessMode)
    if let Some((class_name, enum_name)) = type_name.split_once('.') {
        if let Some(class_info) = class_db.get_class(class_name) {
            if let Some(enum_info) = class_info.enums.iter().find(|e| e.name == enum_name) {
                return enum_info.values.iter().map(|v| v.name.clone()).collect();
            }
        }
    }

    Vec::new()
}
