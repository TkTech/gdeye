use std::path::Path;

use tree_sitter::{Language, Node, Parser, Tree};

/// A parsed GDScript file, holding both the source text and the syntax tree.
pub struct ParsedFile {
    source: String,
    tree: Tree,
}

impl std::fmt::Debug for ParsedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedFile")
            .field("source_len", &self.source.len())
            .field("has_errors", &self.tree.root_node().has_error())
            .finish()
    }
}

impl ParsedFile {
    pub fn source(&self) -> &str {
        &self.source
    }

    #[allow(dead_code)] // Public API for tree access
    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    /// Get the text content of a tree-sitter node.
    pub fn node_text(&self, node: Node) -> &str {
        node.utf8_text(self.source.as_bytes()).unwrap_or("")
    }
}

/// Parse a GDScript file from disk.
pub fn parse_file(path: &Path) -> Result<ParsedFile, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    parse_source(&source)
}

/// Parse GDScript source text.
pub fn parse_source(source: &str) -> Result<ParsedFile, String> {
    let mut parser = Parser::new();
    let language: Language = tree_sitter_gdscript::LANGUAGE.into();
    parser
        .set_language(&language)
        .map_err(|e| format!("Failed to set language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "Failed to parse source".to_string())?;

    Ok(ParsedFile {
        source: source.to_string(),
        tree,
    })
}

/// Iterator that walks all descendant nodes in a tree using a cursor.
#[allow(dead_code)] // Utility for AST traversal
pub struct NodeIter<'a> {
    cursor: tree_sitter::TreeCursor<'a>,
    done: bool,
}

#[allow(dead_code)] // Utility for AST traversal
impl<'a> NodeIter<'a> {
    pub fn new(node: Node<'a>) -> Self {
        Self {
            cursor: node.walk(),
            done: false,
        }
    }
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let node = self.cursor.node();

        // Try to descend to a child first
        if self.cursor.goto_first_child() {
            return Some(node);
        }

        // Try to move to the next sibling
        if self.cursor.goto_next_sibling() {
            return Some(node);
        }

        // Walk up until we find a sibling or reach the root
        loop {
            if !self.cursor.goto_parent() {
                self.done = true;
                return Some(node);
            }
            if self.cursor.goto_next_sibling() {
                return Some(node);
            }
        }
    }
}

/// Walk all descendant nodes of the given node.
#[allow(dead_code)] // Utility for AST traversal
pub fn walk_nodes(node: Node) -> NodeIter {
    NodeIter::new(node)
}

/// Find all descendant nodes matching a given kind.
pub fn find_nodes_by_kind<'a>(node: Node<'a>, kind: &str) -> Vec<Node<'a>> {
    let mut results = Vec::new();
    collect_by_kind(node, kind, &mut results);
    results
}

fn collect_by_kind<'a>(node: Node<'a>, kind: &str, results: &mut Vec<Node<'a>>) {
    if node.kind() == kind {
        results.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_by_kind(child, kind, results);
    }
}

/// Find the first ancestor of a node that matches the given kind.
#[allow(dead_code)] // Utility for AST traversal
pub fn find_ancestor<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(n) = current {
        if n.kind() == kind {
            return Some(n);
        }
        current = n.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_source_simple() {
        let parsed = parse_source("var x = 1\n").unwrap();
        assert_eq!(parsed.source(), "var x = 1\n");
        assert_eq!(parsed.root_node().kind(), "source");
    }

    #[test]
    fn parsed_file_tree_accessor() {
        let parsed = parse_source("var x = 1\n").unwrap();
        let tree = parsed.tree();
        assert_eq!(tree.root_node().kind(), "source");
    }

    #[test]
    fn parse_file_nonexistent() {
        let result = parse_file(Path::new("/nonexistent/file.gd"));
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("Failed to read"));
    }

    #[test]
    fn node_text_works() {
        let parsed = parse_source("var hello = 42\n").unwrap();
        let root = parsed.root_node();
        let var_stmt = root.child(0).unwrap();
        let names = find_nodes_by_kind(var_stmt, "name");
        assert!(!names.is_empty());
        assert_eq!(parsed.node_text(names[0]), "hello");
    }

    #[test]
    fn walk_nodes_visits_all() {
        let parsed = parse_source("var x = 1\n").unwrap();
        let nodes: Vec<_> = walk_nodes(parsed.root_node()).collect();
        assert!(nodes.len() > 1);
        // Should include the root and children
        assert!(nodes.iter().any(|n| n.kind() == "source"));
    }

    #[test]
    fn find_nodes_by_kind_finds_identifiers() {
        let parsed = parse_source("var x = 1\nvar y = 2\n").unwrap();
        let names = find_nodes_by_kind(parsed.root_node(), "name");
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn find_nodes_by_kind_empty() {
        let parsed = parse_source("var x = 1\n").unwrap();
        let fns = find_nodes_by_kind(parsed.root_node(), "function_definition");
        assert!(fns.is_empty());
    }

    #[test]
    fn find_ancestor_found() {
        let parsed = parse_source("func foo():\n    var x = 1\n").unwrap();
        let names = find_nodes_by_kind(parsed.root_node(), "name");
        // Find the "x" name inside the function
        let x_node = names.iter().find(|n| parsed.node_text(**n) == "x").unwrap();
        let ancestor = find_ancestor(*x_node, "function_definition");
        assert!(ancestor.is_some());
        assert_eq!(ancestor.unwrap().kind(), "function_definition");
    }

    #[test]
    fn find_ancestor_not_found() {
        let parsed = parse_source("var x = 1\n").unwrap();
        let names = find_nodes_by_kind(parsed.root_node(), "name");
        let x_node = names[0];
        let ancestor = find_ancestor(x_node, "function_definition");
        assert!(ancestor.is_none());
    }
}
