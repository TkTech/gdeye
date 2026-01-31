use tree_sitter::Node;

/// Print the tree-sitter AST for debugging purposes.
#[allow(dead_code)]
pub fn print_tree(node: Node, source: &str, indent: usize) {
    let prefix = " ".repeat(indent);
    let text = node
        .utf8_text(source.as_bytes())
        .unwrap_or("")
        .lines()
        .next()
        .unwrap_or("");
    let text_preview = if text.len() > 60 {
        format!("{}...", &text[..60])
    } else {
        text.to_string()
    };

    if node.is_named() {
        println!(
            "{}{} [{}:{}] {:?}",
            prefix,
            node.kind(),
            node.start_position().row + 1,
            node.start_position().column,
            text_preview,
        );
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_tree(child, source, indent + 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    #[test]
    fn print_tree_does_not_panic() {
        let parsed = parser::parse_source("var x = 1\nfunc foo():\n    pass\n").unwrap();
        // Just ensure it doesn't panic
        print_tree(parsed.root_node(), parsed.source(), 0);
    }

    #[test]
    fn print_tree_long_text_truncated() {
        // Create source with a very long string literal
        let long_str = "x".repeat(100);
        let source = format!("var s = \"{}\"\n", long_str);
        let parsed = parser::parse_source(&source).unwrap();
        // Should not panic even with long text
        print_tree(parsed.root_node(), parsed.source(), 0);
    }
}
