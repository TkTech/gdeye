use std::collections::BTreeMap;
use tree_sitter::Node;

/// A single comment extracted from the source tree.
#[derive(Debug, Clone)]
pub struct Comment {
    pub text: String,
    pub start_byte: usize,
    pub start_row: usize,
}

/// Stores comments classified as leading or trailing relative to code nodes.
///
/// Leading comments appear above their associated node (on a preceding line).
/// Trailing comments appear after code on the same line.
pub struct CommentStore {
    /// Leading comments keyed by the start_byte of the node they precede.
    leading: BTreeMap<usize, Vec<Comment>>,
    /// Trailing comments keyed by the end_byte of the node they follow.
    trailing: BTreeMap<usize, Comment>,
}

impl CommentStore {
    /// Extract all comments from the tree and classify them as leading or trailing.
    pub fn extract(root: Node, source: &str) -> Self {
        let mut comments: Vec<(Node, String)> = Vec::new();
        collect_comments(root, source, &mut comments);

        let mut leading: BTreeMap<usize, Vec<Comment>> = BTreeMap::new();
        let mut trailing: BTreeMap<usize, Comment> = BTreeMap::new();

        for (node, text) in &comments {
            let comment_row = node.start_position().row;

            // Check if there's a non-comment node ending on the same line before this comment.
            if let Some(prev) = find_prev_code_sibling(*node) {
                if prev.end_position().row == comment_row {
                    // Trailing comment: same line as preceding code.
                    trailing.insert(
                        prev.end_byte(),
                        Comment {
                            text: text.clone(),
                            start_byte: node.start_byte(),
                            start_row: comment_row,
                        },
                    );
                    continue;
                }
            }

            // Leading comment: find the next non-comment sibling.
            if let Some(next) = find_next_code_sibling(*node) {
                leading.entry(next.start_byte()).or_default().push(Comment {
                    text: text.clone(),
                    start_byte: node.start_byte(),
                    start_row: comment_row,
                });
            } else {
                // Orphan comment at end of block — attach to parent's end.
                if let Some(parent) = node.parent() {
                    leading.entry(parent.end_byte()).or_default().push(Comment {
                        text: text.clone(),
                        start_byte: node.start_byte(),
                        start_row: comment_row,
                    });
                }
            }
        }

        CommentStore { leading, trailing }
    }

    /// Take (consume) all leading comments associated with a node starting at `node_start_byte`.
    pub fn take_leading(&mut self, node_start_byte: usize) -> Vec<Comment> {
        self.leading.remove(&node_start_byte).unwrap_or_default()
    }

    /// Take (consume) a trailing comment for a node ending at `node_end_byte`
    /// on the given row.
    pub fn take_trailing(&mut self, node_end_byte: usize, _end_row: usize) -> Option<Comment> {
        self.trailing.remove(&node_end_byte)
    }

    /// Check if there are any remaining comments (for debugging/validation).
    #[allow(dead_code)] // For debugging/validation
    pub fn is_empty(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty()
    }

    /// Take all remaining dangling comments (comments that weren't consumed
    /// during formatting, e.g., comments at the end of the file).
    pub fn take_dangling(&mut self) -> Vec<Comment> {
        let mut result: Vec<Comment> = Vec::new();
        for (_, comments) in self.leading.iter() {
            result.extend(comments.iter().cloned());
        }
        for (_, comment) in self.trailing.iter() {
            result.push(comment.clone());
        }
        self.leading.clear();
        self.trailing.clear();
        result.sort_by_key(|c| c.start_byte);
        result
    }
}

/// Recursively collect all comment nodes from the tree.
fn collect_comments<'a>(node: Node<'a>, source: &str, out: &mut Vec<(Node<'a>, String)>) {
    if node.kind() == "comment" {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
        out.push((node, text));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_comments(child, source, out);
    }
}

/// Find the previous named sibling that is not a comment.
fn find_prev_code_sibling(node: Node) -> Option<Node> {
    let mut current = node.prev_sibling();
    while let Some(n) = current {
        if n.kind() != "comment" && n.is_named() {
            return Some(n);
        }
        current = n.prev_sibling();
    }
    None
}

/// Find the next named sibling that is not a comment.
fn find_next_code_sibling(node: Node) -> Option<Node> {
    let mut current = node.next_sibling();
    while let Some(n) = current {
        if n.kind() != "comment" && n.is_named() {
            return Some(n);
        }
        current = n.next_sibling();
    }
    None
}
