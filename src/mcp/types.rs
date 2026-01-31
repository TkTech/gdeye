//! MCP parameter and response types.

use serde::Serialize;

/// Response from the format_snippet tool.
#[derive(Debug, Serialize)]
pub struct FormatResponse {
    /// The formatted source code.
    pub formatted: String,
    /// Whether the source was already formatted (no changes made).
    pub unchanged: bool,
}

/// Response from the lint_snippet tool.
#[derive(Debug, Serialize)]
pub struct LintResponse {
    /// List of diagnostics found.
    pub diagnostics: Vec<DiagnosticInfo>,
}

/// A single diagnostic from linting.
#[derive(Debug, Serialize)]
pub struct DiagnosticInfo {
    /// The rule ID that produced this diagnostic.
    pub rule: String,
    /// Severity level: "error", "warning", or "info".
    pub severity: String,
    /// The diagnostic message.
    pub message: String,
    /// Line number (1-based).
    pub line: usize,
    /// Column number (0-based).
    pub col: usize,
    /// End line number (1-based).
    pub end_line: usize,
    /// End column number (0-based).
    pub end_col: usize,
    /// Optional note providing additional context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Response from the list_rules tool.
#[derive(Debug, Serialize)]
pub struct ListRulesResponse {
    /// List of all available rules.
    pub rules: Vec<RuleSummary>,
}

/// Summary information about a lint rule.
#[derive(Debug, Serialize)]
pub struct RuleSummary {
    /// The rule ID (e.g., "correctness/unused-variable").
    pub id: String,
    /// Brief description of what the rule checks.
    pub description: String,
    /// Default severity: "error", "warning", or "info".
    pub default_severity: String,
    /// The category this rule belongs to.
    pub category: String,
}

/// Response from the get_rule_info tool.
#[derive(Debug, Serialize)]
pub struct RuleInfoResponse {
    /// The rule ID.
    pub id: String,
    /// Brief description of what the rule checks.
    pub description: String,
    /// Default severity: "error", "warning", or "info".
    pub default_severity: String,
    /// The category this rule belongs to.
    pub category: String,
    /// Configurable options for this rule.
    pub options: Vec<RuleOptionInfo>,
}

/// Information about a configurable rule option.
#[derive(Debug, Serialize)]
pub struct RuleOptionInfo {
    /// The option name as used in gdeye.toml.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// The default value as a string.
    pub default: String,
    /// The value type: "integer", "string", or "boolean".
    pub value_type: String,
}

// ============================================================================
// New tool response types
// ============================================================================

/// Response from the lookup_godot_class tool.
#[derive(Debug, Serialize)]
pub struct GodotClassResponse {
    /// The class name.
    pub name: String,
    /// Parent class name (empty for root classes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Whether this is a builtin type (Vector2, Array, etc.) vs engine class.
    pub is_builtin: bool,
    /// Methods available on this class.
    pub methods: Vec<MethodSummary>,
    /// Properties available on this class.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<PropertySummary>,
    /// Signals available on this class.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<SignalSummary>,
    /// Constants defined on this class.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub constants: Vec<ConstantSummary>,
}

#[derive(Debug, Serialize)]
pub struct MethodSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    pub arguments: Vec<ArgumentSummary>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_static: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_virtual: bool,
}

#[derive(Debug, Serialize)]
pub struct ArgumentSummary {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: String,
}

#[derive(Debug, Serialize)]
pub struct PropertySummary {
    pub name: String,
    #[serde(rename = "type")]
    pub prop_type: String,
}

#[derive(Debug, Serialize)]
pub struct SignalSummary {
    pub name: String,
    pub arguments: Vec<ArgumentSummary>,
}

#[derive(Debug, Serialize)]
pub struct ConstantSummary {
    pub name: String,
    pub value: i64,
}

/// Response from the get_symbols tool.
#[derive(Debug, Serialize)]
pub struct SymbolsResponse {
    /// Class name if declared with class_name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    /// What class this script extends.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Signal declarations.
    pub signals: Vec<SymbolSignal>,
    /// Enum declarations.
    pub enums: Vec<SymbolEnum>,
    /// Constant declarations.
    pub constants: Vec<SymbolConst>,
    /// Variable declarations (class-level).
    pub variables: Vec<SymbolVar>,
    /// Function declarations.
    pub functions: Vec<SymbolFunc>,
}

#[derive(Debug, Serialize)]
pub struct SymbolSignal {
    pub name: String,
    pub parameters: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolEnum {
    pub name: String,
    pub values: Vec<String>,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolConst {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<String>,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolVar {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_onready: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_export: bool,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolFunc {
    pub name: String,
    pub parameters: Vec<SymbolParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug, Serialize)]
pub struct SymbolParam {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<String>,
}

/// Response from the apply_fix tool.
#[derive(Debug, Serialize)]
pub struct ApplyFixResponse {
    /// The source code with fixes applied.
    pub fixed_source: String,
    /// Number of fixes applied.
    pub num_fixed: usize,
    /// Whether any fixes were available.
    pub had_fixes: bool,
}

/// Response from the parse_scene tool.
#[derive(Debug, Serialize)]
pub struct SceneResponse {
    /// External resources referenced by this scene.
    pub resources: Vec<SceneResource>,
    /// Nodes in the scene tree.
    pub nodes: Vec<SceneNodeInfo>,
    /// Signal connections defined in the scene.
    pub connections: Vec<SceneConnection>,
}

#[derive(Debug, Serialize)]
pub struct SceneResource {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct SceneNodeInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SceneConnection {
    pub signal: String,
    pub from_node: String,
    pub to_node: String,
    pub method: String,
}

/// Response from the list_autoloads tool.
#[derive(Debug, Serialize)]
pub struct AutoloadsResponse {
    /// Project name.
    pub project_name: String,
    /// Autoload singletons in load order.
    pub autoloads: Vec<AutoloadEntry>,
    /// Input actions defined in the project.
    pub input_actions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoloadEntry {
    /// The singleton name (accessible as global).
    pub name: String,
    /// Path to the script (res:// format).
    pub script_path: String,
}

/// Response from the get_call_graph tool.
#[derive(Debug, Serialize)]
pub struct CallGraphResponse {
    /// Functions defined in this file.
    pub functions: Vec<CallGraphFunction>,
    /// Calls made from this file to other functions.
    pub calls: Vec<CallInfo>,
}

#[derive(Debug, Serialize)]
pub struct CallGraphFunction {
    pub name: String,
    pub line: usize,
    pub param_count: usize,
}

#[derive(Debug, Serialize)]
pub struct CallInfo {
    /// Function making the call.
    pub caller: String,
    /// Function being called.
    pub callee: String,
    /// Line where the call occurs.
    pub line: usize,
}

/// Response from the check_type tool.
#[derive(Debug, Serialize)]
pub struct CheckTypeResponse {
    /// The inferred type, or null if unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inferred_type: Option<String>,
    /// Explanation of how the type was determined.
    pub explanation: String,
}
