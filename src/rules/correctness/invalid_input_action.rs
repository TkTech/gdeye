use crate::parser::{self, ParsedFile};
use crate::project::ProjectInfo;

use super::super::{Diagnostic, Rule, RuleContext, Severity};

const RULE_ID: &str = "correctness/invalid-input-action";

/// Built-in UI actions that are always valid even if not in project.godot.
const BUILTIN_ACTIONS: &[&str] = &[
    "ui_accept",
    "ui_cancel",
    "ui_select",
    "ui_focus_next",
    "ui_focus_prev",
    "ui_left",
    "ui_right",
    "ui_up",
    "ui_down",
    "ui_page_up",
    "ui_page_down",
    "ui_home",
    "ui_end",
    "ui_text_newline",
    "ui_text_backspace",
    "ui_text_delete",
    "ui_text_indent",
    "ui_undo",
    "ui_redo",
    "ui_copy",
    "ui_cut",
    "ui_paste",
    "ui_swap_input_direction",
    "ui_text_completion_query",
    "ui_text_completion_accept",
    "ui_text_completion_replace",
    "ui_filedialog_up_one_level",
    "ui_filedialog_refresh",
    "ui_filedialog_show_hidden",
    "ui_graph_duplicate",
    "ui_graph_delete",
];

/// Input method patterns to check.
const INPUT_METHODS: &[&str] = &[
    "is_action_pressed",
    "is_action_released",
    "is_action_just_pressed",
    "is_action_just_released",
    "get_action_strength",
    "get_action_raw_strength",
    "get_axis",
    "get_vector",
];

pub struct InvalidInputAction;

impl Rule for InvalidInputAction {
    fn id(&self) -> &'static str {
        RULE_ID
    }

    fn description(&self) -> &'static str {
        "Input action string not defined in project.godot"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn category(&self) -> &'static str {
        "correctness"
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        // Only check if we have input actions defined (project.godot was found)
        if ctx.project_info.input_actions.is_empty() {
            return diagnostics;
        }
        check_invalid_input_actions(ctx.parsed, ctx.project_info, &mut diagnostics);
        diagnostics
    }
}

fn check_invalid_input_actions(
    parsed: &ParsedFile,
    project_info: &ProjectInfo,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let root = parsed.root_node();
    // Input.method() uses attribute nodes with attribute_call children
    let attrs = parser::find_nodes_by_kind(root, "attribute");

    for attr in attrs {
        let mut cursor = attr.walk();
        let children: Vec<_> = attr.children(&mut cursor).collect();

        if children.len() < 2 {
            continue;
        }

        // First child should be "Input"
        let receiver = children[0];
        if receiver.kind() != "identifier" || parsed.node_text(receiver) != "Input" {
            continue;
        }

        // Find attribute_call child
        let attr_call = children.iter().find(|c| c.kind() == "attribute_call");
        let attr_call = match attr_call {
            Some(c) => *c,
            None => continue,
        };

        let mut call_cursor = attr_call.walk();
        let call_children: Vec<_> = attr_call.children(&mut call_cursor).collect();

        if call_children.is_empty() {
            continue;
        }

        // Check method name
        let method = parsed.node_text(call_children[0]);
        if !INPUT_METHODS.contains(&method) {
            continue;
        }

        // Find the arguments
        let args_node = call_children.iter().find(|c| c.kind() == "arguments");
        let args_node = match args_node {
            Some(n) => *n,
            None => continue,
        };

        // Check all string arguments
        let mut args_cursor = args_node.walk();
        let args: Vec<_> = args_node
            .children(&mut args_cursor)
            .filter(|c| c.kind() == "string")
            .collect();

        for arg in args {
            let raw = parsed.node_text(arg);
            let action_name = raw
                .trim_start_matches('"')
                .trim_end_matches('"')
                .trim_start_matches('\'')
                .trim_end_matches('\'');

            if action_name.is_empty() {
                continue;
            }

            let is_valid = project_info.input_actions.iter().any(|a| a == action_name)
                || BUILTIN_ACTIONS.contains(&action_name);

            if !is_valid {
                diagnostics.push(
                    Diagnostic::new(
                        RULE_ID,
                        Severity::Warning,
                        format!(
                            "Input action `{}` is not defined in project.godot.",
                            action_name
                        ),
                        arg.start_position().row + 1,
                    )
                    .span(
                        arg.start_position().column,
                        arg.end_position().row + 1,
                        arg.end_position().column,
                    ),
                );
            }
        }
    }
}
