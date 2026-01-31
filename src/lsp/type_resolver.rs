//! Unified type resolution for LSP features.
//!
//! This module consolidates type resolution logic that was previously scattered
//! across state.rs, handlers.rs, and cross_file_usage.rs.

use std::path::{Path, PathBuf};

use crate::classdb::ClassDb;
use crate::symbol_index::SymbolIndex;
use crate::symbols::FileSymbols;

use crate::project_index::ProjectIndex;

/// The result of resolving a type.
#[derive(Debug, Clone)]
pub enum ResolvedType {
    /// A builtin/engine type (e.g., Node, Vector2).
    Builtin { name: String },
    /// A user-defined class from the project.
    UserClass {
        path: PathBuf,
        class_name: Option<String>,
    },
    /// An autoload singleton.
    Autoload { name: String, path: PathBuf },
    /// A local variable.
    LocalVar {
        func_name: String,
        var_name: String,
        type_hint: Option<String>,
    },
    /// A function parameter.
    Parameter {
        func_name: String,
        param_name: String,
        type_hint: Option<String>,
    },
    /// A function definition.
    Function {
        name: String,
        return_type: Option<String>,
    },
    /// A signal definition.
    Signal { name: String },
    /// A constant.
    Constant {
        name: String,
        type_hint: Option<String>,
    },
    /// A class member variable.
    MemberVar {
        name: String,
        type_hint: Option<String>,
    },
}

impl ResolvedType {
    /// Get the type name for this resolved type.
    pub fn type_name(&self) -> Option<&str> {
        match self {
            ResolvedType::Builtin { name } => Some(name),
            ResolvedType::UserClass { class_name, .. } => class_name.as_deref(),
            ResolvedType::Autoload { name, .. } => Some(name),
            ResolvedType::LocalVar { type_hint, .. } => type_hint.as_deref(),
            ResolvedType::Parameter { type_hint, .. } => type_hint.as_deref(),
            ResolvedType::Function { return_type, .. } => return_type.as_deref(),
            ResolvedType::MemberVar { type_hint, .. } => type_hint.as_deref(),
            ResolvedType::Constant { type_hint, .. } => type_hint.as_deref(),
            ResolvedType::Signal { .. } => Some("Signal"),
        }
    }
}

/// Information about a class member.
#[derive(Debug, Clone)]
pub enum MemberInfo {
    Function {
        name: String,
        return_type: Option<String>,
        parameters: Vec<(String, Option<String>)>,
        documentation: Option<String>,
        line: usize,
    },
    Variable {
        name: String,
        type_hint: Option<String>,
        documentation: Option<String>,
        line: usize,
    },
    Signal {
        name: String,
        parameters: Vec<String>,
        documentation: Option<String>,
        line: usize,
    },
    Constant {
        name: String,
        type_hint: Option<String>,
        documentation: Option<String>,
        line: usize,
    },
}

impl MemberInfo {
    /// Get the name of this member.
    pub fn name(&self) -> &str {
        match self {
            MemberInfo::Function { name, .. }
            | MemberInfo::Variable { name, .. }
            | MemberInfo::Signal { name, .. }
            | MemberInfo::Constant { name, .. } => name,
        }
    }

    /// Get the line number of this member.
    pub fn line(&self) -> usize {
        match self {
            MemberInfo::Function { line, .. }
            | MemberInfo::Variable { line, .. }
            | MemberInfo::Signal { line, .. }
            | MemberInfo::Constant { line, .. } => *line,
        }
    }
}

/// Type resolver for a specific file context.
pub struct TypeResolver<'a> {
    /// Project-wide symbol index.
    project_index: &'a ProjectIndex,
    /// Class database for builtin types.
    class_db: &'a ClassDb,
    /// Current file symbols.
    current_symbols: &'a FileSymbols,
    /// Current file symbol index.
    current_index: Option<&'a SymbolIndex>,
}

impl<'a> TypeResolver<'a> {
    /// Create a new type resolver for a file.
    pub fn new(
        project_index: &'a ProjectIndex,
        class_db: &'a ClassDb,
        _current_file: &'a Path,
        current_symbols: &'a FileSymbols,
        current_index: Option<&'a SymbolIndex>,
    ) -> Self {
        Self {
            project_index,
            class_db,
            current_symbols,
            current_index,
        }
    }

    /// Resolve a name at a specific byte offset.
    pub fn resolve_name(&self, name: &str, at_byte: usize) -> Option<ResolvedType> {
        // 1. Try local resolution using the symbol index
        if let Some(index) = self.current_index {
            if let Some(def_id) = index.resolve_name(name, at_byte) {
                if let Some(def) = index.get_definition(def_id) {
                    return Some(self.resolved_type_from_def(def));
                }
            }
        }

        // 2. Try current file's members
        if let Some(resolved) = self.resolve_in_file(name, self.current_symbols) {
            return Some(resolved);
        }

        // 3. Try autoloads
        if let Some(path) = self.project_index.path_for_autoload(name) {
            return Some(ResolvedType::Autoload {
                name: name.to_string(),
                path: path.to_path_buf(),
            });
        }

        // 4. Try class_name references
        if let Some(path) = self.project_index.path_for_class_name(name) {
            return Some(ResolvedType::UserClass {
                path: path.to_path_buf(),
                class_name: Some(name.to_string()),
            });
        }

        // 5. Try builtin types
        if self.class_db.get_class(name).is_some()
            || self.class_db.get_builtin_class(name).is_some()
        {
            return Some(ResolvedType::Builtin {
                name: name.to_string(),
            });
        }

        None
    }

    /// Resolve a type name (without position context).
    pub fn resolve_type_name(&self, type_name: &str) -> Option<ResolvedType> {
        // Check class_name first
        if let Some(path) = self.project_index.path_for_class_name(type_name) {
            return Some(ResolvedType::UserClass {
                path: path.to_path_buf(),
                class_name: Some(type_name.to_string()),
            });
        }

        // Check autoloads
        if let Some(path) = self.project_index.path_for_autoload(type_name) {
            return Some(ResolvedType::Autoload {
                name: type_name.to_string(),
                path: path.to_path_buf(),
            });
        }

        // Check builtin types
        if self.class_db.get_class(type_name).is_some()
            || self.class_db.get_builtin_class(type_name).is_some()
        {
            return Some(ResolvedType::Builtin {
                name: type_name.to_string(),
            });
        }

        None
    }

    /// Resolve a variable's type.
    pub fn resolve_variable_type(&self, var_name: &str, at_byte: usize) -> Option<String> {
        // Check local variables in functions
        for func in &self.current_symbols.functions {
            if at_byte >= func.start_byte && at_byte < func.end_byte {
                // Check local vars
                for local in &func.local_vars {
                    if local.name == var_name {
                        if let Some(ref t) = local.type_annotation {
                            return Some(t.clone());
                        }
                        if let Some(ref t) = local.inferred_type {
                            return Some(t.clone());
                        }
                        if let Some(ref t) = local.initializer_type {
                            return Some(t.clone());
                        }
                    }
                }
                // Check parameters
                for param in &func.parameters {
                    if param.name == var_name {
                        if let Some(ref t) = param.type_annotation {
                            return Some(t.clone());
                        }
                        if let Some(ref t) = param.inferred_type {
                            return Some(t.clone());
                        }
                    }
                }
            }
        }

        // Check class-level variables
        for var in &self.current_symbols.variables {
            if var.name == var_name {
                if let Some(ref t) = var.type_annotation {
                    return Some(t.clone());
                }
                if let Some(ref t) = var.inferred_type {
                    return Some(t.clone());
                }
                if let Some(ref t) = var.initializer_type {
                    return Some(t.clone());
                }
            }
        }

        None
    }

    /// Find a member on a type.
    pub fn find_member(&self, type_name: &str, member_name: &str) -> Option<MemberInfo> {
        // Try user-defined class first
        if let Some(file) = self.project_index.get_by_class_name(type_name) {
            return self.find_member_in_symbols(&file.symbols, member_name);
        }

        // Try autoload
        if let Some(file) = self.project_index.get_by_autoload(type_name) {
            return self.find_member_in_symbols(&file.symbols, member_name);
        }

        // Builtin types are handled separately via ClassDb
        None
    }

    /// Find a member in the given file symbols.
    pub fn find_member_in_symbols(
        &self,
        symbols: &FileSymbols,
        member_name: &str,
    ) -> Option<MemberInfo> {
        // Check functions
        for func in &symbols.functions {
            if func.name == member_name {
                return Some(MemberInfo::Function {
                    name: func.name.clone(),
                    return_type: func
                        .return_type
                        .clone()
                        .or_else(|| func.inferred_return_type.clone()),
                    parameters: func
                        .parameters
                        .iter()
                        .map(|p| (p.name.clone(), p.type_annotation.clone()))
                        .collect(),
                    documentation: func.documentation.clone(),
                    line: func.line,
                });
            }
        }

        // Check variables
        for var in &symbols.variables {
            if var.name == member_name {
                return Some(MemberInfo::Variable {
                    name: var.name.clone(),
                    type_hint: var
                        .type_annotation
                        .clone()
                        .or_else(|| var.inferred_type.clone()),
                    documentation: var.documentation.clone(),
                    line: var.line,
                });
            }
        }

        // Check signals
        for signal in &symbols.signals {
            if signal.name == member_name {
                return Some(MemberInfo::Signal {
                    name: signal.name.clone(),
                    parameters: signal.parameters.clone(),
                    documentation: signal.documentation.clone(),
                    line: signal.line,
                });
            }
        }

        // Check constants
        for constant in &symbols.constants {
            if constant.name == member_name {
                return Some(MemberInfo::Constant {
                    name: constant.name.clone(),
                    type_hint: constant.type_annotation.clone(),
                    documentation: constant.documentation.clone(),
                    line: constant.line,
                });
            }
        }

        None
    }

    /// Get the path for a class or autoload.
    pub fn path_for_type(&self, type_name: &str) -> Option<&Path> {
        self.project_index
            .path_for_class_name(type_name)
            .or_else(|| self.project_index.path_for_autoload(type_name))
    }

    /// Check if a type is a user-defined class.
    pub fn is_user_class(&self, type_name: &str) -> bool {
        self.project_index.path_for_class_name(type_name).is_some()
    }

    /// Check if a type is an autoload.
    pub fn is_autoload(&self, type_name: &str) -> bool {
        self.project_index.path_for_autoload(type_name).is_some()
    }

    /// Check if a type is a builtin/engine type.
    pub fn is_builtin(&self, type_name: &str) -> bool {
        self.class_db.get_class(type_name).is_some()
            || self.class_db.get_builtin_class(type_name).is_some()
    }

    /// Resolve a name in a file's symbols.
    fn resolve_in_file(&self, name: &str, symbols: &FileSymbols) -> Option<ResolvedType> {
        // Check functions
        for func in &symbols.functions {
            if func.name == name {
                return Some(ResolvedType::Function {
                    name: func.name.clone(),
                    return_type: func
                        .return_type
                        .clone()
                        .or_else(|| func.inferred_return_type.clone()),
                });
            }
        }

        // Check class-level variables
        for var in &symbols.variables {
            if var.name == name {
                return Some(ResolvedType::MemberVar {
                    name: var.name.clone(),
                    type_hint: var
                        .type_annotation
                        .clone()
                        .or_else(|| var.inferred_type.clone()),
                });
            }
        }

        // Check constants
        for constant in &symbols.constants {
            if constant.name == name {
                return Some(ResolvedType::Constant {
                    name: constant.name.clone(),
                    type_hint: constant.type_annotation.clone(),
                });
            }
        }

        // Check signals
        for signal in &symbols.signals {
            if signal.name == name {
                return Some(ResolvedType::Signal {
                    name: signal.name.clone(),
                });
            }
        }

        None
    }

    /// Convert a symbol definition to a ResolvedType.
    fn resolved_type_from_def(&self, def: &crate::symbol_index::SymbolDef) -> ResolvedType {
        use crate::symbol_index::SymbolKind;
        use crate::symbols::Scope;

        match (&def.kind, &def.scope) {
            (SymbolKind::Function, _) => ResolvedType::Function {
                name: def.name.clone(),
                return_type: def.type_hint.clone(),
            },
            (SymbolKind::Variable, Scope::Function(func_name)) => ResolvedType::LocalVar {
                func_name: func_name.clone(),
                var_name: def.name.clone(),
                type_hint: def.type_hint.clone(),
            },
            (SymbolKind::Variable, Scope::File) => ResolvedType::MemberVar {
                name: def.name.clone(),
                type_hint: def.type_hint.clone(),
            },
            (SymbolKind::Parameter, Scope::Function(func_name)) => ResolvedType::Parameter {
                func_name: func_name.clone(),
                param_name: def.name.clone(),
                type_hint: def.type_hint.clone(),
            },
            (SymbolKind::Constant, _) => ResolvedType::Constant {
                name: def.name.clone(),
                type_hint: def.type_hint.clone(),
            },
            (SymbolKind::Signal, _) => ResolvedType::Signal {
                name: def.name.clone(),
            },
            _ => ResolvedType::MemberVar {
                name: def.name.clone(),
                type_hint: def.type_hint.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests would require setting up ProjectIndex and ClassDb, which is complex
    // These are better tested via integration tests
}
