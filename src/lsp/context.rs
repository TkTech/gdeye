//! Request context helpers for LSP handlers.
//!
//! This module provides a unified way to access common context needed by handlers,
//! reducing boilerplate code in each handler.

use std::path::PathBuf;

use tower_lsp::lsp_types::Position;

use crate::classdb::ClassDb;
use crate::document::Document;
use crate::parser::ParsedFile;
use crate::symbol_index::SymbolIndex;
use crate::symbols::FileSymbols;

use super::cursor::CursorContext;
use super::type_resolver::TypeResolver;
use super::uri::uri_to_path;
use crate::project_index::{IndexedFile, ProjectIndex};

/// Context for a request to a specific document.
pub struct RequestContext<'a> {
    /// The document URI.
    pub uri: String,
    /// The file path.
    pub path: PathBuf,
    /// The indexed file from the project.
    pub file: &'a IndexedFile,
    /// The document (for open documents).
    pub document: Option<&'a Document>,
    /// The parsed file.
    pub parsed: Option<&'a ParsedFile>,
    /// The symbol index.
    pub index: Option<&'a SymbolIndex>,
    /// The file symbols.
    pub symbols: &'a FileSymbols,
    /// The project index.
    pub project_index: &'a ProjectIndex,
    /// The class database.
    pub class_db: &'a ClassDb,
}

impl<'a> RequestContext<'a> {
    /// Create a type resolver for this context.
    pub fn type_resolver(&'a self) -> TypeResolver<'a> {
        TypeResolver::new(
            self.project_index,
            self.class_db,
            &self.path,
            self.symbols,
            self.index,
        )
    }

    /// Get cursor context at the given LSP position.
    pub fn cursor_at(&'a self, position: Position) -> Option<CursorContext<'a>> {
        let parsed = self.parsed?;
        let doc = self.document?;
        CursorContext::at_position(
            parsed,
            position.line as usize,
            position.character as usize,
            doc,
        )
    }

    /// Get cursor context at a byte offset.
    pub fn cursor_at_offset(&'a self, offset: usize) -> Option<CursorContext<'a>> {
        let parsed = self.parsed?;
        CursorContext::at_offset(parsed, offset)
    }

    /// Convert an LSP position to a byte offset.
    pub fn offset_at(&self, position: Position) -> Option<usize> {
        self.document
            .and_then(|doc| doc.offset_at(position.line as usize, position.character as usize))
    }

    /// Convert a byte offset to an LSP position.
    pub fn position_at(&self, offset: usize) -> Option<Position> {
        self.document.map(|doc| {
            let (line, col) = doc.position_at(offset);
            Position {
                line: line as u32,
                character: col as u32,
            }
        })
    }
}

/// Builder for constructing RequestContext from different sources.
pub struct RequestContextBuilder<'a> {
    uri: String,
    path: Option<PathBuf>,
    file: Option<&'a IndexedFile>,
    document: Option<&'a Document>,
    parsed: Option<&'a ParsedFile>,
    index: Option<&'a SymbolIndex>,
    symbols: Option<&'a FileSymbols>,
    project_index: &'a ProjectIndex,
    class_db: &'a ClassDb,
}

impl<'a> RequestContextBuilder<'a> {
    /// Create a new builder with required dependencies.
    pub fn new(uri: String, project_index: &'a ProjectIndex, class_db: &'a ClassDb) -> Self {
        Self {
            uri,
            path: None,
            file: None,
            document: None,
            parsed: None,
            index: None,
            symbols: None,
            project_index,
            class_db,
        }
    }

    /// Set the file path (optional - will be derived from URI if not set).
    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    /// Set the indexed file.
    pub fn with_file(mut self, file: &'a IndexedFile) -> Self {
        self.file = Some(file);
        self
    }

    /// Set the document.
    pub fn with_document(mut self, document: &'a Document) -> Self {
        self.document = Some(document);
        self
    }

    /// Set the parsed file.
    pub fn with_parsed(mut self, parsed: &'a ParsedFile) -> Self {
        self.parsed = Some(parsed);
        self
    }

    /// Set the symbol index.
    pub fn with_index(mut self, index: &'a SymbolIndex) -> Self {
        self.index = Some(index);
        self
    }

    /// Set the file symbols.
    pub fn with_symbols(mut self, symbols: &'a FileSymbols) -> Self {
        self.symbols = Some(symbols);
        self
    }

    /// Build the RequestContext.
    ///
    /// Returns None if required fields are missing.
    pub fn build(self) -> Option<RequestContext<'a>> {
        let path = self
            .path
            .unwrap_or_else(|| uri_to_path(&self.uri).unwrap_or_else(|_| PathBuf::from(&self.uri)));

        // If we have an indexed file, use its data
        let (symbols, parsed, index) = if let Some(file) = self.file {
            (
                file.symbols.as_ref(),
                self.parsed.or(file.parsed.as_ref()),
                self.index.or(file.index.as_ref()),
            )
        } else {
            (self.symbols?, self.parsed, self.index)
        };

        Some(RequestContext {
            uri: self.uri,
            path,
            file: self.file?,
            document: self.document,
            parsed,
            index,
            symbols,
            project_index: self.project_index,
            class_db: self.class_db,
        })
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require full ServerState setup
}
