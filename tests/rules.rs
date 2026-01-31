mod common;

#[path = "rules/allocation.rs"]
mod allocation;
#[path = "rules/await_correctness.rs"]
mod await_correctness;
#[path = "rules/await_in_loop.rs"]
mod await_in_loop;
#[path = "rules/broken_node_path.rs"]
mod broken_node_path;
#[path = "rules/comparison_with_itself.rs"]
mod comparison_with_itself;
#[path = "rules/dead_store.rs"]
mod dead_store;
#[path = "rules/duplicate_dict_key.rs"]
mod duplicate_dict_key;
#[path = "rules/duplicated_load.rs"]
mod duplicated_load;
#[path = "rules/excessive_nesting.rs"]
mod excessive_nesting;
#[path = "rules/function_too_long.rs"]
mod function_too_long;
#[path = "rules/invalid_input_action.rs"]
mod invalid_input_action;
#[path = "rules/loop_invariant.rs"]
mod loop_invariant;
#[path = "rules/match_exhaustiveness.rs"]
mod match_exhaustiveness;
#[path = "rules/missing_return.rs"]
mod missing_return;
#[path = "rules/naming_convention.rs"]
mod naming_convention;
#[path = "rules/no_else_return.rs"]
mod no_else_return;
#[path = "rules/null_access.rs"]
mod null_access;
#[path = "rules/onready_hoist.rs"]
mod onready_hoist;
#[path = "rules/orphan_node.rs"]
mod orphan_node;
#[path = "rules/private_access.rs"]
mod private_access;
#[path = "rules/process_get_node.rs"]
mod process_get_node;
#[path = "rules/return_type_mismatch.rs"]
mod return_type_mismatch;
#[path = "rules/self_assignment.rs"]
mod self_assignment;
#[path = "rules/shadowed_variable.rs"]
mod shadowed_variable;
#[path = "rules/signal_signature_mismatch.rs"]
mod signal_signature_mismatch;
#[path = "rules/standalone_expression.rs"]
mod standalone_expression;
#[path = "rules/string_concat_loop.rs"]
mod string_concat_loop;
#[path = "rules/type_mismatch.rs"]
mod type_mismatch;
#[path = "rules/uninitialized_variable.rs"]
mod uninitialized_variable;
#[path = "rules/unnecessary_pass.rs"]
mod unnecessary_pass;
#[path = "rules/unreachable_code.rs"]
mod unreachable_code;
#[path = "rules/untyped_parameter.rs"]
mod untyped_parameter;
#[path = "rules/untyped_return.rs"]
mod untyped_return;
#[path = "rules/unused_function.rs"]
mod unused_function;
#[path = "rules/unused_parameter.rs"]
mod unused_parameter;
#[path = "rules/unused_signal.rs"]
mod unused_signal;
