mod allocation;
mod loop_invariant;
mod process_get_node;
mod string_concat_loop;

pub use allocation::Allocation;
pub use loop_invariant::LoopInvariant;
pub use process_get_node::ProcessGetNode;
pub use string_concat_loop::StringConcatLoop;

use tree_sitter::Node;

use crate::parser::ParsedFile;

/// Process function names that are called every frame.
pub(super) const PROCESS_FUNCTIONS: &[&str] =
    &["_process", "_physics_process", "_input", "_unhandled_input"];

/// Extract the callable name from a call node.
pub(super) fn get_call_name(node: Node, parsed: &ParsedFile) -> String {
    // Try to get the function field
    if let Some(func_node) = node.child_by_field_name("function") {
        return parsed.node_text(func_node).to_string();
    }
    // Fallback: first child text
    if let Some(first) = node.child(0) {
        return parsed.node_text(first).to_string();
    }
    String::new()
}
