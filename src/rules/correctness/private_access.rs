use crate::parser::{self, ParsedFile};
use crate::symbols::FileSymbols;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/private-access";

pub struct PrivateAccess;

impl Rule for PrivateAccess {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Accessing private member (prefixed with _) of another class"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        check_private_access(
            ctx.parsed,
            ctx.file_sym,
            ctx.all_file_symbols,
            &mut diagnostics,
        );
        diagnostics
    }
}

fn check_private_access(
    parsed: &ParsedFile,
    file_sym: &FileSymbols,
    all_file_symbols: &[FileSymbols],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let attrs = parser::find_nodes_by_kind(root, "attribute");

    for attr in attrs {
        let mut cursor = attr.walk();
        let children: Vec<_> = attr.children(&mut cursor).collect();

        // attribute node: object.member
        // children: [object, ".", member]
        if children.len() < 3 {
            continue;
        }

        let member_node = children.last().unwrap();
        let member_name = parsed.node_text(*member_node);

        // Only check members that start with _
        if !member_name.starts_with('_') {
            continue;
        }

        // Skip dunder methods like __init__
        if member_name.starts_with("__") {
            continue;
        }

        let receiver_node = children[0];
        let receiver_text = parsed.node_text(receiver_node);

        // Skip if receiver is self - accessing own private members is fine
        if receiver_text == "self" {
            continue;
        }

        // Skip if receiver is super
        if receiver_text == "super" {
            continue;
        }

        // Skip if this member is defined in the current file
        let is_own_member = file_sym.functions.iter().any(|f| f.name == member_name)
            || file_sym.variables.iter().any(|v| v.name == member_name);

        if is_own_member {
            continue;
        }

        // Try to resolve receiver type to check if it's from another file
        let receiver_type = resolve_receiver_type(parsed, receiver_node, file_sym);

        if let Some(ref type_name) = receiver_type {
            // Check if the type is defined in another file
            let is_external = all_file_symbols.iter().any(|fs| {
                !std::ptr::eq(fs, file_sym) && fs.class_name.as_deref() == Some(type_name.as_str())
            });

            if is_external {
                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!(
                            "Accessing private member `{}` of class `{}`.",
                            member_name, type_name
                        ),
                        attr.start_position().row + 1,
                    )
                    .span(
                        member_node.start_position().column,
                        member_node.end_position().row + 1,
                        member_node.end_position().column,
                    ),
                );
                continue;
            }
        }

        // If we couldn't resolve the type but it's clearly not self,
        // and the receiver is a typed variable, flag it
        if receiver_type.is_some() {
            // Already handled above
            continue;
        }

        // For unresolved types, check if receiver is a local variable with a type annotation
        // that points to a different class
        let typed = find_typed_var(file_sym, receiver_text);
        if let Some(type_name) = typed {
            let is_external = all_file_symbols.iter().any(|fs| {
                !std::ptr::eq(fs, file_sym) && fs.class_name.as_deref() == Some(type_name.as_str())
            });

            if is_external {
                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!(
                            "Accessing private member `{}` of class `{}`.",
                            member_name, type_name
                        ),
                        attr.start_position().row + 1,
                    )
                    .span(
                        member_node.start_position().column,
                        member_node.end_position().row + 1,
                        member_node.end_position().column,
                    ),
                );
            }
        }
    }
}

fn resolve_receiver_type(
    parsed: &ParsedFile,
    receiver: tree_sitter::Node,
    file_sym: &FileSymbols,
) -> Option<String> {
    let receiver_text = parsed.node_text(receiver);

    // Check local variables and member variables for type annotations
    find_typed_var(file_sym, receiver_text)
}

fn find_typed_var(file_sym: &FileSymbols, name: &str) -> Option<String> {
    // Check member variables
    for var in &file_sym.variables {
        if var.name == name {
            if let Some(ref t) = var.type_annotation {
                if !t.is_empty() {
                    return Some(t.clone());
                }
            }
            if let Some(ref t) = var.inferred_type {
                if !t.is_empty() && t != ":=" {
                    return Some(t.clone());
                }
            }
        }
    }

    // Check function local vars
    for func in &file_sym.functions {
        for var in &func.local_vars {
            if var.name == name {
                if let Some(ref t) = var.type_annotation {
                    if !t.is_empty() {
                        return Some(t.clone());
                    }
                }
                if let Some(ref t) = var.inferred_type {
                    if !t.is_empty() && t != ":=" {
                        return Some(t.clone());
                    }
                }
            }
        }
    }

    None
}
