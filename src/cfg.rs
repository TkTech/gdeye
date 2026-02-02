use std::collections::HashMap;

use tree_sitter::Node;

use crate::parser::ParsedFile;

/// Find the name of a node by field or child kind.
fn find_node_name(node: Node, parsed: &ParsedFile) -> Option<String> {
    if let Some(n) = node.child_by_field_name("name") {
        return Some(parsed.node_text(n).to_string());
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            if child.kind() == "name" {
                return Some(parsed.node_text(child).to_string());
            }
        }
    }
    None
}

/// The kind of edge in the control flow graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// Normal control flow (sequential, branch, loop)
    Normal,
    /// Suspension point entering an await expression
    AwaitSuspend,
    /// Resumption point after an await expression completes
    AwaitResume,
}

/// A control flow graph for a single function.
#[derive(Debug)]
pub struct Cfg {
    pub function_name: String,
    pub blocks: Vec<BasicBlock>,
    /// Edges: from block index -> to block indices (simple view)
    pub edges: HashMap<usize, Vec<usize>>,
    /// Edges with kind annotation: from block index -> to (block, kind) pairs
    pub typed_edges: HashMap<usize, Vec<(usize, EdgeKind)>>,
    #[allow(dead_code)] // Used for CFG traversal entry point
    pub entry: usize,
    #[allow(dead_code)] // Used for CFG traversal exit point
    pub exit: usize,
}

/// A basic block: a sequence of statements with no branches.
#[derive(Debug)]
pub struct BasicBlock {
    pub id: usize,
    /// Line range of the statements in this block
    pub start_line: usize,
    pub end_line: usize,
    /// Variable definitions in this block (name, line, is_var_declaration)
    /// is_var_declaration is true for `var x = ...` statements, false for reassignments
    pub definitions: Vec<(String, usize, bool)>,
    /// Variable uses in this block (name, line)
    pub uses: Vec<(String, usize)>,
    /// Await expressions in this block (line numbers)
    pub awaits: Vec<usize>,
    /// Whether this block ends with a return statement
    pub has_return: bool,
    /// Whether this block ends with break/continue
    pub has_break: bool,
    pub has_continue: bool,
    /// Whether this block contains assert(false) which always terminates
    pub has_assert_false: bool,
    /// Lines with push_error() calls (informational tracking)
    pub error_calls: Vec<usize>,
    /// Type refinements active in this block (var_name -> narrowed_type)
    /// These come from `if x is Type:` patterns where this is the then-branch
    pub type_refinements: HashMap<String, String>,
}

/// Build control flow graphs for all functions in a parsed file.
pub fn build_cfgs(parsed: &ParsedFile) -> Vec<Cfg> {
    let root = parsed.root_node();
    let mut cfgs = Vec::new();

    let func_nodes = crate::parser::find_nodes_by_kind(root, "function_definition");
    for func_node in func_nodes {
        if let Some(cfg) = build_cfg_for_function(func_node, parsed) {
            cfgs.push(cfg);
        }
    }

    cfgs
}

fn build_cfg_for_function(func_node: Node, parsed: &ParsedFile) -> Option<Cfg> {
    let name = func_node
        .child_by_field_name("name")
        .map(|n| parsed.node_text(n).to_string())?;

    let body = func_node.child_by_field_name("body")?;

    let mut builder = CfgBuilder::new();
    builder.build_from_body(body, parsed);

    // Ensure we have at least entry and exit blocks
    if builder.blocks.is_empty() {
        builder.blocks.push(BasicBlock {
            id: 0,
            start_line: func_node.start_position().row + 1,
            end_line: func_node.end_position().row + 1,
            definitions: Vec::new(),
            uses: Vec::new(),
            awaits: Vec::new(),
            has_return: false,
            has_break: false,
            has_continue: false,
            has_assert_false: false,
            error_calls: Vec::new(),
            type_refinements: HashMap::new(),
        });
    }

    let exit = builder.blocks.len() - 1;

    Some(Cfg {
        function_name: name,
        blocks: builder.blocks,
        edges: builder.edges.clone(),
        typed_edges: builder.typed_edges,
        entry: 0,
        exit,
    })
}

/// Find a call node within an expression statement.
fn find_call_in_expression(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "call" {
            return Some(child);
        }
        if let Some(call) = find_call_in_expression(child) {
            return Some(call);
        }
    }
    None
}

/// Extract type narrowing pattern from `x is Type` expression.
/// Returns (variable_name, type_name) if the pattern matches.
fn extract_is_pattern(cond: Node, parsed: &ParsedFile) -> Option<(String, String)> {
    // Look for pattern: identifier "is" identifier
    // The "is" expression might be represented as a binary_operator or comparison node
    if cond.kind() == "comparison_operator" || cond.kind() == "binary_operator" {
        let mut cursor = cond.walk();
        let children: Vec<_> = cond.children(&mut cursor).collect();

        if children.len() >= 3 {
            // Check if operator is "is"
            let op = children.get(1)?;
            if parsed.node_text(*op) == "is" {
                let lhs = children.first()?;
                let rhs = children.get(2)?;

                // LHS should be an identifier (variable name)
                if lhs.kind() == "identifier" || lhs.kind() == "name" {
                    let var_name = parsed.node_text(*lhs).to_string();

                    // RHS should be a type name (identifier)
                    if rhs.kind() == "identifier" || rhs.kind() == "name" {
                        let type_name = parsed.node_text(*rhs).to_string();
                        return Some((var_name, type_name));
                    }
                }
            }
        }
    }

    // Also handle the case where `is` is a direct child kind
    // (tree-sitter grammar variations)
    let mut cursor = cond.walk();
    for child in cond.children(&mut cursor) {
        if let Some(result) = extract_is_pattern(child, parsed) {
            return Some(result);
        }
    }

    None
}

/// Check if a call node is `assert(false)` or `assert(false, msg)`.
fn is_assert_false(call: Node, parsed: &ParsedFile) -> bool {
    // Find arguments node
    let mut cursor = call.walk();
    for child in call.children(&mut cursor) {
        if child.kind() == "arguments" {
            let mut arg_cursor = child.walk();
            for arg in child.children(&mut arg_cursor) {
                if arg.kind() == "false" {
                    return true;
                }
                // Only check the first actual argument (skip parentheses)
                if arg.is_named() && arg.kind() != "(" && arg.kind() != ")" {
                    // Check if it's a `false` literal
                    return parsed.node_text(arg) == "false";
                }
            }
        }
    }
    false
}

struct CfgBuilder {
    blocks: Vec<BasicBlock>,
    edges: HashMap<usize, Vec<usize>>,
    typed_edges: HashMap<usize, Vec<(usize, EdgeKind)>>,
    current_block: usize,
}

impl CfgBuilder {
    fn new() -> Self {
        let entry_block = BasicBlock {
            id: 0,
            start_line: 0,
            end_line: 0,
            definitions: Vec::new(),
            uses: Vec::new(),
            awaits: Vec::new(),
            has_return: false,
            has_break: false,
            has_continue: false,
            has_assert_false: false,
            error_calls: Vec::new(),
            type_refinements: HashMap::new(),
        };
        Self {
            blocks: vec![entry_block],
            edges: HashMap::new(),
            typed_edges: HashMap::new(),
            current_block: 0,
        }
    }

    fn new_block(&mut self, start_line: usize) -> usize {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock {
            id,
            start_line,
            end_line: start_line,
            definitions: Vec::new(),
            uses: Vec::new(),
            awaits: Vec::new(),
            has_return: false,
            has_break: false,
            has_continue: false,
            has_assert_false: false,
            error_calls: Vec::new(),
            type_refinements: HashMap::new(),
        });
        id
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        self.add_typed_edge(from, to, EdgeKind::Normal);
    }

    fn add_typed_edge(&mut self, from: usize, to: usize, kind: EdgeKind) {
        self.edges.entry(from).or_default().push(to);
        self.typed_edges.entry(from).or_default().push((to, kind));
    }

    fn build_from_body(&mut self, body: Node, parsed: &ParsedFile) {
        self.blocks[0].start_line = body.start_position().row + 1;
        self.process_statements(body, parsed);
        self.blocks[self.current_block].end_line = body.end_position().row + 1;
    }

    fn process_statements(&mut self, node: Node, parsed: &ParsedFile) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "variable_statement" => {
                    self.process_var_def(child, parsed);
                }
                "expression_statement" => {
                    self.process_expression(child, parsed);
                }
                "return_statement" => {
                    // Collect uses from the return expression
                    self.collect_uses(child, parsed);
                    self.blocks[self.current_block].has_return = true;
                    self.blocks[self.current_block].end_line = child.end_position().row + 1;
                }
                "break_statement" => {
                    self.blocks[self.current_block].has_break = true;
                }
                "continue_statement" => {
                    self.blocks[self.current_block].has_continue = true;
                }
                "if_statement" => {
                    self.process_if(child, parsed);
                }
                "for_statement" => {
                    self.process_for(child, parsed);
                }
                "while_statement" => {
                    self.process_while(child, parsed);
                }
                "match_statement" => {
                    self.process_match(child, parsed);
                }
                _ => {
                    // Collect identifier uses from any unhandled statement type
                    self.collect_uses(child, parsed);
                }
            }
        }
    }

    fn process_var_def(&mut self, node: Node, parsed: &ParsedFile) {
        let name = find_node_name(node, parsed);
        let has_initializer = node.child_by_field_name("value").is_some();

        if let Some(name) = name {
            let line = node.start_position().row + 1;
            // Only record as a definition if there's an initializer.
            // `var x: Type` without `= value` doesn't assign anything.
            if has_initializer {
                self.blocks[self.current_block]
                    .definitions
                    .push((name, line, true));
            }
        }
        // Also track uses in the initializer
        if let Some(value) = node.child_by_field_name("value") {
            self.collect_uses(value, parsed);
        }
    }

    fn process_expression(&mut self, node: Node, parsed: &ParsedFile) {
        // Check for terminating assert(false) or push_error() calls
        if let Some(call) = find_call_in_expression(node) {
            let func_name = call.child(0).map(|n| parsed.node_text(n)).unwrap_or("");
            let line = node.start_position().row + 1;

            if func_name == "assert" && is_assert_false(call, parsed) {
                self.blocks[self.current_block].has_assert_false = true;
                self.blocks[self.current_block].end_line = line;
            } else if func_name == "push_error" {
                self.blocks[self.current_block].error_calls.push(line);
            }
        }

        // Walk children to find assignment nodes and handle them specially
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "assignment" {
                self.process_assignment(child, parsed);
            } else if child.kind() == "augmented_assignment" {
                self.process_augmented_assignment(child, parsed);
            } else {
                self.collect_uses(child, parsed);
            }
        }
    }

    fn process_assignment(&mut self, node: Node, parsed: &ParsedFile) {
        // Assignment: LHS is a definition, RHS is a use.
        // For simple `x = expr`, record x as a definition and collect uses from expr.
        // For property/subscript assignments like `obj.x = expr` or `arr[i] = expr`,
        // treat the whole LHS as uses (the object/array is being used, not defined).
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        if children.len() >= 2 {
            let lhs = children[0];
            // Simple identifier assignment: record as definition
            if lhs.kind() == "identifier" || lhs.kind() == "name" {
                let name = parsed.node_text(lhs).to_string();
                let line = lhs.start_position().row + 1;
                self.blocks[self.current_block]
                    .definitions
                    .push((name, line, false));
            } else {
                // Property access, subscript, etc. - LHS is a use
                self.collect_uses(lhs, parsed);
            }
            // Collect uses from the RHS (skip operator token)
            for child in &children[1..] {
                if child.kind() != "=" {
                    self.collect_uses(*child, parsed);
                }
            }
        }
    }

    fn process_augmented_assignment(&mut self, node: Node, parsed: &ParsedFile) {
        // Augmented assignment (+=, -=, etc.): LHS is both a use AND a definition.
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();

        if children.len() >= 2 {
            let lhs = children[0];
            if lhs.kind() == "identifier" || lhs.kind() == "name" {
                let name = parsed.node_text(lhs).to_string();
                let line = lhs.start_position().row + 1;
                // It's a use (reads old value) and a definition (writes new value)
                self.blocks[self.current_block]
                    .uses
                    .push((name.clone(), line));
                self.blocks[self.current_block]
                    .definitions
                    .push((name, line, false));
            } else {
                self.collect_uses(lhs, parsed);
            }
            // Collect uses from the RHS (skip operator token like +=, -=, etc.)
            for child in &children[1..] {
                let kind = child.kind();
                if kind != "+="
                    && kind != "-="
                    && kind != "*="
                    && kind != "/="
                    && kind != "%="
                    && kind != "&="
                    && kind != "|="
                    && kind != "^="
                    && kind != "<<="
                    && kind != ">>="
                {
                    self.collect_uses(*child, parsed);
                }
            }
        }
    }

    fn collect_uses(&mut self, node: Node, parsed: &ParsedFile) {
        // Skip assignment nodes here - they should be handled by process_expression
        if node.kind() == "assignment" {
            self.process_assignment(node, parsed);
            return;
        }
        if node.kind() == "augmented_assignment" {
            self.process_augmented_assignment(node, parsed);
            return;
        }
        // For attribute access (e.g., obj.method or obj.method(args)), collect uses from
        // the object and any method call arguments, but not the attribute/method name itself.
        if node.kind() == "attribute" {
            let first_child_start = node.child(0).map(|c| c.start_byte());
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                // The first child is the object being accessed - can be any expression
                // (identifier, parenthesized_expression, binary_operator, call, etc.)
                if Some(child.start_byte()) == first_child_start {
                    self.collect_uses(child, parsed);
                    continue;
                }
                match child.kind() {
                    // Method call with arguments - collect uses from arguments
                    "attribute_call" => {
                        if let Some(args) = child.child_by_field_name("arguments") {
                            self.collect_uses(args, parsed);
                        } else {
                            // Fallback: collect from all children of attribute_call except the name
                            let mut inner_cursor = child.walk();
                            for inner in child.children(&mut inner_cursor) {
                                if inner.kind() != "identifier" && inner.kind() != "name" {
                                    self.collect_uses(inner, parsed);
                                }
                            }
                        }
                    }
                    // Skip the attribute name (e.g., "normalized" in expr.normalized())
                    "identifier" | "name" => {}
                    _ => {}
                }
            }
            return;
        }
        if node.kind() == "identifier" || node.kind() == "name" {
            let name = parsed.node_text(node).to_string();
            let line = node.start_position().row + 1;
            self.blocks[self.current_block].uses.push((name, line));
        }
        // Track await expressions as suspension points and create new blocks
        if node.kind() == "await" || node.kind() == "await_expression" {
            let line = node.start_position().row + 1;
            self.blocks[self.current_block].awaits.push(line);

            // Create a continuation block for after the await resumes
            let current = self.current_block;
            let continuation = self.new_block(line);

            // Add typed edges: suspend to continuation (marking the await boundary)
            self.add_typed_edge(current, continuation, EdgeKind::AwaitSuspend);

            // Move to continuation block
            self.current_block = continuation;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.collect_uses(child, parsed);
        }
    }

    fn process_if(&mut self, node: Node, parsed: &ParsedFile) {
        // Collect uses from the if condition in the current block
        if let Some(cond) = node.child_by_field_name("condition") {
            self.collect_uses(cond, parsed);
        }

        // Check for type narrowing pattern: `if x is Type:`
        let type_refinement = node
            .child_by_field_name("condition")
            .and_then(|cond| extract_is_pattern(cond, parsed));

        let pre_if_block = self.current_block;
        let merge_block = self.new_block(node.end_position().row + 1);
        let mut cursor = node.walk();
        let mut branch_blocks = Vec::new();
        let mut has_else = false;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "body" => {
                    // The if-true body (direct child of if_statement)
                    let branch_block = self.new_block(child.start_position().row + 1);
                    self.add_edge(pre_if_block, branch_block);

                    // Apply type refinement to the then-branch
                    if let Some((var_name, type_name)) = &type_refinement {
                        self.blocks[branch_block]
                            .type_refinements
                            .insert(var_name.clone(), type_name.clone());
                    }

                    self.current_block = branch_block;
                    self.process_statements(child, parsed);
                    branch_blocks.push(self.current_block);
                }
                "else_clause" | "elif_clause" => {
                    // Only a plain else_clause guarantees exhaustive coverage.
                    // elif_clause is another conditional branch that may not execute.
                    if child.kind() == "else_clause" {
                        has_else = true;
                    }
                    let else_block = self.new_block(child.start_position().row + 1);
                    self.add_edge(pre_if_block, else_block);
                    self.current_block = else_block;
                    // else_clause contains either a body (plain else) or
                    // an if_statement (elif chain).
                    // elif_clause has a condition expression + body.
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        match inner.kind() {
                            "body" => {
                                self.process_statements(inner, parsed);
                            }
                            "if_statement" => {
                                self.process_if(inner, parsed);
                            }
                            _ => {
                                // Collect uses from elif conditions and other expressions
                                self.collect_uses(inner, parsed);
                            }
                        }
                    }
                    branch_blocks.push(self.current_block);
                }
                _ => {
                    // Collect uses from condition and other expressions
                    if child.kind() != "body" {
                        let saved = self.current_block;
                        self.current_block = pre_if_block;
                        self.collect_uses(child, parsed);
                        self.current_block = saved;
                    }
                }
            }
        }

        // All branches merge
        for bb in &branch_blocks {
            if !self.block_terminates(*bb) {
                self.add_edge(*bb, merge_block);
            }
        }
        // If no else, the condition-false path goes directly to merge
        if !has_else {
            self.add_edge(pre_if_block, merge_block);
        }
        self.current_block = merge_block;
    }

    fn process_for(&mut self, node: Node, parsed: &ParsedFile) {
        // For statement structure: first identifier/name is the loop variable,
        // remaining non-body children are the iterator expression.
        let mut skipped_loop_var = false;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "body" {
                continue;
            }
            if !skipped_loop_var && (child.kind() == "identifier" || child.kind() == "name") {
                // This is the loop variable - record as definition, not use
                let name = parsed.node_text(child).to_string();
                let line = child.start_position().row + 1;
                self.blocks[self.current_block]
                    .definitions
                    .push((name, line, true));
                skipped_loop_var = true;
                continue;
            }
            // Everything else is part of the iterator expression
            self.collect_uses(child, parsed);
        }

        let pre_loop = self.current_block;
        let loop_body = self.new_block(node.start_position().row + 1);
        let after_loop = self.new_block(node.end_position().row + 1);

        self.add_edge(pre_loop, loop_body);
        self.add_edge(pre_loop, after_loop); // loop might not execute

        self.current_block = loop_body;
        if let Some(body) = node.child_by_field_name("body") {
            self.process_statements(body, parsed);
        }

        // Back edge
        if !self.block_terminates(self.current_block) {
            self.add_edge(self.current_block, loop_body);
            self.add_edge(self.current_block, after_loop);
        }

        self.current_block = after_loop;
    }

    fn process_while(&mut self, node: Node, parsed: &ParsedFile) {
        let pre_loop = self.current_block;
        let cond_block = self.new_block(node.start_position().row + 1);
        let loop_body = self.new_block(node.start_position().row + 1);
        let after_loop = self.new_block(node.end_position().row + 1);

        // Collect uses from the condition in the condition block
        self.current_block = cond_block;
        if let Some(cond) = node.child_by_field_name("condition") {
            self.collect_uses(cond, parsed);
        }

        self.add_edge(pre_loop, cond_block);
        self.add_edge(cond_block, loop_body);
        self.add_edge(cond_block, after_loop);

        self.current_block = loop_body;
        if let Some(body) = node.child_by_field_name("body") {
            self.process_statements(body, parsed);
        }

        // Back edge to condition
        if !self.block_terminates(self.current_block) {
            self.add_edge(self.current_block, cond_block);
        }

        self.current_block = after_loop;
    }

    fn process_match(&mut self, node: Node, parsed: &ParsedFile) {
        // Collect uses from the match subject (direct children that aren't
        // match_body, e.g. an identifier or expression)
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() != "match_body" {
                self.collect_uses(child, parsed);
            }
        }

        let pre_match = self.current_block;
        let merge_block = self.new_block(node.end_position().row + 1);

        // Track if we have a catch-all pattern (_, var x, etc.)
        let mut has_catch_all = false;

        // Pattern sections live inside a match_body wrapper node
        let mut outer_cursor = node.walk();
        for child in node.children(&mut outer_cursor) {
            if child.kind() == "match_body" {
                let mut inner_cursor = child.walk();
                for section in child.children(&mut inner_cursor) {
                    if section.kind() == "pattern_section" || section.kind() == "match_branch" {
                        if self.is_catch_all_pattern(section, parsed) {
                            has_catch_all = true;
                        }
                        self.process_match_branch(section, parsed, pre_match, merge_block);
                    }
                }
            }
            // Also handle pattern_section as a direct child (grammar variations)
            if child.kind() == "pattern_section" || child.kind() == "match_branch" {
                if self.is_catch_all_pattern(child, parsed) {
                    has_catch_all = true;
                }
                self.process_match_branch(child, parsed, pre_match, merge_block);
            }
        }

        // GDScript match is not necessarily exhaustive, so add a fallthrough
        // edge for the case where no pattern matches - but only if there's
        // no catch-all pattern like `_` or `var x`.
        if !has_catch_all {
            self.add_edge(pre_match, merge_block);
        }

        self.current_block = merge_block;
    }

    /// Check if a pattern section contains a catch-all pattern (_, var x, etc.)
    fn is_catch_all_pattern(&self, section: Node, parsed: &ParsedFile) -> bool {
        let mut cursor = section.walk();
        for child in section.children(&mut cursor) {
            // Skip the body - we only care about the pattern itself
            if child.kind() == "body" {
                continue;
            }
            // Check for underscore pattern `_`
            if child.kind() == "identifier" || child.kind() == "name" {
                let text = parsed.node_text(child);
                if text == "_" {
                    return true;
                }
            }
            // Check for binding pattern `var x` which also catches everything
            if child.kind() == "pattern_binding" {
                return true;
            }
            // Recursively check pattern nodes
            if child.kind() == "pattern" {
                if self.is_catch_all_pattern(child, parsed) {
                    return true;
                }
            }
        }
        false
    }

    fn process_match_branch(
        &mut self,
        section: Node,
        parsed: &ParsedFile,
        pre_match: usize,
        merge_block: usize,
    ) {
        let branch_block = self.new_block(section.start_position().row + 1);
        self.add_edge(pre_match, branch_block);
        self.current_block = branch_block;

        // Find the body node inside the pattern section
        let mut cursor = section.walk();
        for child in section.children(&mut cursor) {
            if child.kind() == "body" {
                self.process_statements(child, parsed);
            }
        }

        if !self.block_terminates(self.current_block) {
            self.add_edge(self.current_block, merge_block);
        }
    }

    /// Check if a block terminates (return, break, or continue).
    /// For edge computation purposes, continue/break prevent normal flow to merge blocks.
    fn block_terminates(&self, block_id: usize) -> bool {
        let block = &self.blocks[block_id];
        block.has_return || block.has_break || block.has_continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;

    /// Helper to build CFG from source and return the first function's CFG.
    fn cfg_from_source(source: &str) -> Option<Cfg> {
        let parsed = parse_source(source).ok()?;
        let cfgs = build_cfgs(&parsed);
        cfgs.into_iter().next()
    }

    #[test]
    fn simple_function_has_one_block() {
        let source = r#"
func foo():
    var x = 1
    var y = 2
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert_eq!(cfg.function_name, "foo");
        assert!(!cfg.blocks.is_empty());
        // Simple linear code should have few blocks
        assert!(
            cfg.blocks.len() <= 2,
            "Expected at most 2 blocks, got {}",
            cfg.blocks.len()
        );
    }

    #[test]
    fn if_statement_creates_branches() {
        let source = r#"
func foo():
    if true:
        var x = 1
    else:
        var y = 2
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        // If-else should create multiple blocks
        assert!(
            cfg.blocks.len() >= 3,
            "Expected at least 3 blocks for if-else, got {}",
            cfg.blocks.len()
        );
    }

    #[test]
    fn while_loop_creates_back_edge() {
        let source = r#"
func foo():
    while true:
        var x = 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        // While loop should have at least a condition block and body block
        assert!(cfg.blocks.len() >= 2);
        // Check for back edge (some block should point back to an earlier block)
        let has_back_edge = cfg
            .edges
            .iter()
            .any(|(from, tos)| tos.iter().any(|to| *to <= *from));
        assert!(has_back_edge, "While loop should create a back edge");
    }

    #[test]
    fn for_loop_creates_blocks() {
        let source = r#"
func foo():
    for i in range(10):
        var x = i
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.len() >= 2,
            "For loop should create at least 2 blocks"
        );
    }

    #[test]
    fn variable_definitions_collected() {
        let source = r#"
func foo():
    var x = 1
    var y = 2
    x = 3
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        let all_defs: Vec<_> = cfg
            .blocks
            .iter()
            .flat_map(|b| b.definitions.iter())
            .collect();
        // Should have definitions for x (twice: var and reassign) and y
        assert!(
            all_defs.iter().any(|(name, _, _)| name == "x"),
            "Should find x definition"
        );
        assert!(
            all_defs.iter().any(|(name, _, _)| name == "y"),
            "Should find y definition"
        );
    }

    #[test]
    fn variable_uses_collected() {
        let source = r#"
func foo():
    var x = 1
    var y = x + 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        let all_uses: Vec<_> = cfg.blocks.iter().flat_map(|b| b.uses.iter()).collect();
        // Should have a use of x in the second assignment
        assert!(
            all_uses.iter().any(|(name, _)| name == "x"),
            "Should find x use"
        );
    }

    #[test]
    fn return_statement_tracked() {
        let source = r#"
func foo():
    return 42
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| b.has_return),
            "Should track return statement"
        );
    }

    #[test]
    fn break_continue_tracked() {
        let source = r#"
func foo():
    while true:
        if true:
            break
        else:
            continue
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| b.has_break),
            "Should track break statement"
        );
        assert!(
            cfg.blocks.iter().any(|b| b.has_continue),
            "Should track continue statement"
        );
    }

    #[test]
    fn await_tracked() {
        let source = r#"
func foo():
    await some_signal
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| !b.awaits.is_empty()),
            "Should track await"
        );
    }

    #[test]
    fn match_statement_branches() {
        let source = r#"
func foo(x):
    match x:
        1:
            var a = 1
        2:
            var b = 2
        _:
            var c = 3
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        // Match with 3 patterns should create multiple blocks
        assert!(cfg.blocks.len() >= 3, "Match should create multiple blocks");
    }

    #[test]
    fn empty_function_has_block() {
        let source = r#"
func foo():
    pass
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            !cfg.blocks.is_empty(),
            "Empty function should still have a block"
        );
    }

    #[test]
    fn augmented_assignment_is_use_and_def() {
        let source = r#"
func foo():
    var x = 1
    x += 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        let all_defs: Vec<_> = cfg
            .blocks
            .iter()
            .flat_map(|b| b.definitions.iter())
            .filter(|(name, _, _)| name == "x")
            .collect();
        let all_uses: Vec<_> = cfg
            .blocks
            .iter()
            .flat_map(|b| b.uses.iter())
            .filter(|(name, _)| name == "x")
            .collect();
        // x should be defined twice (var x = 1, and x += 1)
        assert!(all_defs.len() >= 2, "x should have at least 2 definitions");
        // x should be used at least once (in x += 1)
        assert!(!all_uses.is_empty(), "x should have at least 1 use from +=");
    }

    #[test]
    fn assert_false_terminates_block() {
        let source = r#"
func foo():
    assert(false)
    var x = 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| b.has_assert_false),
            "Should track assert(false)"
        );
    }

    #[test]
    fn assert_false_with_message_terminates() {
        let source = r#"
func foo():
    assert(false, "error message")
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| b.has_assert_false),
            "Should track assert(false, msg)"
        );
    }

    #[test]
    fn push_error_tracked() {
        let source = r#"
func foo():
    push_error("something went wrong")
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            cfg.blocks.iter().any(|b| !b.error_calls.is_empty()),
            "Should track push_error calls"
        );
    }

    #[test]
    fn assert_true_does_not_terminate() {
        let source = r#"
func foo():
    assert(true)
    var x = 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");
        assert!(
            !cfg.blocks.iter().any(|b| b.has_assert_false),
            "assert(true) should not terminate"
        );
    }

    #[test]
    fn await_creates_suspension_edge() {
        let source = r#"
func foo():
    await some_signal
    var x = 1
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");

        // Should have typed edges
        assert!(
            !cfg.typed_edges.is_empty(),
            "Should have typed edges for await"
        );

        // Check that we have an AwaitSuspend edge
        let has_suspend_edge = cfg
            .typed_edges
            .values()
            .flat_map(|v| v.iter())
            .any(|(_, kind)| *kind == EdgeKind::AwaitSuspend);

        assert!(has_suspend_edge, "Should have AwaitSuspend edge type");
    }

    #[test]
    fn await_splits_blocks() {
        let source = r#"
func foo():
    var x = 1
    await some_signal
    var y = 2
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");

        // Should have more blocks due to await splitting
        assert!(
            cfg.blocks.len() >= 2,
            "Await should split into multiple blocks, got {}",
            cfg.blocks.len()
        );
    }

    #[test]
    fn type_narrowing_is_pattern() {
        let source = r#"
func foo(x):
    if x is Node:
        print(x)
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");

        // Find a block with type refinement for 'x' to 'Node'
        let has_refinement = cfg
            .blocks
            .iter()
            .any(|b| b.type_refinements.get("x") == Some(&"Node".to_string()));

        assert!(
            has_refinement,
            "Should have type refinement for x -> Node in then-branch"
        );
    }

    #[test]
    fn type_narrowing_not_in_else() {
        let source = r#"
func foo(x):
    if x is Node:
        print("is node")
    else:
        print("not node")
"#;
        let cfg = cfg_from_source(source).expect("Should build CFG");

        // Count blocks with the refinement
        let refinement_count = cfg
            .blocks
            .iter()
            .filter(|b| b.type_refinements.get("x") == Some(&"Node".to_string()))
            .count();

        // Should only be in one block (the then-branch), not the else-branch
        assert_eq!(
            refinement_count, 1,
            "Type refinement should only be in then-branch, not else"
        );
    }
}
