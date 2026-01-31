//! Conversion utilities between gdeye types and LSP types.

use tower_lsp::lsp_types::*;

use super::state::MemberInfo;
use crate::classdb::MethodInfo;
use crate::document::Document;
use crate::rules::{DiagLabel, Diagnostic as GdDiagnostic, Severity as GdSeverity};
use crate::symbol_index::{SymbolDef, SymbolKind as GdSymbolKind};

/// Convert a gdeye diagnostic to an LSP diagnostic.
///
/// If a URI is provided, diagnostic labels will be converted to related_information.
pub fn to_lsp_diagnostic(diag: &GdDiagnostic) -> Diagnostic {
    to_lsp_diagnostic_with_uri(diag, None)
}

/// Convert a gdeye diagnostic to an LSP diagnostic with URI context for labels.
pub fn to_lsp_diagnostic_with_uri(diag: &GdDiagnostic, uri: Option<&Url>) -> Diagnostic {
    let related_information = if diag.labels.is_empty() {
        None
    } else {
        uri.map(|u| {
            diag.labels
                .iter()
                .map(|label| label_to_related_info(label, u))
                .collect()
        })
    };

    Diagnostic {
        range: Range {
            start: Position {
                line: diag.line.saturating_sub(1) as u32,
                character: diag.col.saturating_sub(1) as u32,
            },
            end: Position {
                line: diag.end_line.saturating_sub(1) as u32,
                character: diag.end_col.saturating_sub(1) as u32,
            },
        },
        severity: Some(to_lsp_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.rule.to_string())),
        code_description: None,
        source: Some("gdeye".to_string()),
        message: diag.message.clone(),
        related_information,
        tags: None,
        data: None,
    }
}

/// Convert a diagnostic label to LSP related information.
fn label_to_related_info(label: &DiagLabel, uri: &Url) -> DiagnosticRelatedInformation {
    DiagnosticRelatedInformation {
        location: Location {
            uri: uri.clone(),
            range: Range {
                start: Position {
                    line: label.line.saturating_sub(1) as u32,
                    character: label.col.saturating_sub(1) as u32,
                },
                end: Position {
                    line: label.end_line.saturating_sub(1) as u32,
                    character: label.end_col.saturating_sub(1) as u32,
                },
            },
        },
        message: label.message.clone(),
    }
}

/// Convert gdeye severity to LSP severity.
pub fn to_lsp_severity(severity: GdSeverity) -> DiagnosticSeverity {
    match severity {
        GdSeverity::Error => DiagnosticSeverity::ERROR,
        GdSeverity::Warning => DiagnosticSeverity::WARNING,
        GdSeverity::Info => DiagnosticSeverity::INFORMATION,
    }
}

/// Convert a gdeye symbol kind to an LSP symbol kind.
pub fn to_lsp_symbol_kind(kind: GdSymbolKind) -> SymbolKind {
    match kind {
        GdSymbolKind::Variable => SymbolKind::VARIABLE,
        GdSymbolKind::Function => SymbolKind::FUNCTION,
        GdSymbolKind::Signal => SymbolKind::EVENT,
        GdSymbolKind::Constant => SymbolKind::CONSTANT,
        GdSymbolKind::Enum => SymbolKind::ENUM,
        GdSymbolKind::EnumValue => SymbolKind::ENUM_MEMBER,
        GdSymbolKind::Class => SymbolKind::CLASS,
        GdSymbolKind::Parameter => SymbolKind::VARIABLE,
        GdSymbolKind::Property => SymbolKind::PROPERTY,
    }
}

/// Convert a gdeye symbol definition to an LSP document symbol.
pub fn to_lsp_document_symbol(sym: &SymbolDef, doc: &Document) -> Option<DocumentSymbol> {
    let range = sym.range?;
    let (start_line, start_col) = doc.position_at(range.0);
    let (end_line, end_col) = doc.position_at(range.1);

    let selection_range = if let Some((name_start, name_end)) = sym.name_range {
        let (sel_start_line, sel_start_col) = doc.position_at(name_start);
        let (sel_end_line, sel_end_col) = doc.position_at(name_end);
        Range {
            start: Position {
                line: sel_start_line as u32,
                character: sel_start_col as u32,
            },
            end: Position {
                line: sel_end_line as u32,
                character: sel_end_col as u32,
            },
        }
    } else {
        Range {
            start: Position {
                line: start_line as u32,
                character: start_col as u32,
            },
            end: Position {
                line: end_line as u32,
                character: end_col as u32,
            },
        }
    };

    #[allow(deprecated)]
    Some(DocumentSymbol {
        name: sym.name.clone(),
        detail: sym.type_hint.clone(),
        kind: to_lsp_symbol_kind(sym.kind),
        tags: None,
        deprecated: None,
        range: Range {
            start: Position {
                line: start_line as u32,
                character: start_col as u32,
            },
            end: Position {
                line: end_line as u32,
                character: end_col as u32,
            },
        },
        selection_range,
        children: None,
    })
}

/// Format hover content for a symbol.
pub fn format_hover_content(sym: &SymbolDef) -> String {
    let mut content = String::new();

    // Type signature
    match sym.kind {
        GdSymbolKind::Function => {
            content.push_str("```gdscript\n");
            content.push_str("func ");
            content.push_str(&sym.name);
            content.push_str("()");
            if let Some(ref ret) = sym.type_hint {
                content.push_str(" -> ");
                content.push_str(ret);
            }
            content.push_str("\n```\n");
        }
        GdSymbolKind::Variable | GdSymbolKind::Parameter => {
            content.push_str("```gdscript\n");
            content.push_str("var ");
            content.push_str(&sym.name);
            if let Some(ref type_hint) = sym.type_hint {
                content.push_str(": ");
                content.push_str(type_hint);
            }
            content.push_str("\n```\n");
        }
        GdSymbolKind::Constant => {
            content.push_str("```gdscript\n");
            content.push_str("const ");
            content.push_str(&sym.name);
            if let Some(ref type_hint) = sym.type_hint {
                content.push_str(": ");
                content.push_str(type_hint);
            }
            content.push_str("\n```\n");
        }
        GdSymbolKind::Signal => {
            content.push_str("```gdscript\n");
            content.push_str("signal ");
            content.push_str(&sym.name);
            content.push_str("\n```\n");
        }
        GdSymbolKind::Enum => {
            content.push_str("```gdscript\n");
            content.push_str("enum ");
            content.push_str(&sym.name);
            content.push_str("\n```\n");
        }
        GdSymbolKind::EnumValue => {
            content.push_str("```gdscript\n");
            content.push_str(&sym.name);
            if let Some(ref enum_name) = sym.type_hint {
                content.push_str(" (");
                content.push_str(enum_name);
                content.push(')');
            }
            content.push_str("\n```\n");
        }
        GdSymbolKind::Class => {
            content.push_str("```gdscript\n");
            content.push_str("class ");
            content.push_str(&sym.name);
            if let Some(ref extends) = sym.type_hint {
                content.push_str(" extends ");
                content.push_str(extends);
            }
            content.push_str("\n```\n");
        }
        GdSymbolKind::Property => {
            content.push_str("```gdscript\n");
            content.push_str(&sym.name);
            if let Some(ref type_hint) = sym.type_hint {
                content.push_str(": ");
                content.push_str(type_hint);
            }
            content.push_str("\n```\n");
        }
    }

    // Documentation
    if let Some(ref doc) = sym.documentation {
        content.push_str("\n---\n\n");
        content.push_str(doc);
    }

    content
}

/// Format hover content for a class member (from cross-file lookup).
pub fn format_member_hover(member: &MemberInfo, class_name: &str) -> String {
    let mut content = String::new();

    match member {
        MemberInfo::Function {
            name,
            return_type,
            parameters,
            documentation,
        } => {
            content.push_str("```gdscript\n");
            content.push_str("func ");
            content.push_str(name);
            content.push('(');
            for (i, (pname, ptype)) in parameters.iter().enumerate() {
                if i > 0 {
                    content.push_str(", ");
                }
                content.push_str(pname);
                if let Some(t) = ptype {
                    content.push_str(": ");
                    content.push_str(t);
                }
            }
            content.push(')');
            if let Some(ref ret) = return_type {
                content.push_str(" -> ");
                content.push_str(ret);
            }
            content.push_str("\n```\n");
            content.push_str(&format!("\n*From class `{}`*\n", class_name));
            if let Some(doc) = documentation {
                content.push_str("\n---\n\n");
                content.push_str(doc);
            }
        }
        MemberInfo::Variable {
            name,
            type_hint,
            documentation,
        } => {
            content.push_str("```gdscript\n");
            content.push_str("var ");
            content.push_str(name);
            if let Some(t) = type_hint {
                content.push_str(": ");
                content.push_str(t);
            }
            content.push_str("\n```\n");
            content.push_str(&format!("\n*From class `{}`*\n", class_name));
            if let Some(doc) = documentation {
                content.push_str("\n---\n\n");
                content.push_str(doc);
            }
        }
        MemberInfo::Signal {
            name,
            parameters,
            documentation,
        } => {
            content.push_str("```gdscript\n");
            content.push_str("signal ");
            content.push_str(name);
            if !parameters.is_empty() {
                content.push('(');
                content.push_str(&parameters.join(", "));
                content.push(')');
            }
            content.push_str("\n```\n");
            content.push_str(&format!("\n*From class `{}`*\n", class_name));
            if let Some(doc) = documentation {
                content.push_str("\n---\n\n");
                content.push_str(doc);
            }
        }
        MemberInfo::Constant {
            name,
            type_hint,
            documentation,
        } => {
            content.push_str("```gdscript\n");
            content.push_str("const ");
            content.push_str(name);
            if let Some(t) = type_hint {
                content.push_str(": ");
                content.push_str(t);
            }
            content.push_str("\n```\n");
            content.push_str(&format!("\n*From class `{}`*\n", class_name));
            if let Some(doc) = documentation {
                content.push_str("\n---\n\n");
                content.push_str(doc);
            }
        }
    }

    content
}

/// Format hover content for a ClassDb method.
pub fn format_classdb_method_hover(method: &MethodInfo, class_name: &str) -> String {
    let mut content = String::new();

    content.push_str("```gdscript\n");
    if method.is_static {
        content.push_str("static ");
    }
    if method.is_virtual {
        content.push_str("virtual ");
    }
    content.push_str("func ");
    content.push_str(&method.name);
    content.push('(');
    for (i, arg) in method.arguments.iter().enumerate() {
        if i > 0 {
            content.push_str(", ");
        }
        content.push_str(&arg.name);
        if !arg.arg_type.is_empty() {
            content.push_str(": ");
            content.push_str(&arg.arg_type);
        }
    }
    content.push(')');
    if !method.return_type.is_empty() && method.return_type != "void" {
        content.push_str(" -> ");
        content.push_str(&method.return_type);
    }
    content.push_str("\n```\n");
    content.push_str(&format!("\n*From engine class `{}`*\n", class_name));

    content
}
