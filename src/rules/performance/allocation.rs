use tree_sitter::Node;

use crate::parser::{self, ParsedFile};

use super::super::{Diagnostic, OptionType, Rule, RuleContext, RuleOption, Severity};
use super::get_call_name;

const RULE_ID: &str = "perf/allocation";

/// Default minimum array elements to flag as allocation.
const DEFAULT_MIN_ARRAY_ELEMENTS: usize = 5;

/// Default minimum dictionary pairs to flag as allocation.
const DEFAULT_MIN_DICT_PAIRS: usize = 3;

/// Types that allocate when constructed.
const ALLOCATING_TYPES: &[&str] = &[
    "Array",
    "Dictionary",
    "PackedByteArray",
    "PackedFloat32Array",
    "PackedFloat64Array",
    "PackedInt32Array",
    "PackedInt64Array",
    "PackedStringArray",
    "PackedVector2Array",
    "PackedVector3Array",
    "PackedColorArray",
];

/// Process function names that are called every frame (hot paths).
const PROCESS_FUNCTIONS: &[&str] = &["_process", "_physics_process", "_input", "_unhandled_input"];

/// Unified allocation detection rule for hot code paths.
///
/// Detects allocations in:
/// - Process functions (_process, _physics_process, _input, _unhandled_input)
/// - For/while loops
/// - Nested combinations (loop inside process function)
///
/// Consolidates: perf/process-allocation, perf/allocation-in-loop
pub struct Allocation;

impl Rule for Allocation {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Allocation in performance-critical code (process functions or loops)"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "performance"
    }

    fn options(&self) -> Vec<RuleOption> {
        vec![
            RuleOption {
                name: "min_array_elements",
                description: "Minimum array literal elements to flag as allocation",
                default: "5",
                value_type: OptionType::Integer,
            },
            RuleOption {
                name: "min_dict_pairs",
                description: "Minimum dictionary literal key-value pairs to flag as allocation",
                default: "3",
                value_type: OptionType::Integer,
            },
        ]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let min_array_elements = ctx
            .config
            .rule_option(RULE_ID, "min_array_elements")
            .and_then(|v| v.as_integer())
            .unwrap_or(DEFAULT_MIN_ARRAY_ELEMENTS as i64) as usize;

        let min_dict_pairs = ctx
            .config
            .rule_option(RULE_ID, "min_dict_pairs")
            .and_then(|v| v.as_integer())
            .unwrap_or(DEFAULT_MIN_DICT_PAIRS as i64) as usize;

        let mut diagnostics = Vec::new();
        let config = AllocationConfig {
            min_array_elements,
            min_dict_pairs,
        };

        check_allocations(ctx.parsed, &config, &mut diagnostics);
        diagnostics
    }
}

struct AllocationConfig {
    min_array_elements: usize,
    min_dict_pairs: usize,
}

/// Context describing why code is "hot" (performance-critical).
#[derive(Clone)]
enum HotContext {
    /// Inside a process function like _process or _physics_process
    ProcessFunction(String),
    /// Inside a for or while loop
    Loop(String), // "for" or "while"
    /// Inside a loop that's inside a process function
    LoopInProcess {
        func_name: String,
        loop_kind: String,
    },
}

impl HotContext {
    fn description(&self) -> String {
        match self {
            HotContext::ProcessFunction(name) => format!("`{}`", name),
            HotContext::Loop(kind) => format!("`{}` loop", kind),
            HotContext::LoopInProcess {
                func_name,
                loop_kind,
            } => format!("`{}` loop inside `{}`", loop_kind, func_name),
        }
    }

    fn suggestion(&self) -> &'static str {
        match self {
            HotContext::ProcessFunction(_) => "Consider caching as a member variable.",
            HotContext::Loop(_) => "Consider moving outside the loop or using object pooling.",
            HotContext::LoopInProcess { .. } => {
                "Consider caching as a member variable or moving outside the loop."
            }
        }
    }

    /// Nest a loop context inside the current context.
    fn with_loop(&self, loop_kind: &str) -> HotContext {
        match self {
            HotContext::ProcessFunction(func_name) => HotContext::LoopInProcess {
                func_name: func_name.clone(),
                loop_kind: loop_kind.to_string(),
            },
            HotContext::Loop(_) => HotContext::Loop(loop_kind.to_string()),
            HotContext::LoopInProcess { func_name, .. } => HotContext::LoopInProcess {
                func_name: func_name.clone(),
                loop_kind: loop_kind.to_string(),
            },
        }
    }
}

fn check_allocations(
    parsed: &ParsedFile,
    config: &AllocationConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    let functions = parser::find_nodes_by_kind(root, "function_definition");

    for func in functions {
        let func_name = match func.child_by_field_name("name") {
            Some(n) => parsed.node_text(n),
            None => continue,
        };

        if let Some(body) = func.child_by_field_name("body") {
            if PROCESS_FUNCTIONS.contains(&func_name) {
                // We're in a process function - this is a hot context
                let ctx = HotContext::ProcessFunction(func_name.to_string());
                check_node_allocations(body, parsed, config, Some(&ctx), &[], diagnostics);
            } else {
                // Regular function - only check loops inside it
                check_node_allocations(body, parsed, config, None, &[], diagnostics);
            }
        }
    }
}

/// Recursively check a node for allocations in hot contexts.
///
/// - `hot_ctx`: The current hot context (process function, loop, or both)
/// - `loop_vars`: Variables assigned within the current loop (for invariance checking)
fn check_node_allocations(
    node: Node,
    parsed: &ParsedFile,
    config: &AllocationConfig,
    hot_ctx: Option<&HotContext>,
    loop_vars: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Don't recurse into nested function definitions or lambdas
    if node.kind() == "function_definition" || node.kind() == "lambda" {
        return;
    }

    // Check for loop entry points
    if node.kind() == "for_statement" || node.kind() == "while_statement" {
        let loop_kind = if node.kind() == "for_statement" {
            "for"
        } else {
            "while"
        };

        // Create or update hot context
        let new_ctx = match hot_ctx {
            Some(ctx) => ctx.with_loop(loop_kind),
            None => HotContext::Loop(loop_kind.to_string()),
        };

        // Collect loop variables for invariance checking
        let mut new_loop_vars = loop_vars.to_vec();
        collect_loop_variables(node, parsed, &mut new_loop_vars);

        if let Some(body) = node.child_by_field_name("body") {
            collect_assigned_variables(body, parsed, &mut new_loop_vars);
            check_node_allocations(
                body,
                parsed,
                config,
                Some(&new_ctx),
                &new_loop_vars,
                diagnostics,
            );
        }
        return; // Don't process children again
    }

    // Only report allocations if we're in a hot context
    if let Some(ctx) = hot_ctx {
        check_allocation_at_node(node, parsed, config, ctx, loop_vars, diagnostics);
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_node_allocations(child, parsed, config, hot_ctx, loop_vars, diagnostics);
    }
}

/// Check if a specific node represents an allocation.
fn check_allocation_at_node(
    node: Node,
    parsed: &ParsedFile,
    config: &AllocationConfig,
    hot_ctx: &HotContext,
    loop_vars: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node.kind() {
        "call" => {
            let call_text = get_call_name(node, parsed);

            // Check for allocating type constructors
            if ALLOCATING_TYPES.contains(&call_text.as_str()) {
                emit_diagnostic(
                    node,
                    format!(
                        "`{}()` allocation inside {}. {}",
                        call_text,
                        hot_ctx.description(),
                        hot_ctx.suggestion()
                    ),
                    diagnostics,
                );
            }

            // Check for .new() calls (object instantiation)
            if call_text.ends_with(".new") {
                emit_diagnostic(
                    node,
                    format!(
                        "Object instantiation `{}()` inside {}. {}",
                        call_text,
                        hot_ctx.description(),
                        hot_ctx.suggestion()
                    ),
                    diagnostics,
                );
            }
        }
        "array" => {
            let element_count = count_array_elements(node);

            // In process functions (not loops), flag any non-empty array
            // In loops, use the configurable threshold
            let threshold = match hot_ctx {
                HotContext::ProcessFunction(_) => 1,
                HotContext::Loop(_) | HotContext::LoopInProcess { .. } => config.min_array_elements,
            };

            if element_count >= threshold {
                // For loops, check if array depends on loop variables
                if matches!(
                    hot_ctx,
                    HotContext::Loop(_) | HotContext::LoopInProcess { .. }
                ) && node_references_any(node, loop_vars, parsed)
                {
                    return; // Depends on loop var, can't be hoisted
                }

                emit_diagnostic(
                    node,
                    format!(
                        "Array literal allocation inside {}. {}",
                        hot_ctx.description(),
                        hot_ctx.suggestion()
                    ),
                    diagnostics,
                );
            }
        }
        "dictionary" => {
            let kv_count = count_dict_elements(node);

            // In process functions, flag any dictionary
            // In loops, use the configurable threshold
            let threshold = match hot_ctx {
                HotContext::ProcessFunction(_) => 0,
                HotContext::Loop(_) | HotContext::LoopInProcess { .. } => config.min_dict_pairs,
            };

            if kv_count >= threshold {
                // For loops, check if dict depends on loop variables
                if matches!(
                    hot_ctx,
                    HotContext::Loop(_) | HotContext::LoopInProcess { .. }
                ) && node_references_any(node, loop_vars, parsed)
                {
                    return; // Depends on loop var, can't be hoisted
                }

                emit_diagnostic(
                    node,
                    format!(
                        "Dictionary literal allocation inside {}. {}",
                        hot_ctx.description(),
                        hot_ctx.suggestion()
                    ),
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

fn emit_diagnostic(node: Node, message: String, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(
        Diagnostic::new(
            RULE_ID,
            Severity::Warning,
            message,
            node.start_position().row + 1,
        )
        .span(
            node.start_position().column,
            node.end_position().row + 1,
            node.end_position().column,
        ),
    );
}

/// Extract the loop variable(s) from a for statement.
fn collect_loop_variables(loop_node: Node, parsed: &ParsedFile, vars: &mut Vec<String>) {
    if loop_node.kind() != "for_statement" {
        return;
    }

    let mut cursor = loop_node.walk();
    for child in loop_node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "name" {
            let name = parsed.node_text(child).to_string();
            if !vars.contains(&name) {
                vars.push(name);
            }
            break; // First identifier is the loop variable
        }
        // Stop at 'in' keyword or body
        if child.kind() == "in" || child.kind() == "body" {
            break;
        }
    }
}

/// Collect all variables assigned within a node tree.
fn collect_assigned_variables(node: Node, parsed: &ParsedFile, vars: &mut Vec<String>) {
    match node.kind() {
        "assignment" | "augmented_assignment" => {
            if let Some(lhs) = node.child(0) {
                if lhs.kind() == "identifier" || lhs.kind() == "name" {
                    let name = parsed.node_text(lhs).to_string();
                    if !vars.contains(&name) {
                        vars.push(name);
                    }
                }
            }
        }
        "variable_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "name" || child.kind() == "identifier" {
                    let name = parsed.node_text(child).to_string();
                    if !vars.contains(&name) {
                        vars.push(name);
                    }
                    break;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_assigned_variables(child, parsed, vars);
    }
}

/// Check if a node references any of the given variable names.
fn node_references_any(node: Node, vars: &[String], parsed: &ParsedFile) -> bool {
    if node.kind() == "identifier" || node.kind() == "name" {
        let text = parsed.node_text(node);
        if vars.iter().any(|v| v == text) {
            return true;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if node_references_any(child, vars, parsed) {
            return true;
        }
    }

    false
}

/// Count the number of elements in an array literal.
fn count_array_elements(node: Node) -> usize {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|c| c.is_named() && c.kind() != "comment")
        .count()
}

/// Count the number of key-value pairs in a dictionary literal.
fn count_dict_elements(node: Node) -> usize {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|c| c.kind() == "pair")
        .count()
}
